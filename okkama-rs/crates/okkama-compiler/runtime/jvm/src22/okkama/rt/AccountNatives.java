package okkama.rt;

import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandle;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

/**
 * Natives {@code runtime/account}: базы пользователей и групп хоста
 * ({@code getpwuid}/{@code getpwnam}/{@code getgrnam}) через FFM.
 */
public final class AccountNatives {
    private AccountNatives() {}

    private static final String DOMAIN = "core.account";

    private static final class Api {
        static final MethodHandle GETEUID = Posix.function("geteuid", FunctionDescriptor.of(Posix.INT));
        static final MethodHandle GETPWUID = Posix.function("getpwuid", FunctionDescriptor.of(Posix.PTR, Posix.INT));
        static final MethodHandle GETPWNAM = Posix.function("getpwnam", FunctionDescriptor.of(Posix.PTR, Posix.PTR));
        static final MethodHandle GETGRNAM = Posix.function("getgrnam", FunctionDescriptor.of(Posix.PTR, Posix.PTR));
        static final MethodHandle SETPWENT = Posix.function("setpwent", FunctionDescriptor.ofVoid());
        static final MethodHandle GETPWENT = Posix.function("getpwent", FunctionDescriptor.of(Posix.PTR));
        static final MethodHandle ENDPWENT = Posix.function("endpwent", FunctionDescriptor.ofVoid());
    }

    /** Смещение {@code pw_dir} в {@code struct passwd}. */
    private static final long DIR = Posix.MAC ? 48 : 32;

    private static String symbol(String name, int arity) {
        return "runtime/account@1::" + name + "/" + arity;
    }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    private static MemorySegment record(MemorySegment pointer) {
        return pointer.address() == 0 ? null : pointer.reinterpret(128);
    }

    private static Map<Object, Object> user(MemorySegment pointer, String operation) {
        MemorySegment entry = record(pointer);
        if (entry == null) {
            throw failure("not_found", operation, "User was not found");
        }
        String name = Posix.string(entry.get(Posix.PTR, 0));
        if (name == null || name.isEmpty()) {
            throw failure("invalid_data", operation, "Account provider returned an invalid user");
        }
        String home = Posix.string(entry.get(Posix.PTR, DIR));
        Map<Object, Object> out = Rt.newMap();
        out.put("home", home == null ? "" : home);
        out.put("identity", Integer.toUnsignedString(entry.get(Posix.INT, 16)));
        out.put("name", name);
        return out;
    }

    private static String name(String value, String kind, String operation) {
        if (value.isEmpty()) {
            throw failure("invalid_input", operation, kind + " name must not be empty");
        }
        if (value.indexOf('\0') >= 0) {
            throw failure("invalid_input", operation, kind + " name contains a zero byte");
        }
        return value;
    }

    private static Failure unexpected(String operation, Throwable error) {
        if (!Posix.SUPPORTED) {
            return failure("unsupported", operation, "Account lookup is supported on macOS and Linux only");
        }
        return error instanceof Failure ? (Failure) error
            : failure("unsupported", operation, "Account provider is unavailable");
    }

    public static Map<Object, Object> runtime_account__account_current_user() {
        String operation = symbol("account_current_user", 0);
        try {
            int uid = (int) Api.GETEUID.invokeExact();
            return user((MemorySegment) Api.GETPWUID.invokeExact(uid), operation);
        } catch (Throwable error) {
            throw unexpected(operation, error);
        }
    }

    public static Map<Object, Object> runtime_account__account_user_named(String value) {
        String operation = symbol("account_user_named", 1);
        String name = name(value, "User", operation);
        try (java.lang.foreign.Arena arena = java.lang.foreign.Arena.ofConfined()) {
            return user((MemorySegment) Api.GETPWNAM.invokeExact(arena.allocateFrom(name)), operation);
        } catch (Throwable error) {
            throw unexpected(operation, error);
        }
    }

    private static MemorySegment group(String name, String operation) throws Throwable {
        try (java.lang.foreign.Arena arena = java.lang.foreign.Arena.ofConfined()) {
            MemorySegment entry = record((MemorySegment) Api.GETGRNAM.invokeExact(arena.allocateFrom(name)));
            if (entry == null) {
                throw failure("not_found", operation, "Group was not found");
            }
            return entry;
        }
    }

    public static Map<Object, Object> runtime_account__account_group_named(String value) {
        String operation = symbol("account_group_named", 1);
        String name = name(value, "Group", operation);
        try {
            MemorySegment entry = group(name, operation);
            String found = Posix.string(entry.get(Posix.PTR, 0));
            if (found == null || found.isEmpty()) {
                throw failure("invalid_data", operation, "Account provider returned an invalid group");
            }
            Map<Object, Object> out = Rt.newMap();
            out.put("identity", Integer.toUnsignedString(entry.get(Posix.INT, 16)));
            out.put("name", found);
            return out;
        } catch (Throwable error) {
            throw unexpected(operation, error);
        }
    }

    public static List<Object> runtime_account__account_group_users(String value) {
        String operation = symbol("account_group_users", 1);
        String name = name(value, "Group", operation);
        try {
            MemorySegment entry = group(name, operation);
            int gid = entry.get(Posix.INT, 16);
            TreeSet<String> names = new TreeSet<String>(KeyOrder::compareCodePoints);
            MemorySegment members = entry.get(Posix.PTR, 24);
            if (members.address() != 0) {
                MemorySegment list = members.reinterpret(Long.MAX_VALUE);
                for (long at = 0; ; at += 8) {
                    String member = Posix.string(list.get(Posix.PTR, at));
                    if (member == null) {
                        break;
                    }
                    if (!member.isEmpty()) {
                        names.add(member);
                    }
                }
            }
            Api.SETPWENT.invokeExact();
            try {
                while (true) {
                    MemorySegment user = record((MemorySegment) Api.GETPWENT.invokeExact());
                    if (user == null) {
                        break;
                    }
                    String login = Posix.string(user.get(Posix.PTR, 0));
                    if (login != null && user.get(Posix.INT, 20) == gid) {
                        names.add(login);
                    }
                }
            } finally {
                Api.ENDPWENT.invokeExact();
            }
            return new ArrayList<Object>(names);
        } catch (Throwable error) {
            throw unexpected(operation, error);
        }
    }
}
