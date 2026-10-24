package okkama.rt;

import java.io.IOException;
import java.math.BigInteger;
import java.net.BindException;
import java.net.ConnectException;
import java.net.Inet4Address;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.NetworkInterface;
import java.net.NoRouteToHostException;
import java.net.SocketException;
import java.net.SocketTimeoutException;
import java.net.StandardSocketOptions;
import java.net.UnknownHostException;
import java.nio.ByteBuffer;
import java.nio.channels.ClosedChannelException;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;

/**
 * Natives {@code runtime/network} (TCP-слушатели и потоки) и
 * {@code runtime/net} (числовые IP-адреса). Сокеты хоста блокирующие:
 * каждая ожидающая операция — на рабочем потоке, исход приходит событием
 * цикла (см. {@link Scheduler#offload}). Результат, пришедший после отмены
 * ожидания (принятое соединение, прочитанные байты), не теряется: его
 * получит следующая операция. Тексты отказов — как у Perl-provider.
 */
public final class NetNatives {
    private NetNatives() {}

    private static final String DOMAIN = "core.net";
    private static final int IPV4 = 1;
    private static final int IPV6 = 2;
    private static final int CHUNK = 65536;

    static final class Listener {
        ServerSocketChannel channel;
        int family;
        boolean listening;
        boolean closed;
        Future accepting;
        boolean keepAlive;
        boolean noDelay;
        Scheduler.Op worker;
        /** Путь и идентичность (dev, inode) файла Unix-listener. */
        String path;
        Object identity;
        /** Принятые без ожидающего {@code accept}: канал или ошибка. */
        final ArrayDeque<Object> ready = new ArrayDeque<Object>();
    }

    /** Сокет {@code SocketState}: поток (TCP, Unix) или UDP. */
    abstract static class Sock {
        int family;
        boolean closed;

        abstract void shut();
    }

    static final class Stream extends Sock {
        SocketChannel channel;
        /** Сокет пары Unix без имени: адреса концов пусты. */
        boolean unnamed;
        boolean inputClosed;
        boolean outputClosed;
        boolean eof;
        Future reading;
        int readLimit;
        Scheduler.Op reader;
        /** Прочитанное без ожидающего {@code stream_read}. */
        byte[] pending;
        Throwable readError;
        Future writing;

        void shut() {
            closeStream(this);
        }

        String label() {
            return family == 3 ? "Unix socket" : "TCP socket";
        }
    }

    static String symbol(String name, int arity) {
        return "runtime/network@1::" + name + "/" + arity;
    }

    static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    /** Код переносимого отказа по исключению хоста (аналог errno у Perl). */
    private static String hostCode(Throwable error) {
        String text = String.valueOf(error.getMessage()).toLowerCase();
        if (error instanceof ClosedChannelException) {
            return "already_closed";
        }
        if (error instanceof BindException || text.contains("address already in use")) {
            return text.contains("assign requested address") ? "address_not_available" : "address_in_use";
        }
        if (error instanceof ConnectException || text.contains("connection refused")) {
            return "connection_refused";
        }
        if (error instanceof NoRouteToHostException || text.contains("unreachable")) {
            return "network_unreachable";
        }
        if (error instanceof SocketTimeoutException || text.contains("timed out")) {
            return "timed_out";
        }
        if (text.contains("reset") || text.contains("aborted")) {
            return "connection_reset";
        }
        if (text.contains("broken pipe")) {
            return "broken_pipe";
        }
        if (text.contains("not connected")) {
            return "not_connected";
        }
        if (text.contains("permission denied") || text.contains("not permitted")) {
            return "permission_denied";
        }
        if (text.contains("too many open files")) {
            return "resource_exhausted";
        }
        return "unexpected";
    }

    static Failure hostFailure(String operation, String message, Throwable error) {
        if (error instanceof Failure) {
            return (Failure) error;
        }
        return failure(hostCode(error), operation, message);
    }

    // ---- конверт адреса: 'O' 'K' 1 family port(be) address ----

    static final class Endpoint {
        int family;
        int port;
        byte[] address;
    }

    private static Endpoint decode(List<?> value, String operation) {
        byte[] bytes = Natives.toBytes(value);
        Endpoint endpoint = new Endpoint();
        int family = bytes.length >= 6 ? bytes[3] & 0xff : 0;
        int expected = family == IPV4 ? 10 : family == IPV6 ? 22 : 0;
        if (bytes.length < 6 || bytes[0] != 79 || bytes[1] != 75 || bytes[2] != 1
                || expected == 0 || bytes.length != expected) {
            throw failure("invalid_data", operation, "Invalid network address envelope");
        }
        endpoint.family = family;
        endpoint.port = ((bytes[4] & 0xff) << 8) | (bytes[5] & 0xff);
        endpoint.address = new byte[bytes.length - 6];
        System.arraycopy(bytes, 6, endpoint.address, 0, endpoint.address.length);
        return endpoint;
    }

    private static List<Object> encode(int family, int port, byte[] address) {
        byte[] bytes = new byte[6 + address.length];
        bytes[0] = 79;
        bytes[1] = 75;
        bytes[2] = 1;
        bytes[3] = (byte) family;
        bytes[4] = (byte) (port >> 8);
        bytes[5] = (byte) port;
        System.arraycopy(address, 0, bytes, 6, address.length);
        return Natives.fromBytes(bytes, bytes.length);
    }

    /** Адрес сокета хоста; зона IPv6 — числовой индекс или имя интерфейса. */
    private static InetSocketAddress socketAddress(Endpoint endpoint, String zone, String operation) {
        try {
            if (endpoint.family == IPV4) {
                if (!zone.isEmpty()) {
                    throw failure("invalid_input", operation, "Invalid IPv6 binding zone");
                }
                return new InetSocketAddress(InetAddress.getByAddress(endpoint.address), endpoint.port);
            }
            if (zone.isEmpty()) {
                return new InetSocketAddress(
                    Inet6Address.getByAddress(null, endpoint.address, -1), endpoint.port);
            }
            if (!zone.matches("[A-Za-z0-9._~-]+")) {
                throw failure("invalid_input", operation, "Invalid IPv6 binding zone");
            }
            if (zone.matches("[0-9]+")) {
                String digits = zone.replaceFirst("^0+", "");
                if (digits.length() > 10 || (digits.length() == 10 && digits.compareTo("4294967295") > 0)) {
                    throw failure("invalid_input", operation, "IPv6 scope index exceeds uint32");
                }
                int scope = digits.isEmpty() ? 0 : (int) Long.parseLong(digits);
                return new InetSocketAddress(
                    Inet6Address.getByAddress(null, endpoint.address, scope), endpoint.port);
            }
            NetworkInterface face = NetworkInterface.getByName(zone);
            if (face == null || face.getIndex() <= 0) {
                throw failure("invalid_input", operation, "Unknown or unsupported IPv6 interface zone");
            }
            return new InetSocketAddress(
                Inet6Address.getByAddress(null, endpoint.address, face), endpoint.port);
        } catch (UnknownHostException | SocketException error) {
            throw failure("invalid_input", operation, "Unknown or unsupported IPv6 interface zone");
        }
    }

