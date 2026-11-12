package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.util.Arrays;
import java.util.List;
import java.util.zip.CRC32;
import java.util.zip.DataFormatException;
import java.util.zip.Deflater;
import java.util.zip.Inflater;

/**
 * runtime/compression: потоковые DEFLATE (RFC 1951), zlib (RFC 1950) и gzip
 * (RFC 1952) на java.util.zip, как в okkama-runtime-core (native/compression.rs):
 * заголовок и хвост gzip строятся и проверяются здесь, отказы — core.compression.
 */
public final class CompressionNatives {
    private CompressionNatives() {}

    private static final int CHUNK = 64 * 1024;
    private static final int HEADER_LIMIT = 64 * 1024;
    private static final String DOMAIN = "core.compression";

    private enum Format { DEFLATE, ZLIB, GZIP }

    private enum Stage { HEADER, BODY, TRAILER, ENDED }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, "runtime/compression@1::" + operation, message);
    }

    static final class Codec {
        boolean compressing;
        Format format;
        Deflater deflater;
        Inflater inflater;
        CRC32 crc = new CRC32();
        long size;
        boolean started;
        boolean finished;
        Stage stage;
        byte[] held = new byte[0];
        long produced;
        long limit;
        ByteArrayOutputStream remainder = new ByteArrayOutputStream();
    }

    private static Codec codec(Object handle) {
        return (Codec) handle;
    }

    public static Object runtime_compression__codec_open(String format, boolean compress, BigInteger level, BigInteger limit) {
        Codec codec = new Codec();
        codec.format = switch (format) {
            case "deflate" -> Format.DEFLATE;
            case "zlib" -> Format.ZLIB;
            case "gzip" -> Format.GZIP;
            default -> throw failure("invalid_input", "codec_open/4", "Unknown compression format");
        };
        codec.compressing = compress;
        if (compress) {
            if (level.signum() < 0 || level.compareTo(BigInteger.valueOf(9)) > 0) {
                throw failure("invalid_input", "codec_open/4", "Compression level must be 0..9");
            }
            codec.deflater = new Deflater(level.intValue(), codec.format != Format.ZLIB);
        } else {
            if (limit.signum() <= 0) {
                throw failure("invalid_input", "codec_open/4", "Decompression limit must be positive");
            }
            codec.limit = limit.bitLength() > 62 ? Long.MAX_VALUE : limit.longValue();
            codec.inflater = new Inflater(codec.format != Format.ZLIB);
            codec.stage = codec.format == Format.GZIP ? Stage.HEADER : Stage.BODY;
        }
        return codec;
    }

    public static List<Object> runtime_compression__codec_write(Object handle, List<?> source) {
        Codec codec = codec(handle);
        byte[] input = Natives.toBytes(source);
        byte[] out = codec.compressing ? compressed(codec, input) : decompressed(codec, input);
        return Natives.fromBytes(out, out.length);
    }

    public static List<Object> runtime_compression__codec_finish(Object handle) {
        Codec codec = codec(handle);
        if (codec.compressing) {
            if (codec.finished) {
                throw failure("invalid_state", "codec_finish/1", "The compressor has finished");
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            header(codec, out);
            codec.deflater.finish();
            byte[] buffer = new byte[CHUNK];
            while (!codec.deflater.finished()) {
                int written = codec.deflater.deflate(buffer);
                out.write(buffer, 0, written);
            }
            codec.deflater.end();
            if (codec.format == Format.GZIP) {
                little(out, codec.crc.getValue());
                little(out, codec.size & 0xffffffffL);
            }
            codec.finished = true;
            byte[] bytes = out.toByteArray();
            return Natives.fromBytes(bytes, bytes.length);
        }
        if (codec.stage != Stage.ENDED) {
            throw failure("invalid_input", "codec_finish/1", "The compressed stream is truncated");
        }
        return Natives.fromBytes(new byte[0], 0);
    }

    public static boolean runtime_compression__codec_ended(Object handle) {
        Codec codec = codec(handle);
        return codec.compressing ? codec.finished : codec.stage == Stage.ENDED;
    }

    public static List<Object> runtime_compression__codec_remainder(Object handle) {
        Codec codec = codec(handle);
        byte[] bytes = codec.remainder.toByteArray();
        return Natives.fromBytes(bytes, bytes.length);
    }

    private static void header(Codec codec, ByteArrayOutputStream out) {
        if (codec.format == Format.GZIP && !codec.started) {
            out.write(new byte[] {0x1f, (byte) 0x8b, 8, 0, 0, 0, 0, 0, 0, (byte) 255}, 0, 10);
        }
        codec.started = true;
    }

    private static void little(ByteArrayOutputStream out, long value) {
        for (int shift = 0; shift < 32; shift += 8) {
            out.write((int) (value >>> shift) & 0xff);
        }
    }

    private static byte[] compressed(Codec codec, byte[] input) {
        if (codec.finished) {
            throw failure("invalid_state", "codec_write/2", "The compressor has finished");
        }
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        header(codec, out);
        codec.crc.update(input);
        codec.size += input.length;
        codec.deflater.setInput(input);
        byte[] buffer = new byte[CHUNK];
        while (!codec.deflater.needsInput()) {
            int written = codec.deflater.deflate(buffer, 0, buffer.length, Deflater.NO_FLUSH);
            out.write(buffer, 0, written);
        }
        return out.toByteArray();
    }

    /** Длина полного заголовка gzip в bytes[from..], -1, пока он не пришёл целиком. */
    private static int gzipHeader(byte[] bytes, int from) {
        int available = bytes.length - from;
        int[] magic = {0x1f, 0x8b, 8};
        for (int index = 0; index < magic.length && index < available; index++) {
            if ((bytes[from + index] & 0xff) != magic[index]) {
                throw failure("invalid_input", "codec_write/2", "The data is not a gzip stream");
            }
        }
        if (available < 10) {
            return -1;
        }
        int flags = bytes[from + 3] & 0xff;
        if ((flags & 0xe0) != 0) {
            throw failure("invalid_input", "codec_write/2", "A reserved gzip flag is set");
        }
        int at = 10;
        if ((flags & 4) != 0) {
            if (available < at + 2) {
                return -1;
            }
            at += 2 + ((bytes[from + at] & 0xff) | (bytes[from + at + 1] & 0xff) << 8);
            if (available < at) {
                return -1;
            }
        }
        for (int flag : new int[] {8, 16}) {
            if ((flags & flag) != 0) {
                int end = at;
                while (end < available && bytes[from + end] != 0) {
                    end++;
                }
                if (end >= available) {
                    return -1;
                }
                at = end + 1;
            }
        }
        if ((flags & 2) != 0) {
            at += 2;
            if (available < at) {
                return -1;
            }
        }
        return at;
    }

    private static byte[] decompressed(Codec codec, byte[] source) {
        byte[] input = new byte[codec.held.length + source.length];
        System.arraycopy(codec.held, 0, input, 0, codec.held.length);
        System.arraycopy(source, 0, input, codec.held.length, source.length);
        codec.held = new byte[0];
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        int at = 0;
        byte[] buffer = new byte[CHUNK];
        while (true) {
            switch (codec.stage) {
                case ENDED -> {
                    codec.remainder.write(input, at, input.length - at);
                    return out.toByteArray();
                }
                case HEADER -> {
                    int length = gzipHeader(input, at);
                    if (length < 0) {
                        if (input.length - at > HEADER_LIMIT) {
                            throw failure("invalid_input", "codec_write/2", "The gzip header is too long");
                        }
                        codec.held = Arrays.copyOfRange(input, at, input.length);
                        return out.toByteArray();
                    }
                    at += length;
                    codec.stage = Stage.BODY;
                }
                case TRAILER -> {
                    if (input.length - at < 8) {
                        codec.held = Arrays.copyOfRange(input, at, input.length);
                        return out.toByteArray();
                    }
                    long crc = 0;
                    long size = 0;
                    for (int index = 3; index >= 0; index--) {
                        crc = crc << 8 | (input[at + index] & 0xff);
                        size = size << 8 | (input[at + 4 + index] & 0xff);
                    }
                    if (crc != codec.crc.getValue() || size != (codec.size & 0xffffffffL)) {
                        throw failure("invalid_input", "codec_write/2", "The gzip trailer does not match the data");
                    }
                    at += 8;
                    codec.stage = Stage.ENDED;
                }
                case BODY -> {
                    if (at < input.length) {
                        codec.inflater.setInput(input, at, input.length - at);
                    }
                    int written;
                    try {
                        written = codec.inflater.inflate(buffer);
                    } catch (DataFormatException damaged) {
                        throw failure("invalid_input", "codec_write/2", "The compressed data is damaged");
                    }
                    at = input.length - codec.inflater.getRemaining();
                    codec.produced += written;
                    if (codec.produced > codec.limit) {
                        throw failure("resource_exhausted", "codec_write/2",
                            "Decompressed data exceeds " + codec.limit + " bytes");
                    }
                    if (codec.format == Format.GZIP) {
                        codec.crc.update(buffer, 0, written);
                        codec.size += written;
                    }
                    out.write(buffer, 0, written);
                    if (codec.inflater.finished()) {
                        codec.inflater.end();
                        codec.stage = codec.format == Format.GZIP ? Stage.TRAILER : Stage.ENDED;
                    } else if (codec.inflater.needsDictionary()) {
                        throw failure("invalid_input", "codec_write/2", "The compressed data needs a dictionary");
                    } else if (written == 0 && codec.inflater.needsInput()) {
                        return out.toByteArray();
                    }
                }
            }
        }
    }
}
