package okkama.rt;

import java.io.IOException;
import java.math.BigInteger;
import java.net.StandardProtocolFamily;
import java.net.UnixDomainSocketAddress;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.List;
import java.util.concurrent.Callable;

/**
 * Natives Unix-сокетов {@code runtime/network} (JDK 16+ {@code AF_UNIX}).
 * Пара сокетов эмулируется через временный listener: у JVM нет
 * {@code socketpair}; концы такой пары помечены безымянными.
 */
public final class UnixNatives {
    private UnixNatives() {}

    private static final int UNIX = 3;

    /** {@code PairState}: два конца, пока владелец их не забрал. */
    static final class Pair {
        NetNatives.Stream[] sockets;
        boolean released;
        boolean closed;
    }

    private static String symbol(String name, int arity) {
        return NetNatives.symbol(name, arity);
    }

    private static Failure failure(String code, String operation, String message) {
        return NetNatives.failure(code, operation, message);
    }

    /** Предел длины {@code sun_path} без завершающего NUL. */
    private static int pathLimit() {
        String os = System.getProperty("os.name", "").toLowerCase();
        return os.contains("linux") ? 107 : 103;
    }

    private static byte[] checked(byte[] path, String operation) {
        if (path.length == 0) {
            throw failure("invalid_input", operation, "Unix socket path must be nonempty and contain no NUL");
        }
        for (byte octet : path) {
            if (octet == 0) {
                throw failure("invalid_input", operation, "Unix socket path must be nonempty and contain no NUL");
            }
        }
        if (path.length > pathLimit()) {
            throw failure("invalid_input", operation, "Unix socket path exceeds " + pathLimit() + " bytes");
        }
        return path;
    }

    /** Абсолютный путь от виртуального cwd с проверкой длины. */
    private static Path absolute(List<?> value, String operation) {
        String text = new String(checked(Natives.toBytes(value), operation), StandardCharsets.UTF_8);
        Path path = Host.path(text).normalize();
        checked(path.toString().getBytes(StandardCharsets.UTF_8), operation);
        return path;
    }

    private static NetNatives.Stream stream(SocketChannel channel, boolean unnamed) {
        NetNatives.Stream stream = new NetNatives.Stream();
        stream.channel = channel;
        stream.family = UNIX;
        stream.unnamed = unnamed;
        return stream;
    }

    public static boolean runtime_network__unix_path_valid(List<?> path) {
        checked(Natives.toBytes(path), symbol("unix_path_valid", 1));
        return true;
    }

    public static Object runtime_network__unix_pair() {
        String operation = symbol("unix_pair", 0);
        Path directory = null;
        Path path = null;
        ServerSocketChannel server = null;
        SocketChannel left = null;
        try {
            directory = Files.createTempDirectory("okkama-pair");
            path = directory.resolve("s");
            server = ServerSocketChannel.open(StandardProtocolFamily.UNIX);
            server.bind(UnixDomainSocketAddress.of(path), 1);
            left = SocketChannel.open(StandardProtocolFamily.UNIX);
            left.connect(UnixDomainSocketAddress.of(path));
            SocketChannel right = server.accept();
            Pair pair = new Pair();
            pair.sockets = new NetNatives.Stream[] {stream(left, true), stream(right, true)};
            left = null;
            return pair;
        } catch (IOException error) {
            NetNatives.closeQuietly(left);
            throw NetNatives.hostFailure(operation, "Cannot create Unix socket pair", error);
        } finally {
            NetNatives.closeQuietly(server);
            try {
                if (path != null) {
                    Files.deleteIfExists(path);
                }
                if (directory != null) {
                    Files.deleteIfExists(directory);
                }
            } catch (IOException ignored) {
                // временный каталог останется в tmp
            }
        }
    }

    private static Pair pair(Object handle, String operation) {
        Pair pair = (Pair) handle;
        if (pair.closed || pair.released) {
            throw failure("already_closed", operation, "Native handle is already closed");
        }
        return pair;
    }

    public static Object runtime_network__unix_pair_socket(Object handle, boolean second) {
        return pair(handle, symbol("unix_pair_socket", 2)).sockets[second ? 1 : 0];
    }

    public static boolean runtime_network__unix_pair_release(Object handle) {
        pair(handle, symbol("unix_pair_release", 1)).released = true;
        return true;
    }

    public static boolean runtime_network__unix_pair_close(Object handle) {
        Pair pair = (Pair) handle;
        if (pair.closed || pair.released) {
            return true;
        }
        pair.closed = true;
        for (NetNatives.Stream socket : pair.sockets) {
            socket.shut();
        }
        return true;
    }

