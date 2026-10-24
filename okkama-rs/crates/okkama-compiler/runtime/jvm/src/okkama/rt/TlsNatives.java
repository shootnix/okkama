package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.channels.SocketChannel;
import java.security.cert.Certificate;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;
import javax.net.ssl.SNIHostName;
import javax.net.ssl.SNIServerName;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLEngine;
import javax.net.ssl.SSLEngineResult;
import javax.net.ssl.SSLException;
import javax.net.ssl.SSLParameters;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;

/**
 * Natives {@code runtime/tls_transport}: TLS-клиент {@link SSLEngine} поверх
 * TCP-потока, отобранного у {@code runtime/network}. Как у Perl-provider,
 * движок не проверяет пира сам: {@code authorize} — явная проверка цепочки
 * ({@code verify_chain}) до любого прикладного ввода-вывода. Рукопожатие,
 * чтение и закрытие идут на потоке чтения состояния, запись — на потоке
 * записи (SSLEngine допускает параллельные wrap/unwrap).
 */
public final class TlsNatives {
    private TlsNatives() {}

    private static final String DOMAIN = "core.tls_transport";
    private static final int CHUNK = 65536;

    static final class Tls {
        SSLEngine engine;
        SocketChannel channel;
        ExecutorService reader;
        ExecutorService writer;
        /** Шифротекст, полученный, но ещё не разобранный движком. */
        ByteBuffer netIn;
        /** Открытый текст, прочитанный без ожидающего {@code read}. */
        final ByteArrayOutputStream plain = new ByteArrayOutputStream();
        final Object sending = new Object();
        boolean negotiated;
        boolean authorized;
        boolean closing;
        volatile boolean closed;
        boolean eof;
        Future handshake;
        Future reading;
        Future writing;
        Future shutting;
        List<Object> peer;
    }

    private static String symbol(String name, int arity) {
        return "runtime/tls_transport@1::" + name + "/" + arity;
    }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    private static ExecutorService thread(final String name) {
        return Executors.newSingleThreadExecutor(new ThreadFactory() {
            public Thread newThread(Runnable work) {
                Thread thread = new Thread(work, name);
                thread.setDaemon(true);
                return thread;
            }
        });
    }

    private static void abort(Tls state) {
        if (state.closed) {
            return;
        }
        state.closed = true;
        NetNatives.closeQuietly(state.channel);
        state.reader.shutdown();
        state.writer.shutdown();
    }

    private static Tls state(Object handle, String operation) {
        Tls state = (Tls) handle;
        if (state.closed) {
            throw failure("already_closed", operation, "TLS owner is closed");
        }
        return state;
    }

    // ---- клиент ----

