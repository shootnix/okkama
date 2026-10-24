package okkama.rt;

/**
 * Значение enum Okkama: синглтон case с номером в объявлении. Экземпляры
 * создаёт класс enum программы ({@code Main$Light.red}); равенство —
 * идентичность, порядок ключей — номер case (решение R06).
 */
public final class Case {
    public final int ordinal;
    public final String name;
    /** Имя enum без модуля: отладочный вывод {@code Type.case}. */
    public final String type;

    public Case(int ordinal, String name, String type) {
        this.ordinal = ordinal;
        this.name = name;
        this.type = type;
    }

    @Override
    public String toString() {
        return name;
    }
}
