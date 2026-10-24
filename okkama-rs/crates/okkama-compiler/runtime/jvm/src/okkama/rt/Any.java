package okkama.rt;

/**
 * Значение {@code any} (OIR-1 §3): непрозрачный box. Программа его не
 * распаковывает; сравнение доходит до box только через поле объекта и
 * всегда — программная ошибка, как в Perl-интерпретаторе.
 */
public final class Any {
    public final Object value;

    public Any(Object value) {
        this.value = value;
    }

    @Override
    public boolean equals(Object other) {
        throw Rt.trap("Cannot compare any without narrowing");
    }

    @Override
    public int hashCode() {
        throw Rt.trap("Cannot compare any without narrowing");
    }
}