    public static Object runtime_tls_transport__client(Object handle, BigInteger versions, List<?> protocols,
            String server) {
        String operation = symbol("client", 4);
        NetNatives.Stream source = NetNatives.stream(handle, operation);
        if (source.family != 1 && source.family != 2) {
            throw failure("invalid_input", operation, "TLS requires a managed TCP stream");
        }
        if (source.inputClosed || source.outputClosed || source.eof) {
            throw failure("already_closed", operation, "TLS requires both open TCP halves");
        }
        if (!source.channel.isConnected()) {
            throw failure("invalid_input", operation, "TLS requires a connected TCP stream");
        }
        if (source.reading != null || source.writing != null || source.reader != null) {
            throw failure("resource_exhausted", operation, "TLS requires drained TCP operations");
        }
        // Владение TCP переходит к TLS: дескриптор сети закрыт, канал — нет.
        source.closed = true;
        Tls state = new Tls();
        state.channel = source.channel;
        state.reader = thread("okkama-tls-read");
        state.writer = thread("okkama-tls-write");
        byte[] early = source.pending != null ? source.pending : new byte[0];
        source.pending = null;
        state.netIn = ByteBuffer.allocate(Math.max(early.length, 1 << 17));
        state.netIn.put(early);
        state.netIn.flip();
        int choice = versions.bitLength() > 31 ? 0 : versions.intValue();
        if (choice != 1 && choice != 2 && choice != 3) {
            abort(state);
            throw failure("invalid_input", operation, "invalid TLS version selection");
        }
        ArrayList<String> names = new ArrayList<String>();
        boolean valid = protocols.size() <= 16 && server.length() <= 253 && server.matches("[A-Za-z0-9.-]*");
        for (Object protocol : protocols) {
            String name = (String) protocol;
            boolean latin = true;
            for (int index = 0; index < name.length(); index++) {
                latin &= name.charAt(index) < 256;
            }
            if (name.isEmpty() || name.length() > 255 || !latin || names.contains(name)) {
                valid = false;
            }
            names.add(name);
        }
        if (!valid) {
            abort(state);
            throw failure("invalid_input", operation, "invalid TLS client configuration");
        }
        try {
            SSLContext context = SSLContext.getInstance("TLS");
            context.init(null, new TrustManager[] {new Deferred()}, null);
            SSLEngine engine = context.createSSLEngine();
            engine.setUseClientMode(true);
            engine.setEnabledProtocols(choice == 1 ? new String[] {"TLSv1.2"}
                : choice == 2 ? new String[] {"TLSv1.3"} : new String[] {"TLSv1.3", "TLSv1.2"});
            SSLParameters parameters = engine.getSSLParameters();
            if (!server.isEmpty() && !server.matches("[0-9.]+")) {
                ArrayList<SNIServerName> sni = new ArrayList<SNIServerName>();
                sni.add(new SNIHostName(server));
                parameters.setServerNames(sni);
            }
            if (!names.isEmpty()) {
                // ALPN: API Java 9+, исходники рантайма — под Java 8.
                SSLParameters.class.getMethod("setApplicationProtocols", String[].class)
                    .invoke(parameters, (Object) names.toArray(new String[0]));
            }
            engine.setSSLParameters(parameters);
            state.engine = engine;
        } catch (Exception error) {
            abort(state);
            throw failure("unsupported", operation, "required TLS engine policy unavailable");
        }
        return state;
    }

    /** Проверка пира отложена до {@code authorize}. */
    static final class Deferred implements X509TrustManager {
        public void checkClientTrusted(X509Certificate[] chain, String auth) {}

        public void checkServerTrusted(X509Certificate[] chain, String auth) {}

        public X509Certificate[] getAcceptedIssuers() {
            return new X509Certificate[0];
        }
    }

    public static boolean runtime_tls_transport__close(Object handle) {
        Tls state = (Tls) handle;
        abort(state);
        String operation = symbol("close", 1);
        for (Future pending : new Future[] {state.handshake, state.reading, state.writing, state.shutting}) {
            if (pending != null) {
                Scheduler.settle(pending, null, failure("already_closed", operation, "TLS owner is closed"));
            }
        }
        return true;
    }

    public static boolean runtime_tls_transport__closed(Object handle) {
        return ((Tls) handle).closed;
    }

    // ---- движок ----

    /** Записать шифротекст полностью. */
    private static void send(Tls state, ByteBuffer out) throws IOException {
        out.flip();
        while (out.hasRemaining()) {
            state.channel.write(out);
        }
        out.clear();
    }

    /** {@code wrap} исходящих данных (или служебных записей) и отправка. */
    private static SSLEngineResult wrap(Tls state, ByteBuffer source) throws IOException {
        synchronized (state.sending) {
            ByteBuffer out = ByteBuffer.allocate(state.engine.getSession().getPacketBufferSize());
            while (true) {
                SSLEngineResult result = state.engine.wrap(source, out);
                if (result.getStatus() == SSLEngineResult.Status.BUFFER_OVERFLOW) {
                    out = ByteBuffer.allocate(out.capacity() * 2);
                    continue;
                }
                send(state, out);
                runTasks(state, result);
                return result;
            }
        }
    }

