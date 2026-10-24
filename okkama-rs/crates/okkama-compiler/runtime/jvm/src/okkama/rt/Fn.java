package okkama.rt;

/**
 * Значение-функция Okkama (OIR-1 §11): замыкание или связанный action.
 * Класс каждой функции генерирует backend; аргументы и результат — в
 * коллекционном представлении ({@code bool} — {@code Boolean}, …).
 */
public interface Fn {
    Object call(Object[] args);

    /** Захваченные значения: через них регион видит владеемые объекты. */
    Object[] captures();
}
