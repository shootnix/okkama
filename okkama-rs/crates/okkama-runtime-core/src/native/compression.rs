//! `runtime/compression`: потоковые DEFLATE (RFC 1951), zlib (RFC 1950) и
//! gzip (RFC 1952) на miniz_oxide. Заголовок и хвост gzip (CRC-32, ISIZE)
//! строятся и проверяются здесь; Adler-32 zlib проверяет miniz_oxide. Один
//! объект сжимает или распаковывает один поток; Sparrow берёт эти natives
//! через мост синхронных natives (`native_origin`).
use std::cell::RefCell;
use std::rc::Rc;

use super::Error;
use crate::{Value, abi::Native, abi::NativeFailure, heap::ArrayHeap};
use miniz_oxide::deflate::core::{
    CompressorOxide, TDEFLFlush, TDEFLStatus, compress, create_comp_flags_from_zip_params,
};
use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZError, MZFlush, MZStatus};

const CHUNK: usize = 64 * 1024;
/// Самый длинный заголовок gzip, который ждём целиком (FEXTRA, FNAME, FCOMMENT).
const HEADER_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Deflate,
    Zlib,
    Gzip,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Header,
    Body,
    Trailer,
    Ended,
}

/// Отказ кодека: код `core.compression` и сообщение.
struct Fail(&'static str, String);

fn invalid(message: impl Into<String>) -> Fail {
    Fail("invalid_input", message.into())
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 (ISO-HDLC) `crc`, продолженный байтами: crc32(crc32(0, a), b) ==
/// crc32(0, a ++ b).
fn crc32(crc: u32, bytes: &[u8]) -> u32 {
    let mut c = !crc;
    for &byte in bytes {
        c = CRC_TABLE[((c ^ byte as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

struct Compressor {
    engine: Box<CompressorOxide>,
    format: Format,
    crc: u32,
    size: u32,
    started: bool,
    finished: bool,
}

impl Compressor {
    fn new(format: Format, level: u8) -> Self {
        let window = if format == Format::Zlib { 15 } else { -15 };
        let flags = create_comp_flags_from_zip_params(level as i32, window, 0);
        Self {
            engine: Box::new(CompressorOxide::new(flags)),
            format,
            crc: 0,
            size: 0,
            started: false,
            finished: false,
        }
    }

    fn header(&mut self, out: &mut Vec<u8>) {
        if self.format == Format::Gzip && !self.started {
            // ID1 ID2 CM=8 FLG=0 MTIME=0 XFL=0 OS=255 (неизвестна).
            out.extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255]);
        }
        self.started = true;
    }

    fn run(&mut self, mut input: &[u8], flush: TDEFLFlush, out: &mut Vec<u8>) -> Result<(), Fail> {
        let mut buffer = vec![0u8; CHUNK];
        loop {
            let (status, consumed, written) = compress(&mut self.engine, input, &mut buffer, flush);
            out.extend_from_slice(&buffer[..written]);
            input = &input[consumed..];
            match status {
                TDEFLStatus::Done => return Ok(()),
                TDEFLStatus::Okay => {
                    if flush == TDEFLFlush::None && input.is_empty() && written < buffer.len() {
                        return Ok(());
                    }
                }
                _ => return Err(Fail("invalid_state", "The compressor failed".into())),
            }
        }
    }

    fn write(&mut self, input: &[u8]) -> Result<Vec<u8>, Fail> {
        if self.finished {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        }
        let mut out = Vec::new();
        self.header(&mut out);
        self.crc = crc32(self.crc, input);
        self.size = self.size.wrapping_add(input.len() as u32);
        self.run(input, TDEFLFlush::None, &mut out)?;
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, Fail> {
        if self.finished {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        }
        let mut out = Vec::new();
        self.header(&mut out);
        self.run(&[], TDEFLFlush::Finish, &mut out)?;
        if self.format == Format::Gzip {
            out.extend_from_slice(&self.crc.to_le_bytes());
            out.extend_from_slice(&self.size.to_le_bytes());
        }
        self.finished = true;
        Ok(out)
    }
}

/// Длина полного заголовка gzip в начале `bytes`, `None`, пока он не пришёл
/// целиком.
fn gzip_header(bytes: &[u8]) -> Result<Option<usize>, Fail> {
    let magic = [0x1f, 0x8b, 8];
    for (index, expected) in magic.iter().enumerate() {
        if bytes.get(index).is_some_and(|byte| byte != expected) {
            return Err(invalid("The data is not a gzip stream"));
        }
    }
    if bytes.len() < 10 {
        return Ok(None);
    }
    let flags = bytes[3];
    if flags & 0xe0 != 0 {
        return Err(invalid("A reserved gzip flag is set"));
    }
    let mut at = 10;
    if flags & 4 != 0 {
        if bytes.len() < at + 2 {
            return Ok(None);
        }
        at += 2 + u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        if bytes.len() < at {
            return Ok(None);
        }
    }
    for flag in [8u8, 16] {
        if flags & flag != 0 {
            match bytes[at..].iter().position(|&byte| byte == 0) {
                Some(end) => at += end + 1,
                None => return Ok(None),
            }
        }
    }
    if flags & 2 != 0 {
        at += 2;
        if bytes.len() < at {
            return Ok(None);
        }
    }
    Ok(Some(at))
}

struct Decompressor {
    engine: Box<InflateState>,
    format: Format,
    stage: Stage,
    /// Начало заголовка или хвоста, пришедшее не целиком.
    held: Vec<u8>,
    crc: u32,
    size: u32,
    produced: u64,
    limit: u64,
    remainder: Vec<u8>,
}

impl Decompressor {
    fn new(format: Format, limit: u64) -> Self {
        let data = if format == Format::Zlib { DataFormat::Zlib } else { DataFormat::Raw };
        Self {
            engine: InflateState::new_boxed(data),
            format,
            stage: if format == Format::Gzip { Stage::Header } else { Stage::Body },
            held: Vec::new(),
            crc: 0,
            size: 0,
            produced: 0,
            limit,
            remainder: Vec::new(),
        }
    }

    fn write(&mut self, source: &[u8]) -> Result<Vec<u8>, Fail> {
        let mut input = std::mem::take(&mut self.held);
        input.extend_from_slice(source);
        let mut out = Vec::new();
        let mut at = 0usize;
        let mut buffer = vec![0u8; CHUNK];
        loop {
            match self.stage {
                Stage::Ended => {
                    self.remainder.extend_from_slice(&input[at..]);
                    return Ok(out);
                }
                Stage::Header => match gzip_header(&input[at..])? {
                    Some(length) => {
                        at += length;
                        self.stage = Stage::Body;
                    }
                    None => {
                        if input.len() - at > HEADER_LIMIT {
                            return Err(invalid("The gzip header is too long"));
                        }
                        self.held = input[at..].to_vec();
                        return Ok(out);
                    }
                },
                Stage::Trailer => {
                    if input.len() - at < 8 {
                        self.held = input[at..].to_vec();
                        return Ok(out);
                    }
                    let tail = &input[at..at + 8];
                    let crc = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
                    let size = u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]);
                    if crc != self.crc || size != self.size {
                        return Err(invalid("The gzip trailer does not match the data"));
                    }
                    at += 8;
                    self.stage = Stage::Ended;
                }
                Stage::Body => {
                    let result = inflate(&mut self.engine, &input[at..], &mut buffer, MZFlush::None);
                    at += result.bytes_consumed;
                    let produced = &buffer[..result.bytes_written];
                    self.produced += produced.len() as u64;
                    if self.produced > self.limit {
                        return Err(Fail(
                            "resource_exhausted",
                            format!("Decompressed data exceeds {} bytes", self.limit),
                        ));
                    }
                    if self.format == Format::Gzip {
                        self.crc = crc32(self.crc, produced);
                        self.size = self.size.wrapping_add(produced.len() as u32);
                    }
                    out.extend_from_slice(produced);
                    match result.status {
                        Ok(MZStatus::StreamEnd) => {
                            self.stage = if self.format == Format::Gzip { Stage::Trailer } else { Stage::Ended };
                        }
                        Ok(_) | Err(MZError::Buf) => {
                            let stuck = result.bytes_consumed == 0 && result.bytes_written == 0;
                            if stuck || (at >= input.len() && result.bytes_written < buffer.len()) {
                                return Ok(out);
                            }
                        }
                        Err(_) => return Err(invalid("The compressed data is damaged")),
                    }
                }
            }
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, Fail> {
        if self.stage != Stage::Ended {
            return Err(invalid("The compressed stream is truncated"));
        }
        Ok(Vec::new())
    }
}

enum Codec {
    Compress(Compressor),
    Decompress(Decompressor),
}

fn failure(native: Native, Fail(code, message): Fail) -> Error {
    Error::Failure(NativeFailure {
        domain: "core.compression".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message,
    })
}

fn bytes(value: &Value, arrays: &ArrayHeap) -> Result<Vec<u8>, Error> {
    Ok(arrays
        .elements(value)?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?)
}

fn state(value: &Value) -> Result<&RefCell<Codec>, Error> {
    super::support::handle::<RefCell<Codec>>(value)
}

fn array(arrays: &mut ArrayHeap, out: Vec<u8>) -> Value {
    arrays.allocate(out.into_iter().map(Value::Byte).collect())
}

pub fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        CompressionCodecOpen
            | CompressionCodecWrite
            | CompressionCodecFinish
            | CompressionCodecEnded
            | CompressionCodecRemainder
    )
}

pub fn execute(native: Native, args: &[Value], arrays: &mut ArrayHeap) -> Result<Value, Error> {
    use Native::*;
    match native {
        CompressionCodecOpen => {
            let format = match args[0].as_string()? {
                "deflate" => Format::Deflate,
                "zlib" => Format::Zlib,
                "gzip" => Format::Gzip,
                _ => return Err(failure(native, invalid("Unknown compression format"))),
            };
            let compressing = args[1].as_bool()?;
            let codec = if compressing {
                let Some(level) = args[2].as_int()?.to_i64().filter(|level| (0..=9).contains(level)) else {
                    return Err(failure(native, invalid("Compression level must be 0..9")));
                };
                Codec::Compress(Compressor::new(format, level as u8))
            } else {
                let Some(limit) = args[3].as_int()?.to_i64().filter(|limit| *limit > 0) else {
                    return Err(failure(native, invalid("Decompression limit must be positive")));
                };
                Codec::Decompress(Decompressor::new(format, limit as u64))
            };
            Ok(Value::Handle(Rc::new(RefCell::new(codec))))
        }
        CompressionCodecWrite => {
            let input = bytes(&args[1], arrays)?;
            let out = match &mut *state(&args[0])?.borrow_mut() {
                Codec::Compress(codec) => codec.write(&input),
                Codec::Decompress(codec) => codec.write(&input),
            }
            .map_err(|fail| failure(native, fail))?;
            Ok(array(arrays, out))
        }
        CompressionCodecFinish => {
            let out = match &mut *state(&args[0])?.borrow_mut() {
                Codec::Compress(codec) => codec.finish(),
                Codec::Decompress(codec) => codec.finish(),
            }
            .map_err(|fail| failure(native, fail))?;
            Ok(array(arrays, out))
        }
        CompressionCodecEnded => Ok(Value::Bool(match &*state(&args[0])?.borrow() {
            Codec::Compress(codec) => codec.finished,
            Codec::Decompress(codec) => codec.stage == Stage::Ended,
        })),
        CompressionCodecRemainder => {
            let out = match &*state(&args[0])?.borrow() {
                Codec::Compress(_) => Vec::new(),
                Codec::Decompress(codec) => codec.remainder.clone(),
            };
            Ok(array(arrays, out))
        }
        _ => Err(Error::InvalidState("not a compression native".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(format: Format, data: &[u8], pieces: usize) -> Vec<u8> {
        let mut compressor = Compressor::new(format, 6);
        let mut packed = Vec::new();
        for chunk in data.chunks((data.len() / pieces).max(1)) {
            packed.extend(compressor.write(chunk).ok().expect("write"));
        }
        packed.extend(compressor.finish().ok().expect("finish"));
        let mut decompressor = Decompressor::new(format, 1 << 30);
        let mut plain = Vec::new();
        for chunk in packed.chunks(7) {
            plain.extend(decompressor.write(chunk).ok().expect("inflate"));
        }
        assert!(decompressor.finish().is_ok());
        plain
    }

    #[test]
    fn every_format_round_trips_in_pieces() {
        let data: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8 ^ (n / 1000) as u8).collect();
        for format in [Format::Deflate, Format::Zlib, Format::Gzip] {
            assert_eq!(roundtrip(format, &data, 13), data);
            assert_eq!(roundtrip(format, b"", 1), b"");
        }
    }

    #[test]
    fn crc32_is_the_iso_hdlc_check() {
        assert_eq!(crc32(0, b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(crc32(0, b"1234"), b"56789"), 0xCBF4_3926);
    }

    #[test]
    fn gzip_rejects_a_damaged_trailer_and_keeps_the_remainder() {
        let mut compressor = Compressor::new(Format::Gzip, 6);
        let mut packed = compressor.write(b"hello").ok().unwrap();
        packed.extend(compressor.finish().ok().unwrap());
        let mut tail = packed.clone();
        tail.extend_from_slice(b"rest");
        let mut decompressor = Decompressor::new(Format::Gzip, 100);
        assert_eq!(decompressor.write(&tail).ok().unwrap(), b"hello");
        assert_eq!(decompressor.remainder, b"rest");
        let last = packed.len() - 1;
        packed[last] ^= 1;
        let mut broken = Decompressor::new(Format::Gzip, 100);
        assert!(broken.write(&packed).is_err());
        let mut small = Decompressor::new(Format::Gzip, 3);
        assert!(small.write(&tail).is_err());
    }
}