    private static void runTasks(Tls state, SSLEngineResult result) {
        if (result.getHandshakeStatus() == SSLEngineResult.HandshakeStatus.NEED_TASK) {
            Runnable task;
            while ((task = state.engine.getDelegatedTask()) != null) {
                task.run();
            }
        }
    }

    /** Дочитать шифротекст из сокета; {@code false} — конец потока. */
    private static boolean fill(Tls state) throws IOException {
        state.netIn.compact();
        try {
            if (!state.netIn.hasRemaining()) {
                ByteBuffer bigger = ByteBuffer.allocate(state.netIn.capacity() * 2);
                state.netIn.flip();
                bigger.put(state.netIn);
                state.netIn = bigger;
            }
            return state.channel.read(state.netIn) >= 0;
        } finally {
            state.netIn.flip();
        }
    }

    /**
     * {@code unwrap} одной записи: открытый текст — в {@code out}. {@code null}
     * — сокет закрыт без разбираемых данных.
     */
    private static SSLEngineResult unwrap(Tls state, ByteBuffer[] out) throws IOException {
        while (true) {
            SSLEngineResult result = state.engine.unwrap(state.netIn, out[0]);
            switch (result.getStatus()) {
                case BUFFER_OVERFLOW: {
                    ByteBuffer bigger = ByteBuffer.allocate(out[0].capacity() * 2 + state.engine.getSession()
                        .getApplicationBufferSize());
                    out[0].flip();
                    bigger.put(out[0]);
                    out[0] = bigger;
                    continue;
                }
                case BUFFER_UNDERFLOW:
                    if (!fill(state)) {
                        return null;
                    }
                    continue;
                default:
                    runTasks(state, result);
                    return result;
            }
        }
    }

    private static void negotiate(Tls state) throws IOException {
        SSLEngine engine = state.engine;
        engine.beginHandshake();
        ByteBuffer[] plain = {ByteBuffer.allocate(engine.getSession().getApplicationBufferSize())};
        ByteBuffer empty = ByteBuffer.allocate(0);
        while (true) {
            SSLEngineResult.HandshakeStatus status = engine.getHandshakeStatus();
            switch (status) {
                case NEED_WRAP:
                    wrap(state, empty);
                    break;
                case NEED_UNWRAP:
                    if (unwrap(state, plain) == null) {
                        throw new SSLException("connection closed during handshake");
                    }
                    break;
                case NEED_TASK: {
                    Runnable task;
                    while ((task = engine.getDelegatedTask()) != null) {
                        task.run();
                    }
                    break;
                }
                default:
                    plain[0].flip();
                    if (plain[0].hasRemaining()) {
                        byte[] early = new byte[plain[0].remaining()];
                        plain[0].get(early);
                        synchronized (state.plain) {
                            state.plain.write(early, 0, early.length);
                        }
                    }
                    return;
            }
        }
    }

    public static Future runtime_tls_transport__handshake(Object handle) {
        final String operation = symbol("handshake", 1);
        final Tls state;
        try {
            state = state(handle, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (state.negotiated) {
            return Scheduler.resolved(Boolean.TRUE, null);
        }
        if (state.handshake != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation, "TLS handshake already pending"));
        }
        final Future promise = Future.promise();
        state.handshake = promise;
        final Scheduler.Op op = Scheduler.offload(state.reader, new Callable<Object>() {
            public Object call() throws IOException {
                negotiate(state);
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Scheduler.settle(promise, null, state.closed ? failure("already_closed", operation,
                        "TLS handshake readiness closed") : failure("invalid_data", operation, "TLS client handshake failed"));
                    return;
                }
                state.negotiated = true;
                Scheduler.settle(promise, Boolean.TRUE, null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                state.handshake = null;
                op.detach();
                if (promise.state != Future.DONE) {
                    abort(state);
                }
            }
        });
        return promise;
    }

    // ---- проверка пира ----