    /** [host, port, scope|""] как у Perl `inet_ntop`. */
    private static List<Object> endpointStrings(InetSocketAddress address, int family) {
        InetAddress host = address.getAddress();
        byte[] raw = host.getAddress();
        String text;
        String scope = "";
        if (family == IPV6 && raw.length == 4) {
            byte[] mapped = new byte[16];
            mapped[10] = (byte) 0xff;
            mapped[11] = (byte) 0xff;
            System.arraycopy(raw, 0, mapped, 12, 4);
            text = format(mapped);
        } else {
            text = format(raw);
            if (host instanceof Inet6Address && ((Inet6Address) host).getScopeId() != 0) {
                scope = Integer.toString(((Inet6Address) host).getScopeId());
            }
        }
        ArrayList<Object> out = new ArrayList<Object>(3);
        out.add(text);
        out.add(Integer.toString(address.getPort()));
        out.add(scope);
        return out;
    }

    // ---- IP-текст: inet_pton / inet_ntop ----

    static byte[] parseIpv4(String text) {
        String[] parts = text.split("\\.", -1);
        if (parts.length != 4) {
            return null;
        }
        byte[] out = new byte[4];
        for (int index = 0; index < 4; index++) {
            String part = parts[index];
            if (!part.matches("0|[1-9][0-9]{0,2}") || Integer.parseInt(part) > 255) {
                return null;
            }
            out[index] = (byte) Integer.parseInt(part);
        }
        return out;
    }

    static byte[] parseIpv6(String text) {
        int gap = text.indexOf("::");
        if (gap >= 0 && text.indexOf("::", gap + 1) >= 0) {
            return null;
        }
        ArrayList<Integer> head = new ArrayList<Integer>();
        ArrayList<Integer> tail = new ArrayList<Integer>();
        if (gap >= 0) {
            if (!groups(text.substring(0, gap), head, false)
                    || !groups(text.substring(gap + 2), tail, true)) {
                return null;
            }
            if (head.size() + tail.size() > 7) {
                return null;
            }
        } else if (!groups(text, head, true) || head.size() != 8) {
            return null;
        }
        byte[] out = new byte[16];
        for (int index = 0; index < head.size(); index++) {
            out[index * 2] = (byte) (head.get(index) >> 8);
            out[index * 2 + 1] = (byte) (int) head.get(index);
        }
        int base = 8 - tail.size();
        for (int index = 0; index < tail.size(); index++) {
            out[(base + index) * 2] = (byte) (tail.get(index) >> 8);
            out[(base + index) * 2 + 1] = (byte) (int) tail.get(index);
        }
        return out;
    }

    /** Группы `h16` через `:`; последняя может быть IPv4 (две группы). */
    private static boolean groups(String text, List<Integer> out, boolean last) {
        if (text.isEmpty()) {
            return true;
        }
        String[] parts = text.split(":", -1);
        for (int index = 0; index < parts.length; index++) {
            String part = parts[index];
            if (last && index == parts.length - 1 && part.indexOf('.') >= 0) {
                byte[] four = parseIpv4(part);
                if (four == null) {
                    return false;
                }
                out.add(((four[0] & 0xff) << 8) | (four[1] & 0xff));
                out.add(((four[2] & 0xff) << 8) | (four[3] & 0xff));
                continue;
            }
            if (!part.matches("[0-9A-Fa-f]{1,4}")) {
                return false;
            }
            out.add(Integer.parseInt(part, 16));
        }
        return true;
    }

    /** {@code inet_ntop} BSD: самый длинный ряд ≥ 2 нулевых групп — `::`. */
    static String format(byte[] raw) {
        if (raw.length == 4) {
            return (raw[0] & 0xff) + "." + (raw[1] & 0xff) + "." + (raw[2] & 0xff) + "." + (raw[3] & 0xff);
        }
        int[] words = new int[8];
        for (int index = 0; index < 8; index++) {
            words[index] = ((raw[index * 2] & 0xff) << 8) | (raw[index * 2 + 1] & 0xff);
        }
        int best = -1;
        int bestLength = 0;
        for (int index = 0; index < 8;) {
            if (words[index] != 0) {
                index++;
                continue;
            }
            int start = index;
            while (index < 8 && words[index] == 0) {
                index++;
            }
            if (index - start > bestLength) {
                best = start;
                bestLength = index - start;
            }
        }
        if (bestLength < 2) {
            best = -1;
        }
        StringBuilder out = new StringBuilder();
        for (int index = 0; index < 8; index++) {
            if (best >= 0 && index >= best && index < best + bestLength) {
                if (index == best) {
                    out.append(':');
                }
                continue;
            }
            if (index != 0) {
                out.append(':');
            }
            if (index == 6 && best == 0 && (bestLength == 6
                    || (bestLength == 7 && words[7] != 0x0001)
                    || (bestLength == 5 && words[5] == 0xffff))) {
                out.append(raw[12] & 0xff).append('.').append(raw[13] & 0xff).append('.')
                    .append(raw[14] & 0xff).append('.').append(raw[15] & 0xff);
                break;
            }
            out.append(Integer.toHexString(words[index]));
        }
        if (best >= 0 && best + bestLength == 8) {
            out.append(':');
        }
        return out.toString();
    }

