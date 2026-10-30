package okkama.rt;

/**
 * Системный объект Okkama {@code NativeFailure} (OIR-1 §9, §14.1): ожидаемый
 * сбой provider. Значения создаёт только рантайм ({@link #raise}); поля
 * читают actions программы ({@code domain()}, {@code code()}, …), а
 * {@code text()} — {@code message}, как в Perl-интерпретаторе.
 */
public final class NativeFailure extends Obj implements Error {
    public final String domain;
    public final String code;
    public final String operation;
    public final String message;

    @Override
    public String okkamaName() {
        return "NativeFailure";
    }

    @Override
    public void okkamaFields(java.util.List<Object> out) {
        out.add("domain");
        out.add(domain);
        out.add("code");
        out.add(code);
        out.add("operation");
        out.add(operation);
        out.add("message");
        out.add(message);
    }

    public NativeFailure(String domain, String code, String operation, String message) {
        this.domain = domain;
        this.code = code;
        this.operation = operation;
        this.message = message;
    }

    /** Типизированный отказ native {@code operation} — канонический символ ABI. */
    public static Failure raise(String domain, String code, String operation, String message) {
        return Rt.thrown(new NativeFailure(domain, code, operation, message));
    }

    @Override
    public String text() {
        return message;
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof NativeFailure)) {
            return false;
        }
        NativeFailure that = (NativeFailure) other;
        return domain.equals(that.domain) && code.equals(that.code)
            && operation.equals(that.operation) && message.equals(that.message);
    }

    @Override
    public int hashCode() {
        return ((domain.hashCode() * 31 + code.hashCode()) * 31 + operation.hashCode()) * 31
            + message.hashCode();
    }
}