    private static List<Object> peerSources(Tls state, String operation) {
        if (!state.negotiated) {
            throw failure("invalid_input", operation, "TLS peer is not negotiated");
        }
        Certificate[] certificates;
        try {
            certificates = state.engine.getSession().getPeerCertificates();
        } catch (javax.net.ssl.SSLPeerUnverifiedException error) {
            throw failure("invalid_data", operation, "TLS peer certificate is absent");
        }
        if (certificates.length == 0) {
            throw failure("invalid_data", operation, "TLS peer certificate is absent");
        }
        if (certificates.length > 32) {
            throw failure("resource_exhausted", operation, "TLS peer certificate count limit");
        }
        ArrayList<Object> sources = new ArrayList<Object>();
        long total = 0;
        for (Certificate certificate : certificates) {
            byte[] der;
            try {
                der = certificate.getEncoded();
            } catch (java.security.cert.CertificateEncodingException error) {
                throw failure("invalid_data", operation, "TLS peer certificate encoding failed");
            }
            total += der.length;
            if (der.length > 1048576 || total > 8388608) {
                throw failure("resource_exhausted", operation, "TLS peer certificate byte limit");
            }
            sources.add(Natives.fromBytes(der, der.length));
        }
        return sources;
    }

    public static boolean runtime_tls_transport__authorize(Object handle, List<?> roots, String kind, String host,
            String at) {
        String operation = symbol("authorize", 5);
        Tls state = state(handle, operation);
        try {
            if (roots.size() > 1024) {
                throw failure("resource_exhausted", operation, "TLS root count limit");
            }
            long total = 0;
            for (Object root : roots) {
                int size = ((List<?>) root).size();
                total += size;
                if (size > 1048576 || total > 16777216) {
                    throw failure("resource_exhausted", operation, "TLS root byte limit");
                }
            }
            List<Object> sources = peerSources(state, operation);
            X509Natives.runtime_x509__verify_chain(sources, roots, kind, host, at, "1.3.6.1.5.5.7.3.1");
            state.peer = sources;
            state.authorized = true;
            return true;
        } catch (Failure failure) {
            abort(state);
            throw failure;
        }
    }

    private static Failure unverified(Tls state, String operation) {
        if (state.closed) {
            return failure("already_closed", operation, "TLS owner is closed");
        }
        if (!state.authorized) {
            return failure("permission_denied", operation, "TLS peer is not verified");
        }
        return null;
    }

    // ---- ввод-вывод ----

    private static byte[] takePlain(Tls state, int limit) {
        synchronized (state.plain) {
            byte[] all = state.plain.toByteArray();
            if (all.length == 0) {
                return null;
            }
            int count = Math.min(all.length, limit);
            state.plain.reset();
            state.plain.write(all, count, all.length - count);
            return java.util.Arrays.copyOf(all, count);
        }
    }

    private static void returnPlain(Tls state, byte[] bytes) {
        synchronized (state.plain) {
            byte[] rest = state.plain.toByteArray();
            state.plain.reset();
            state.plain.write(bytes, 0, bytes.length);
            state.plain.write(rest, 0, rest.length);
        }
    }

    /** Следующий открытый текст; пусто — {@code close_notify} пира. */
    private static byte[] receive(Tls state, int limit) throws IOException {
        byte[] ready = takePlain(state, limit);
        if (ready != null) {
            return ready;
        }
        if (state.eof) {
            return new byte[0];
        }
        ByteBuffer[] plain = {ByteBuffer.allocate(state.engine.getSession().getApplicationBufferSize())};
        while (true) {
            SSLEngineResult result = unwrap(state, plain);
            if (result == null) {
                try {
                    state.engine.closeInbound();
                } catch (SSLException truncated) {
                    throw new Truncated();
                }
                state.eof = true;
                return new byte[0];
            }
            if (result.getHandshakeStatus() == SSLEngineResult.HandshakeStatus.NEED_WRAP) {
                wrap(state, ByteBuffer.allocate(0));
            }
            if (result.getStatus() == SSLEngineResult.Status.CLOSED) {
                state.eof = true;
            }
            if (plain[0].position() > 0 || state.eof) {
                plain[0].flip();
                byte[] bytes = new byte[plain[0].remaining()];
                plain[0].get(bytes);
                if (bytes.length > limit) {
                    returnPlain(state, java.util.Arrays.copyOfRange(bytes, limit, bytes.length));
                    bytes = java.util.Arrays.copyOf(bytes, limit);
                }
                return bytes;
            }
        }
    }