    public static List<Object> runtime_net__ip_parse(BigInteger family, String text, BigInteger port) {
        String operation = "runtime/net@1::ip_parse/3";
        if (!family.equals(BigInteger.valueOf(IPV4)) && !family.equals(BigInteger.valueOf(IPV6))) {
            throw failure("unsupported", operation, "IP address family is not supported");
        }
        if (port.signum() < 0 || port.compareTo(BigInteger.valueOf(65535)) > 0) {
            throw failure("invalid_input", operation, "Network port must be in range 0..65535");
        }
        int kind = family.intValue();
        if (kind == IPV4 || text.indexOf('.') >= 0) {
            String dotted = text.substring(text.lastIndexOf(':') + 1);
            if (parseIpv4(dotted) == null) {
                throw failure("invalid_input", operation, "Invalid numeric IP address");
            }
        }
        byte[] address = kind == IPV4 ? parseIpv4(text) : parseIpv6(text);
        if (address == null) {
            throw failure("invalid_input", operation, "Invalid numeric IP address");
        }
        return encode(kind, port.intValue(), address);
    }

    public static String runtime_net__ip_format(List<?> value) {
        return format(decode(value, "runtime/net@1::ip_format/1").address);
    }

    // ---- TCP-слушатель ----

    static Listener listener(Object handle, String operation) {
        Listener state = (Listener) handle;
        if (state.closed) {
            throw failure("already_closed", operation, "Native handle is already closed");
        }
        return state;
    }

    public static Object runtime_network__tcp_bound(List<?> address, String zone, boolean reuse,
            boolean dualStack) {
        String operation = symbol("tcp_bound", 4);
        Endpoint target = decode(address, operation);
        if (target.family == IPV4 && dualStack) {
            throw failure("unsupported", operation, "Dual stack requires IPv6 listener");
        }
        InetSocketAddress local = socketAddress(target, zone, operation);
        ServerSocketChannel channel;
        try {
            channel = ServerSocketChannel.open();
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot create TCP socket", error);
        }
        try {
            channel.setOption(StandardSocketOptions.SO_REUSEADDR, reuse);
        } catch (IOException error) {
            closeQuietly(channel);
            throw hostFailure(operation, "Cannot apply TCP listener address reuse", error);
        }
        try {
            // JVM связывает и начинает слушать одним вызовом; `tcp_start_listening`
            // разрешает `accept`.
            channel.bind(local, 0);
        } catch (IOException error) {
            closeQuietly(channel);
            throw hostFailure(operation, "Cannot bind TCP listener", error);
        }
        Listener state = new Listener();
        state.channel = channel;
        state.family = target.family;
        return state;
    }

    public static boolean runtime_network__tcp_start_listening(Object handle, BigInteger backlog) {
        String operation = symbol("tcp_start_listening", 2);
        Listener state = listener(handle, operation);
        if (backlog.signum() < 0 || backlog.compareTo(BigInteger.valueOf(Integer.MAX_VALUE)) > 0) {
            throw failure("invalid_input", operation, "TCP backlog must be nonnegative and fit a host int");
        }
        if (state.listening) {
            throw failure("invalid_input", operation, "TCP listener is already listening");
        }
        state.listening = true;
        return true;
    }

    public static List<Object> runtime_network__tcp_listener_endpoint(Object handle) {
        String operation = symbol("tcp_listener_endpoint", 1);
        Listener state = listener(handle, operation);
        try {
            return endpointStrings((InetSocketAddress) state.channel.getLocalAddress(), state.family);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot inspect TCP endpoint", error);
        }
    }

