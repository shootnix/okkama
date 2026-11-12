package okkama.rt;

import java.math.BigInteger;
import java.util.List;

/**
 * runtime/checksum (#193), as okkama-runtime-core's native/compression.rs:
 * CRC-32/ISO-HDLC continued from a finalized CRC, the CRC-16/CCITT register
 * (polynomial 0x1021, MSB first, no reflection, no final xor), and Adler-32
 * packed sums with both halves of the seed reduced modulo 65521.
 * java.util.zip's CRC32 and Adler32 cannot start from a seed, so the loops are
 * here.
 */
public final class ChecksumNatives {
    private ChecksumNatives() {}

    private static final int[] CRC32_TABLE = crc32Table();

    private static int[] crc32Table() {
        int[] table = new int[256];
        for (int index = 0; index < 256; index++) {
            int value = index;
            for (int bit = 0; bit < 8; bit++) {
                value = (value & 1) != 0 ? (value >>> 1) ^ 0xEDB88320 : value >>> 1;
            }
            table[index] = value;
        }
        return table;
    }

    private static long seed(BigInteger initial, long maximum, String name) {
        if (initial.signum() < 0 || initial.compareTo(BigInteger.valueOf(maximum)) > 0) {
            throw NativeFailure.raise("core.compression", "invalid_input",
                "runtime/checksum@1::" + name + "/2", "checksum seed out of range");
        }
        return initial.longValue();
    }

    public static BigInteger runtime_checksum__checksum_crc32(List<?> origin, BigInteger initial) {
        int crc = ~(int) seed(initial, 0xFFFFFFFFL, "checksum_crc32");
        for (byte octet : Natives.toBytes(origin)) {
            crc = CRC32_TABLE[(crc ^ octet) & 0xFF] ^ (crc >>> 8);
        }
        return BigInteger.valueOf(Integer.toUnsignedLong(~crc));
    }

    public static BigInteger runtime_checksum__checksum_crc16(List<?> origin, BigInteger initial) {
        int crc = (int) seed(initial, 0xFFFFL, "checksum_crc16");
        for (byte octet : Natives.toBytes(origin)) {
            crc ^= (octet & 0xFF) << 8;
            for (int bit = 0; bit < 8; bit++) {
                crc = (crc & 0x8000) != 0 ? ((crc << 1) ^ 0x1021) & 0xFFFF : (crc << 1) & 0xFFFF;
            }
        }
        return BigInteger.valueOf(crc);
    }

    public static BigInteger runtime_checksum__checksum_adler32(List<?> origin, BigInteger initial) {
        long packed = seed(initial, 0xFFFFFFFFL, "checksum_adler32");
        long low = (packed & 0xFFFF) % 65521;
        long high = (packed >>> 16) % 65521;
        for (byte octet : Natives.toBytes(origin)) {
            low = (low + (octet & 0xFF)) % 65521;
            high = (high + low) % 65521;
        }
        return BigInteger.valueOf((high << 16) | low);
    }
}