    static final class Truncated extends IOException {
        private static final long serialVersionUID = 1L;
    }

    public static Future runtime_tls_transport__read(Object handle, BigInteger limitValue) {
        final String operation = symbol("read", 2);
        if (limitValue.signum() <= 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "TLS read limit must be positive"));
        }
        final Tls state = (Tls) handle;
        Failure refused = unverified(state, operation);
        if (refused == null && state.closing) {
            refused = failure("already_closed", operation, "TLS owner is shutting down");
        }
        if (refused == null && state.reading != null) {
            refused = failure("resource_exhausted", operation, "TLS operation already pending");
        }
        if (refused != null) {
            return Scheduler.resolved(null, refused);
        }
        final int limit = limitValue.compareTo(BigInteger.valueOf(CHUNK)) > 0 ? CHUNK : limitValue.intValue();
        byte[] ready = takePlain(state, limit);
        if (ready != null) {
            return Scheduler.resolved(Natives.fromBytes(ready, ready.length), null);
        }
        final Future promise = Future.promise();
        state.reading = promise;
        final Scheduler.Op op = Scheduler.offload(state.reader, new Callable<Object>() {
            public Object call() throws IOException {
                return receive(state, limit);
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (promise.state != Future.RUNNING) {
                    if (value != null) {
                        returnPlain(state, (byte[]) value);
                    }
                    return;
                }
                if (error instanceof Truncated) {
                    Scheduler.settle(promise, null, failure("invalid_data", operation, "TLS peer truncated input"));
                } else if (error != null) {
                    Scheduler.settle(promise, null, state.closed ? failure("already_closed", operation,
                        "TLS I/O readiness closed") : failure("invalid_data", operation, "TLS I/O failed"));
                } else {
                    byte[] bytes = (byte[]) value;
                    Scheduler.settle(promise, Natives.fromBytes(bytes, bytes.length), null);
                }
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                state.reading = null;
                op.detach();
                if (promise.state == Future.FAILED && !Future.cancelled(promise)) {
                    abort(state);
                }
            }
        });
        return promise;
    }

    public static Future runtime_tls_transport__write(Object handle, List<?> source) {
        final String operation = symbol("write", 2);
        final Tls state = (Tls) handle;
        Failure refused = unverified(state, operation);
        if (refused == null && state.closing) {
            refused = failure("already_closed", operation, "TLS owner is shutting down");
        }
        if (refused == null && state.writing != null) {
            refused = failure("resource_exhausted", operation, "TLS operation already pending");
        }
        if (refused != null) {
            return Scheduler.resolved(null, refused);
        }
        final byte[] payload = Natives.toBytes(source);
        final Future promise = Future.promise();
        state.writing = promise;
        final Scheduler.Op op = Scheduler.offload(state.writer, new Callable<Object>() {
            public Object call() throws IOException {
                ByteBuffer buffer = ByteBuffer.wrap(payload);
                while (buffer.hasRemaining()) {
                    SSLEngineResult result = wrap(state, buffer);
                    if (result.getStatus() == SSLEngineResult.Status.CLOSED) {
                        throw new SSLException("TLS output is closed");
                    }
                }
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                Scheduler.settle(promise, error == null ? Boolean.TRUE : null, error == null ? null
                    : state.closed ? failure("already_closed", operation, "TLS I/O readiness closed")
                    : failure("invalid_data", operation, "TLS I/O failed"));
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                state.writing = null;
                op.detach();
                // Как у Perl: незавершённая запись закрывает состояние.
                if (promise.state != Future.DONE) {
                    abort(state);
                }
            }
        });
        return promise;
    }

    public static Future runtime_tls_transport__shutdown(Object handle) {
        final String operation = symbol("shutdown", 1);
        final Tls state = (Tls) handle;
        if (state.reading != null || state.writing != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation,
                "TLS shutdown requires drained operations"));
        }
        Failure refused = unverified(state, operation);
        if (refused == null && state.shutting != null) {
            refused = failure("resource_exhausted", operation, "TLS operation already pending");
        }
        if (refused != null) {
            return Scheduler.resolved(null, refused);
        }
        state.closing = true;
        final Future promise = Future.promise();
        state.shutting = promise;
        final Scheduler.Op op = Scheduler.offload(state.reader, new Callable<Object>() {
            public Object call() throws IOException {
                state.engine.closeOutbound();
                ByteBuffer empty = ByteBuffer.allocate(0);
                while (!state.engine.isOutboundDone()) {
                    wrap(state, empty);
                }
                // Двусторонний shutdown: ждём close_notify пира.
                ByteBuffer[] plain = {ByteBuffer.allocate(state.engine.getSession().getApplicationBufferSize())};
                while (!state.engine.isInboundDone() && !state.eof) {
                    SSLEngineResult result = unwrap(state, plain);
                    if (result == null || result.getStatus() == SSLEngineResult.Status.CLOSED) {
                        break;
                    }
                    plain[0].clear();
                }
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                Scheduler.settle(promise, error == null ? Boolean.TRUE : null, error == null ? null
                    : state.closed ? failure("already_closed", operation, "TLS I/O readiness closed")
                    : failure("invalid_data", operation, "TLS I/O failed"));
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                state.shutting = null;
                op.detach();
                abort(state);
            }
        });
        return promise;
    }

    // ---- наблюдения ----

    private static Tls verified(Object handle, String operation) {
        Tls state = state(handle, operation);
        if (!state.authorized) {
            throw failure("permission_denied", operation, "TLS peer is not verified");
        }
        return state;
    }

    public static List<Object> runtime_tls_transport__peer(Object handle) {
        return new ArrayList<Object>(verified(handle, symbol("peer", 1)).peer);
    }

    public static String runtime_tls_transport__version(Object handle) {
        // Имена OpenSSL: TLSv1_3, TLSv1_2.
        return verified(handle, symbol("version", 1)).engine.getSession().getProtocol().replace('.', '_');
    }

    public static String runtime_tls_transport__cipher(Object handle) {
        return opensslCipher(verified(handle, symbol("cipher", 1)).engine.getSession().getCipherSuite());
    }

    public static String runtime_tls_transport__protocol(Object handle) {
        Tls state = verified(handle, symbol("protocol", 1));
        try {
            Object selected = SSLEngine.class.getMethod("getApplicationProtocol").invoke(state.engine);
            return selected == null ? "" : (String) selected;
        } catch (ReflectiveOperationException error) {
            return "";
        }
    }

    /** Имя набора по OpenSSL: TLS 1.3 совпадает с IANA, TLS 1.2 — {@code ECDHE-RSA-AES128-GCM-SHA256}. */
    static String opensslCipher(String iana) {
        java.util.regex.Matcher match = java.util.regex.Pattern
            .compile("TLS_(ECDHE|DHE)_(RSA|ECDSA)_WITH_(AES_(128|256)_GCM|CHACHA20_POLY1305)_(SHA256|SHA384)")
            .matcher(iana);
        if (!match.matches()) {
            return iana;
        }
        String exchange = match.group(1);
        String auth = match.group(2);
        if (match.group(4) == null) {
            return exchange + "-" + auth + "-CHACHA20-POLY1305";
        }
        return exchange + "-" + auth + "-AES" + match.group(4) + "-GCM-" + match.group(5);
    }
}