    public static Future runtime_network__tcp_accept(Object handle, boolean keepAlive, boolean noDelay) {
        String operation = symbol("tcp_accept", 3);
        final Listener state;
        try {
            state = listener(handle, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (!state.listening) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "TCP socket is not listening"));
        }
        return accept(state, keepAlive, noDelay);
    }

    /** Общий {@code accept} TCP и Unix listener. */
    static Future accept(final Listener state, boolean keepAlive, boolean noDelay) {
        if (state.accepting != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", acceptSymbol(state),
                (state.family == 3 ? "Unix socket" : "TCP listener") + " already has a pending accepting"));
        }
        final Future promise = Future.promise();
        state.accepting = promise;
        state.keepAlive = keepAlive;
        state.noDelay = noDelay;
        promise.listen(new Runnable() {
            public void run() {
                state.accepting = null;
                if (state.worker != null) {
                    state.worker.detach();
                }
            }
        });
        if (!state.ready.isEmpty()) {
            serveAccept(state);
        } else if (state.worker != null) {
            state.worker.attach();
        } else {
            startAccept(state);
        }
        return promise;
    }

    private static void startAccept(final Listener state) {
        final ServerSocketChannel channel = state.channel;
        state.worker = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                return channel.accept();
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                state.worker = null;
                state.ready.add(error != null ? error : value);
                if (state.accepting != null) {
                    serveAccept(state);
                } else if (state.closed) {
                    discard(state);
                }
            }
        });
    }

    private static String acceptSymbol(Listener state) {
        return state.family == 3 ? symbol("unix_accept", 1) : symbol("tcp_accept", 3);
    }

    private static void serveAccept(Listener state) {
        String operation = acceptSymbol(state);
        Future promise = state.accepting;
        Object next = state.ready.poll();
        if (next instanceof Throwable) {
            Scheduler.settle(promise, null, state.closed
                ? failure("already_closed", operation, "Socket is closed")
                : hostFailure(operation, state.family == 3 ? "Cannot accept Unix connection"
                    : "Cannot accept TCP connection", (Throwable) next));
            return;
        }
        SocketChannel client = (SocketChannel) next;
        if (state.family != 3) {
            try {
                client.setOption(StandardSocketOptions.SO_KEEPALIVE, state.keepAlive);
                client.setOption(StandardSocketOptions.TCP_NODELAY, state.noDelay);
            } catch (IOException error) {
                closeQuietly(client);
                Scheduler.settle(promise, null, hostFailure(operation, "Cannot apply TCP connection option", error));
                return;
            }
        }
        Stream stream = new Stream();
        stream.channel = client;
        stream.family = state.family;
        Scheduler.settle(promise, stream, null);
    }

    private static void discard(Listener state) {
        for (Object item : state.ready) {
            if (item instanceof SocketChannel) {
                closeQuietly((SocketChannel) item);
            }
        }
        state.ready.clear();
    }

    public static boolean runtime_network__tcp_listener_close(Object handle) {
        closeListener((Listener) handle);
        return true;
    }

    /** Закрывает listener; {@code false}, если уже был закрыт. */
    static boolean closeListener(Listener state) {
        if (state.closed) {
            return false;
        }
        state.closed = true;
        closeQuietly(state.channel);
        discard(state);
        if (state.accepting != null) {
            Scheduler.settle(state.accepting, null, failure("already_closed", acceptSymbol(state), "Socket is closed"));
        }
        return true;
    }

    public static boolean runtime_network__tcp_listener_closed(Object handle) {
        return ((Listener) handle).closed;
    }

    public static Future runtime_network__tcp_connect(List<?> address, String zone, final boolean keepAlive,
            final boolean noDelay) {
        final String operation = symbol("tcp_connect", 4);
        final Endpoint target;
        final InetSocketAddress remote;
        try {
            target = decode(address, operation);
            if (target.port <= 0) {
                throw failure("invalid_input", operation, "TCP destination port must be positive");
            }
            remote = socketAddress(target, zone, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                SocketChannel channel = SocketChannel.open();
                try {
                    channel.connect(remote);
                } catch (IOException error) {
                    closeQuietly(channel);
                    throw error;
                }
                try {
                    channel.setOption(StandardSocketOptions.SO_KEEPALIVE, keepAlive);
                    channel.setOption(StandardSocketOptions.TCP_NODELAY, noDelay);
                } catch (IOException error) {
                    closeQuietly(channel);
                    throw new IllegalStateException("options", error);
                }
                return channel;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Throwable cause = error instanceof IllegalStateException ? error.getCause() : error;
                    String message = error instanceof IllegalStateException ? "Cannot apply TCP connection option"
                        : "TCP connection failed";
                    Scheduler.settle(promise, null, hostFailure(operation, message, cause));
                    return;
                }
                SocketChannel channel = (SocketChannel) value;
                if (promise.state != Future.RUNNING) {
                    closeQuietly(channel);
                    return;
                }
                Stream stream = new Stream();
                stream.channel = channel;
                stream.family = target.family;
                Scheduler.settle(promise, stream, null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    // ---- TCP-поток ----

    /** Управляемый поток ({@code managed_stream} Perl): TCP или Unix. */
    static Stream stream(Object handle, String operation) {
        Sock socket = (Sock) handle;
        if (socket.closed) {
            throw failure("already_closed", operation, "Native handle is already closed");
        }
        if (!(socket instanceof Stream)) {
            throw failure("invalid_input", operation, "Operation requires a managed stream connection");
        }
        return (Stream) socket;
    }

    private static Stream tcpStream(Object handle, String operation) {
        Stream state = stream(handle, operation);
        if (state.family != IPV4 && state.family != IPV6) {
            throw failure("invalid_input", operation, "Operation requires a TCP stream");
        }
        return state;
    }

    public static List<Object> runtime_network__tcp_endpoint(Object handle, boolean peer) {
        String operation = symbol("tcp_endpoint", 2);
        Stream state = tcpStream(handle, operation);
        try {
            InetSocketAddress address = (InetSocketAddress) (peer
                ? state.channel.getRemoteAddress() : state.channel.getLocalAddress());
            if (address == null) {
                throw failure("not_connected", operation, "Cannot inspect TCP endpoint");
            }
            return endpointStrings(address, state.family);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot inspect TCP endpoint", error);
        }
    }

    public static Future runtime_network__stream_read(Object handle, BigInteger limit) {
        String operation = symbol("stream_read", 2);
        if (limit.signum() <= 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "Stream read limit must be positive"));
        }
        final Stream state;
        try {
            state = stream(handle, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (state.inputClosed) {
            return Scheduler.resolved(null, failure("already_closed", operation, "Stream input is closed"));
        }
        if (state.reading != null) {
            return Scheduler.resolved(null,
                failure("resource_exhausted", operation, state.label() + " already has a pending reading"));
        }
        final Future promise = Future.promise();
        state.reading = promise;
        state.readLimit = limit.compareTo(BigInteger.valueOf(CHUNK)) < 0 ? limit.intValue() : CHUNK;
        promise.listen(new Runnable() {
            public void run() {
                state.reading = null;
                if (state.reader != null) {
                    state.reader.detach();
                }
            }
        });
        if (state.pending != null || state.readError != null || state.eof) {
            serveRead(state);
        } else if (state.reader != null) {
            state.reader.attach();
        } else {
            startRead(state);
        }
        return promise;
    }

    private static void startRead(final Stream state) {
        final SocketChannel channel = state.channel;
        state.reader = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                ByteBuffer buffer = ByteBuffer.allocate(CHUNK);
                int count = channel.read(buffer);
                if (count < 0) {
                    return null;
                }
                byte[] bytes = new byte[count];
                buffer.flip();
                buffer.get(bytes);
                return bytes;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                state.reader = null;
                if (error != null) {
                    state.readError = error;
                } else if (value == null) {
                    state.eof = true;
                } else {
                    state.pending = (byte[]) value;
                }
                if (state.reading != null) {
                    serveRead(state);
                }
            }
        });
    }

    private static void serveRead(Stream state) {
        String operation = symbol("stream_read", 2);
        Future promise = state.reading;
        if (state.closed) {
            Scheduler.settle(promise, null, failure("already_closed", operation, "Socket is closed"));
            return;
        }
        if (state.pending != null) {
            byte[] bytes = state.pending;
            int count = Math.min(bytes.length, state.readLimit);
            if (count == bytes.length) {
                state.pending = null;
            } else {
                byte[] rest = new byte[bytes.length - count];
                System.arraycopy(bytes, count, rest, 0, rest.length);
                state.pending = rest;
            }
            Scheduler.settle(promise, Natives.fromBytes(bytes, count), null);
            return;
        }
        if (state.readError != null) {
            Throwable error = state.readError;
            state.readError = null;
            Scheduler.settle(promise, null, hostFailure(operation, "Cannot read managed stream", error));
            return;
        }
        if (state.eof) {
            Scheduler.settle(promise, new ArrayList<Object>(), null);
            return;
        }
        if (state.reader == null) {
            startRead(state);
        }
    }

    public static Future runtime_network__stream_write(final Object handle, List<?> payload) {
        final String operation = symbol("stream_write", 2);
        final Stream state;
        try {
            state = stream(handle, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (state.outputClosed) {
            return Scheduler.resolved(null, failure("already_closed", operation, "Stream output is closed"));
        }
        if (state.writing != null) {
            return Scheduler.resolved(null,
                failure("resource_exhausted", operation, state.label() + " already has a pending writing"));
        }
        final byte[] bytes = Natives.toBytes(payload);
        if (bytes.length == 0) {
            return Scheduler.resolved(Boolean.TRUE, null);
        }
        final Future promise = Future.promise();
        state.writing = promise;
        final SocketChannel channel = state.channel;
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                ByteBuffer buffer = ByteBuffer.wrap(bytes);
                while (buffer.hasRemaining()) {
                    channel.write(buffer);
                }
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error == null) {
                    Scheduler.settle(promise, Boolean.TRUE, null);
                } else if (state.closed) {
                    Scheduler.settle(promise, null, failure("already_closed", operation, "Socket is closed"));
                } else {
                    Scheduler.settle(promise, null, hostFailure(operation, "Cannot write managed stream", error));
                }
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                state.writing = null;
                op.detach();
                // Как у Perl: незавершённая запись (отказ или отмена) закрывает поток.
                if (promise.state != Future.DONE && !state.closed) {
                    closeStream(state);
                }
            }
        });
        return promise;
    }

    public static boolean runtime_network__stream_shutdown(Object handle, boolean output) {
        String operation = symbol("stream_shutdown", 2);
        if (((Sock) handle).closed) {
            return true;
        }
        Stream state = stream(handle, operation);
        if (output ? state.outputClosed : state.inputClosed) {
            return true;
        }
        if (output && state.writing != null) {
            throw failure("resource_exhausted", operation, "Output shutdown requires a drained writer");
        }
        try {
            if (output) {
                state.channel.shutdownOutput();
            } else {
                state.channel.shutdownInput();
            }
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot shut down stream half", error);
        }
        if (output) {
            state.outputClosed = true;
        } else {
            state.inputClosed = true;
            if (state.reading != null) {
                Scheduler.settle(state.reading, null,
                    failure("already_closed", symbol("stream_read", 2), "Stream input is closed"));
            }
        }
        return true;
    }

    public static boolean runtime_network__stream_half_closed(Object handle, boolean output) {
        if (((Sock) handle).closed) {
            return true;
        }
        Stream state = stream(handle, symbol("stream_half_closed", 2));
        return output ? state.outputClosed : state.inputClosed;
    }

    static void closeStream(Stream state) {
        state.closed = true;
        closeQuietly(state.channel);
        if (state.reading != null) {
            Scheduler.settle(state.reading, null,
                failure("already_closed", symbol("stream_read", 2), "Socket is closed"));
        }
        if (state.writing != null) {
            Scheduler.settle(state.writing, null,
                failure("already_closed", symbol("stream_write", 2), "Socket is closed"));
        }
    }

    public static boolean runtime_network__close(Object handle) {
        // Как таблица дескрипторов Perl: повторное закрытие — не ошибка.
        Sock state = (Sock) handle;
        if (!state.closed) {
            state.shut();
        }
        return true;
    }

    public static boolean runtime_network__closed(Object handle) {
        return ((Sock) handle).closed;
    }

    static void closeQuietly(java.io.Closeable channel) {
        try {
            channel.close();
        } catch (IOException ignored) {
            // Как у Perl: отказ закрытия уже закрытого ресурса не наблюдаем.
        }
    }

    // ---- UDP ----

    static final class Membership {
        java.nio.channels.MembershipKey key;
        Udp socket;
        byte[] group;
        boolean closed;
    }

    static final class Udp extends Sock {
        java.nio.channels.DatagramChannel channel;
        boolean dualStack;
        Future receiving;
        Future sending;
        InetSocketAddress peer;
        int peerMaximum;
        String multicastPolicy;
        final java.util.LinkedHashMap<String, Membership> memberships = new java.util.LinkedHashMap<String, Membership>();

        void shut() {
            closed = true;
            for (Membership member : memberships.values()) {
                member.closed = true;
            }
            memberships.clear();
            closeQuietly(channel);
            if (receiving != null) {
                Scheduler.settle(receiving, null, failure("already_closed", symbol("udp_receive", 2), "Socket is closed"));
            }
            if (sending != null) {
                Scheduler.settle(sending, null, failure("already_closed", symbol("udp_send", 4), "Socket is closed"));
            }
        }
    }

    private static Udp udp(Object handle, String operation) {
        Sock socket = (Sock) handle;
        if (socket.closed) {
            throw failure("already_closed", operation, "Native handle is already closed");
        }
        if (!(socket instanceof Udp)) {
            throw failure("invalid_input", operation, "Operation requires a datagram socket");
        }
        return (Udp) socket;
    }

    private static java.nio.channels.DatagramChannel datagram(int family) throws IOException {
        return java.nio.channels.DatagramChannel.open(family == IPV4 ? java.net.StandardProtocolFamily.INET
            : java.net.StandardProtocolFamily.INET6);
    }

    public static Object runtime_network__socket(BigInteger family, BigInteger type, BigInteger protocol) {
        String operation = symbol("socket", 3);
        if (!family.equals(BigInteger.ONE) && !family.equals(BigInteger.valueOf(2))) {
            throw failure("unsupported", operation, "IP address family is not supported");
        }
        int kind = type.bitLength() > 31 ? -1 : type.intValue();
        if (kind != 0 && kind != 1 && kind != 2) {
            throw failure("invalid_input", operation, "Unknown portable socket type");
        }
        int proto = protocol.bitLength() > 31 ? -1 : protocol.intValue();
        if (!(proto == 0 || (kind == 1 && proto == 1) || (kind == 2 && proto == 2))) {
            throw failure("invalid_input", operation, "Socket protocol does not match its type");
        }
        try {
            if (kind == 2) {
                Udp socket = new Udp();
                socket.family = family.intValue();
                socket.channel = datagram(socket.family);
                return socket;
            }
            Stream stream = new Stream();
            stream.family = family.intValue();
            stream.channel = SocketChannel.open();
            return stream;
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot create socket", error);
        }
    }

    public static Object runtime_network__udp_bound(List<?> address, String zone, boolean reuse, boolean broadcast,
            boolean dual) {
        String operation = symbol("udp_bound", 5);
        Endpoint endpoint = decode(address, operation);
        if (endpoint.family == IPV4 && dual) {
            throw failure("unsupported", operation, "Dual stack requires an IPv6 binding");
        }
        if (endpoint.family == IPV6 && broadcast) {
            throw failure("unsupported", operation, "IPv6 has no broadcast socket option");
        }
        InetSocketAddress local = socketAddress(endpoint, zone, operation);
        java.nio.channels.DatagramChannel channel;
        try {
            channel = datagram(endpoint.family);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot create UDP socket", error);
        }
        try {
            channel.setOption(StandardSocketOptions.SO_REUSEADDR, reuse);
            if (endpoint.family == IPV4) {
                channel.setOption(StandardSocketOptions.SO_BROADCAST, broadcast);
            }
        } catch (IOException error) {
            closeQuietly(channel);
            throw hostFailure(operation, "Cannot apply UDP binding option", error);
        }
        try {
            channel.bind(local);
        } catch (IOException error) {
            closeQuietly(channel);
            throw hostFailure(operation, "Cannot bind UDP socket", error);
        }
        Udp socket = new Udp();
        socket.family = endpoint.family;
        socket.channel = channel;
        socket.dualStack = dual;
        return socket;
    }

    public static List<Object> runtime_network__local_endpoint(Object handle) {
        String operation = symbol("local_endpoint", 1);
        Sock socket = (Sock) handle;
        if (socket.closed) {
            throw failure("already_closed", operation, "Native handle is already closed");
        }
        if (socket.family != IPV4 && socket.family != IPV6) {
            throw failure("invalid_input", operation, "Operation requires an IP socket");
        }
        try {
            java.net.SocketAddress local = socket instanceof Udp ? ((Udp) socket).channel.getLocalAddress()
                : ((Stream) socket).channel.getLocalAddress();
            if (local == null) {
                return endpointStrings(new InetSocketAddress(
                    socket.family == IPV4 ? InetAddress.getByAddress(new byte[4]) : InetAddress.getByAddress(new byte[16]), 0),
                    socket.family);
            }
            return endpointStrings((InetSocketAddress) local, socket.family);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot inspect local socket endpoint", error);
        }
    }

    private static boolean ipv4Mapped(Endpoint endpoint) {
        if (endpoint.family != IPV6) {
            return false;
        }
        for (int index = 0; index < 10; index++) {
            if (endpoint.address[index] != 0) {
                return false;
            }
        }
        return (endpoint.address[10] & 0xff) == 0xff && (endpoint.address[11] & 0xff) == 0xff;
    }

    private static InetSocketAddress destination(Udp socket, Endpoint endpoint, String zone, String operation) {
        boolean ipv4 = endpoint.family == IPV4 || ipv4Mapped(endpoint);
        if (socket.family != endpoint.family) {
            if (!(socket.family == IPV6 && socket.dualStack)) {
                throw failure("unsupported", operation, "UDP socket and destination families do not match");
            }
            if (!zone.isEmpty()) {
                throw failure("invalid_input", operation, "IPv4 destination cannot have a zone");
            }
            Endpoint mapped = new Endpoint();
            mapped.family = IPV6;
            mapped.port = endpoint.port;
            mapped.address = new byte[16];
            mapped.address[10] = (byte) 0xff;
            mapped.address[11] = (byte) 0xff;
            System.arraycopy(endpoint.address, 0, mapped.address, 12, 4);
            endpoint = mapped;
        }
        if (socket.family == IPV6 && ipv4 && !socket.dualStack) {
            throw failure("unsupported", operation, "IPv6-only socket cannot send to IPv4");
        }
        return socketAddress(endpoint, zone, operation);
    }

    private static byte[] endpointBytes(InetSocketAddress address) {
        byte[] raw = address.getAddress().getAddress();
        int family = raw.length == 4 ? IPV4 : IPV6;
        return Natives.toBytes(encode(family, address.getPort(), raw));
    }

    /** Конверт принятого пакета v3: 'O' 'K' 3 усечён scope, затем конверт v2. */
    private static List<Object> receivedPacket(InetSocketAddress source, byte[] payload, int count, int limit) {
        byte[] address = endpointBytes(source);
        int scope = source.getAddress() instanceof Inet6Address ? ((Inet6Address) source.getAddress()).getScopeId() : 0;
        boolean truncated = count > limit;
        int size = Math.min(count, limit);
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        out.write(79);
        out.write(75);
        out.write(3);
        out.write(truncated ? 1 : 0);
        out.write(scope >>> 24);
        out.write(scope >>> 16);
        out.write(scope >>> 8);
        out.write(scope);
        out.write(79);
        out.write(75);
        out.write(2);
        out.write(address.length >> 8);
        out.write(address.length);
        out.write(address, 0, address.length);
        out.write(payload, 0, size);
        byte[] bytes = out.toByteArray();
        return Natives.fromBytes(bytes, bytes.length);
    }

    private static Future receive(final Udp socket, BigInteger limitValue, String operation) {
        if (socket.receiving != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation, "UDP socket already has a pending receive"));
        }
        final int limit = limitValue.compareTo(BigInteger.valueOf(65536)) > 0 ? 65536 : limitValue.intValue();
        final Future promise = Future.promise();
        socket.receiving = promise;
        final java.nio.channels.DatagramChannel channel = socket.channel;
        final String name = operation;
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                ByteBuffer buffer = ByteBuffer.allocate(65536);
                java.net.SocketAddress source = channel.receive(buffer);
                return new Object[] {source, buffer};
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Scheduler.settle(promise, null, socket.closed
                        ? failure("already_closed", name, "Socket is closed")
                        : hostFailure(name, "Cannot receive UDP datagram", error));
                    return;
                }
                Object[] result = (Object[]) value;
                ByteBuffer buffer = (ByteBuffer) result[1];
                buffer.flip();
                byte[] payload = new byte[buffer.remaining()];
                buffer.get(payload);
                Scheduler.settle(promise, receivedPacket((InetSocketAddress) result[0], payload, payload.length, limit), null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                socket.receiving = null;
                op.detach();
            }
        });
        return promise;
    }

    public static Future runtime_network__udp_receive(Object handle, BigInteger limit) {
        String operation = symbol("udp_receive", 2);
        if (limit.signum() <= 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "UDP receive limit must be positive"));
        }
        try {
            Udp socket = udp(handle, operation);
            if (socket.peer != null) {
                throw failure("invalid_input", operation, "Unconnected receive requires an unconnected UDP socket");
            }
            return receive(socket, limit, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_network__udp_connected_receive(Object handle, BigInteger limit) {
        String operation = symbol("udp_connected_receive", 2);
        if (limit.signum() <= 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "UDP receive limit must be positive"));
        }
        try {
            Udp socket = udp(handle, operation);
            if (socket.peer == null) {
                throw failure("not_connected", operation, "UDP socket is not connected");
            }
            return receive(socket, limit, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    private static Future send(final Udp socket, final byte[] payload, final InetSocketAddress target, final String operation) {
        if (socket.sending != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation, "UDP socket already has a pending send"));
        }
        final Future promise = Future.promise();
        socket.sending = promise;
        final java.nio.channels.DatagramChannel channel = socket.channel;
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                ByteBuffer buffer = ByteBuffer.wrap(payload);
                return target == null ? channel.write(buffer) : channel.send(buffer, target);
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    String text = String.valueOf(error.getMessage()).toLowerCase();
                    if (text.contains("message too long")) {
                        Scheduler.settle(promise, null, failure("message_too_large", operation,
                            "UDP payload exceeds host or path limit"));
                    } else {
                        Scheduler.settle(promise, null, socket.closed ? failure("already_closed", operation, "Socket is closed")
                            : hostFailure(operation, "Cannot send UDP datagram", error));
                    }
                    return;
                }
                int count = (Integer) value;
                Scheduler.settle(promise, count == payload.length ? BigInteger.valueOf(count) : null,
                    count == payload.length ? null
                        : failure("unexpected", operation, "Host reported an incomplete UDP datagram send"));
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                socket.sending = null;
                op.detach();
            }
        });
        return promise;
    }

    public static Future runtime_network__udp_send(Object handle, List<?> address, String zone, List<?> payload) {
        String operation = symbol("udp_send", 4);
        try {
            Endpoint endpoint = decode(address, operation);
            if (endpoint.port == 0) {
                throw failure("invalid_input", operation, "UDP destination port must be positive");
            }
            Udp socket = udp(handle, operation);
            if (socket.peer != null) {
                throw failure("invalid_input", operation, "Unconnected send requires an unconnected UDP socket");
            }
            boolean ipv4 = endpoint.family == IPV4 || ipv4Mapped(endpoint);
            if (payload.size() > (ipv4 ? 65507 : 65527)) {
                throw failure("message_too_large", operation, "UDP payload exceeds IP version limit");
            }
            InetSocketAddress target = destination(socket, endpoint, zone, operation);
            return send(socket, Natives.toBytes(payload), target, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_network__udp_connected_send(Object handle, List<?> payload) {
        String operation = symbol("udp_connected_send", 2);
        try {
            Udp socket = udp(handle, operation);
            if (socket.peer == null) {
                throw failure("not_connected", operation, "UDP socket is not connected");
            }
            if (payload.size() > socket.peerMaximum) {
                throw failure("message_too_large", operation, "UDP payload exceeds IP version limit");
            }
            return send(socket, Natives.toBytes(payload), null, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_network__udp_connect(Object handle, List<?> address, String zone) {
        String operation = symbol("udp_connect", 3);
        try {
            Udp socket = udp(handle, operation);
            if (socket.peer != null || socket.sending != null || socket.receiving != null || socket.multicastPolicy != null) {
                throw failure("invalid_input", operation, "Connect requires an idle unconnected UDP socket");
            }
            Endpoint endpoint = decode(address, operation);
            if (endpoint.port == 0) {
                throw failure("invalid_input", operation, "UDP destination port must be positive");
            }
            InetSocketAddress target = destination(socket, endpoint, zone, operation);
            try {
                socket.channel.connect(target);
                socket.peer = (InetSocketAddress) socket.channel.getRemoteAddress();
            } catch (IOException error) {
                socket.shut();
                throw hostFailure(operation, "Cannot connect UDP socket", error);
            }
            Endpoint decoded = decode(encode(socket.peer.getAddress().getAddress().length == 4 ? IPV4 : IPV6,
                socket.peer.getPort(), socket.peer.getAddress().getAddress()), operation);
            socket.peerMaximum = decoded.family == IPV4 || ipv4Mapped(decoded) ? 65507 : 65527;
            return Scheduler.resolved(Boolean.TRUE, null);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static List<Object> runtime_network__peer_endpoint(Object handle) {
        String operation = symbol("peer_endpoint", 1);
        Udp socket = udp(handle, operation);
        if (socket.peer == null) {
            throw failure("not_connected", operation, "UDP socket is not connected");
        }
        return endpointStrings(socket.peer, socket.peer.getAddress().getAddress().length == 4 ? IPV4 : IPV6);
    }

    /** Разбор конверта v3: {усечён, scope, адрес-конверт, данные}. */
    private static Object[] packet(List<?> value, String operation) {
        byte[] bytes = Natives.toBytes(value);
        if (bytes.length < 8 || bytes[0] != 79 || bytes[1] != 75 || bytes[2] != 3 || (bytes[3] & 0xff) > 1) {
            throw failure("invalid_data", operation, "Invalid received packet envelope");
        }
        int scope = ((bytes[4] & 0xff) << 24) | ((bytes[5] & 0xff) << 16) | ((bytes[6] & 0xff) << 8) | (bytes[7] & 0xff);
        int at = 8;
        if (bytes.length - at < 5 || bytes[at] != 79 || bytes[at + 1] != 75 || bytes[at + 2] != 2) {
            throw failure("invalid_data", operation, "Invalid datagram envelope");
        }
        int length = ((bytes[at + 3] & 0xff) << 8) | (bytes[at + 4] & 0xff);
        if (length == 0 || bytes.length - at < 5 + length) {
            throw failure("invalid_data", operation, "Invalid datagram envelope");
        }
        byte[] address = new byte[length];
        System.arraycopy(bytes, at + 5, address, 0, length);
        decode(Natives.fromBytes(address, length), operation);
        byte[] payload = new byte[bytes.length - at - 5 - length];
        System.arraycopy(bytes, at + 5 + length, payload, 0, payload.length);
        return new Object[] {bytes[3] == 1, scope, address, payload};
    }

    public static List<Object> runtime_network__packet_bytes(List<?> value) {
        byte[] payload = (byte[]) packet(value, symbol("packet_bytes", 1))[3];
        return Natives.fromBytes(payload, payload.length);
    }

    public static boolean runtime_network__packet_truncated(List<?> value) {
        return (Boolean) packet(value, symbol("packet_truncated", 1))[0];
    }

    public static List<Object> runtime_network__packet_source(List<?> value) {
        String operation = symbol("packet_source", 1);
        Object[] parsed = packet(value, operation);
        byte[] address = (byte[]) parsed[2];
        Endpoint endpoint = decode(Natives.fromBytes(address, address.length), operation);
        int scope = (Integer) parsed[1];
        ArrayList<Object> out = new ArrayList<Object>();
        out.add(format(endpoint.address));
        out.add(Integer.toString(endpoint.port));
        out.add(scope != 0 ? Integer.toString(scope) : "");
        return out;
    }

    // ---- multicast ----

    public static Object runtime_network__multicast_join(Object handle, List<?> address, String name, BigInteger index,
            BigInteger hops, boolean loopback) {
        String operation = symbol("multicast_join", 6);
        Udp socket = udp(handle, operation);
        Endpoint group = decode(address, operation);
        if (socket.peer != null || socket.sending != null) {
            throw failure("invalid_input", operation, "Multicast requires an unconnected UDP socket without a pending send");
        }
        if (hops.signum() < 0 || hops.compareTo(BigInteger.valueOf(255)) > 0) {
            throw failure("invalid_input", operation, "Multicast hop limit must be in 0..255");
        }
        int first = group.address[0] & 0xff;
        if (!(group.port == 0 && (group.family == IPV4 ? (first & 0xf0) == 0xe0 : first == 0xff))) {
            throw failure("invalid_input", operation, "Address is not a multicast group");
        }
        if (group.family != socket.family) {
            throw failure("unsupported", operation, "Multicast group must match socket IP family");
        }
        String policy = index + ":" + hops + ":" + (loopback ? 1 : 0);
        if (socket.multicastPolicy != null && !socket.multicastPolicy.equals(policy)) {
            throw failure("invalid_input", operation, "Multicast interface and options conflict with the socket policy");
        }
        String key = format(group.address);
        if (socket.memberships.containsKey(key)) {
            throw failure("already_exists", operation, "Multicast group is already joined on this socket");
        }
        if (index.signum() <= 0 || index.compareTo(BigInteger.valueOf(4294967295L)) > 0
                || !name.matches("[A-Za-z0-9._~-]+") || name.matches("[0-9]+")) {
            throw failure("invalid_input", operation, "Invalid multicast interface identity");
        }
        NetworkInterface face;
        try {
            face = NetworkInterface.getByName(name);
        } catch (SocketException error) {
            face = null;
        }
        if (face == null || face.getIndex() != index.longValue()) {
            throw failure("invalid_input", operation, "Multicast interface name/index no longer identifies the same interface");
        }
        try {
            if (socket.multicastPolicy == null) {
                socket.channel.setOption(StandardSocketOptions.IP_MULTICAST_IF, face);
                socket.channel.setOption(StandardSocketOptions.IP_MULTICAST_TTL, hops.intValue());
                socket.channel.setOption(StandardSocketOptions.IP_MULTICAST_LOOP, loopback);
            }
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot configure multicast socket", error);
        }
        Membership member = new Membership();
        try {
            member.key = socket.channel.join(InetAddress.getByAddress(group.address), face);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot join multicast group", error);
        }
        socket.multicastPolicy = policy;
        member.socket = socket;
        member.group = group.address;
        socket.memberships.put(key, member);
        return member;
    }

    public static boolean runtime_network__multicast_closed(Object handle) {
        return ((Membership) handle).closed;
    }

    public static boolean runtime_network__multicast_close(Object handle) {
        Membership member = (Membership) handle;
        if (member.closed) {
            return true;
        }
        member.closed = true;
        if (member.socket != null && !member.socket.closed) {
            member.socket.memberships.remove(format(member.group));
            member.key.drop();
        }
        return true;
    }

    // ---- DNS ----

    public static Future runtime_network__dns_addresses(final String host, final BigInteger portValue) {
        final String operation = symbol("dns_addresses", 2);
        if (!host.matches("[A-Za-z0-9.-]{1,254}")) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "Invalid system resolver hostname"));
        }
        if (portValue.signum() < 0 || portValue.compareTo(BigInteger.valueOf(65535)) > 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "Invalid system resolver port"));
        }
        final int port = portValue.intValue();
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws UnknownHostException {
                return InetAddress.getAllByName(host);
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Scheduler.settle(promise, null, failure("not_found", operation, "Network name was not found"));
                    return;
                }
                InetAddress[] records = (InetAddress[]) value;
                if (records.length > 256) {
                    Scheduler.settle(promise, null, failure("resource_exhausted", operation,
                        "System resolution answer exceeds 256 records"));
                    return;
                }
                ArrayList<Object> out = new ArrayList<Object>();
                java.util.HashSet<String> seen = new java.util.HashSet<String>();
                for (InetAddress record : records) {
                    List<Object> endpoint = endpointStrings(new InetSocketAddress(record, port),
                        record instanceof Inet4Address ? IPV4 : IPV6);
                    if (seen.add(endpoint.toString())) {
                        out.add(endpoint);
                    }
                }
                if (out.isEmpty()) {
                    Scheduler.settle(promise, null, failure("not_found", operation,
                        "System name resolution returned no usable address"));
                    return;
                }
                Scheduler.settle(promise, out, null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    public static Future runtime_network__dns_name(final String text) {
        final String operation = symbol("dns_name", 1);
        if (!text.matches("[0-9A-Fa-f:.]{1,45}")) {
            return Scheduler.resolved(null, failure("invalid_input", operation,
                "Reverse lookup requires a bounded numeric IP address"));
        }
        byte[] parsed = parseIpv4(text);
        if (parsed == null && text.indexOf('.') < 0) {
            parsed = parseIpv6(text);
        } else if (parsed == null && text.indexOf(':') >= 0) {
            String dotted = text.substring(text.lastIndexOf(':') + 1);
            parsed = parseIpv4(dotted) == null ? null : parseIpv6(text);
        }
        if (parsed == null) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "Reverse lookup requires a numeric IP address"));
        }
        final byte[] address = parsed;
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws UnknownHostException {
                InetAddress host = InetAddress.getByAddress(address);
                String name = host.getCanonicalHostName();
                return name.equals(host.getHostAddress()) ? null : name;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null || value == null) {
                    Scheduler.settle(promise, null, failure("not_found", operation, "Network name was not found"));
                    return;
                }
                Scheduler.settle(promise, value, null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }
}