    public static List<Object> runtime_network__unix_endpoint(Object handle, boolean peer) {
        String operation = symbol("unix_endpoint", 2);
        NetNatives.Stream state = NetNatives.stream(handle, operation);
        if (state.family != UNIX) {
            throw failure("invalid_input", operation, "Operation requires a Unix stream connection");
        }
        if (state.unnamed) {
            return Natives.fromBytes(new byte[0], 0);
        }
        try {
            Object address = peer ? state.channel.getRemoteAddress() : state.channel.getLocalAddress();
            String path = address == null ? "" : ((UnixDomainSocketAddress) address).getPath().toString();
            return Host.bytes(path);
        } catch (IOException error) {
            throw NetNatives.hostFailure(operation, "Cannot inspect Unix endpoint", error);
        }
    }

    public static Future runtime_network__unix_connect(List<?> target) {
        final String operation = symbol("unix_connect", 1);
        final Path path;
        try {
            path = absolute(target, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                SocketChannel channel = SocketChannel.open(StandardProtocolFamily.UNIX);
                try {
                    channel.connect(UnixDomainSocketAddress.of(path));
                } catch (IOException error) {
                    NetNatives.closeQuietly(channel);
                    throw error;
                }
                return channel;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Scheduler.settle(promise, null, NetNatives.hostFailure(operation, "Cannot connect Unix socket", error));
                    return;
                }
                SocketChannel channel = (SocketChannel) value;
                if (promise.state != Future.RUNNING) {
                    NetNatives.closeQuietly(channel);
                    return;
                }
                Scheduler.settle(promise, stream(channel, false), null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    private static Object identity(Path path) throws IOException {
        BasicFileAttributes attributes = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        if (!attributes.isOther()) {
            return null;
        }
        Object key = attributes.fileKey();
        return key != null ? key : Files.getAttribute(path, "unix:ino", LinkOption.NOFOLLOW_LINKS);
    }

    public static Object runtime_network__unix_listen(List<?> target, BigInteger backlog) {
        String operation = symbol("unix_listen", 2);
        if (backlog.signum() < 0 || backlog.compareTo(BigInteger.valueOf(Integer.MAX_VALUE)) > 0) {
            throw failure("invalid_input", operation, "Unix backlog must be nonnegative and fit a host int");
        }
        Path path = absolute(target, operation);
        ServerSocketChannel channel;
        try {
            channel = ServerSocketChannel.open(StandardProtocolFamily.UNIX);
        } catch (IOException error) {
            throw NetNatives.hostFailure(operation, "Cannot create Unix stream socket", error);
        }
        try {
            channel.bind(UnixDomainSocketAddress.of(path), backlog.intValue());
        } catch (IOException error) {
            NetNatives.closeQuietly(channel);
            throw NetNatives.hostFailure(operation, "Cannot bind Unix listener", error);
        }
        Object identity;
        try {
            identity = identity(path);
        } catch (IOException error) {
            identity = null;
        }
        if (identity == null) {
            NetNatives.closeQuietly(channel);
            throw failure("unexpected", operation, "Cannot capture Unix listener entry identity");
        }
        NetNatives.Listener state = new NetNatives.Listener();
        state.channel = channel;
        state.family = UNIX;
        state.listening = true;
        state.path = path.toString();
        state.identity = identity;
        return state;
    }

    public static List<Object> runtime_network__unix_listener_path(Object handle) {
        NetNatives.Listener state = NetNatives.listener(handle, symbol("unix_listener_path", 1));
        return Host.bytes(state.path);
    }

    public static Future runtime_network__unix_accept(Object handle) {
        try {
            return NetNatives.accept(NetNatives.listener(handle, symbol("unix_accept", 1)), false, false);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static boolean runtime_network__unix_listener_close(Object handle) {
        NetNatives.Listener state = (NetNatives.Listener) handle;
        if (!NetNatives.closeListener(state)) {
            return true;
        }
        // Удаляем запись, только если это всё ещё наш сокет.
        Path path = Host.path(state.path);
        try {
            if (state.identity.equals(identity(path))) {
                Files.delete(path);
            }
        } catch (java.nio.file.NoSuchFileException missing) {
            return true;
        } catch (IOException error) {
            throw NetNatives.hostFailure(symbol("unix_listener_close", 1), "Cannot remove Unix listener entry", error);
        }
        return true;
    }

    public static boolean runtime_network__unix_listener_closed(Object handle) {
        return ((NetNatives.Listener) handle).closed;
    }
}
