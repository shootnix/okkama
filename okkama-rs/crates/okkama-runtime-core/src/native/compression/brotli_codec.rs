//! Brotli (RFC 7932) on the `brotli` crate, pure Rust (#184). The decoder is
//! strict: large-window streams, which RFC 7932 does not define, are refused.
//! The bytes after the end of the stream are the remainder.

use super::{CHUNK, Fail, Stream, exceeded, invalid};
use brotli::{BrotliDecompressStream, BrotliResult, BrotliState, CompressorWriter, HeapAlloc, HuffmanCode};
use std::io::Write;

/// The window (log2 of its size) the encoder uses: Brotli's default, 4 MiB.
const WINDOW: u32 = 22;

pub(super) struct BrotliCompressor {
    engine: Option<CompressorWriter<Vec<u8>>>,
}

impl BrotliCompressor {
    pub(super) fn new(quality: u32) -> Self {
        Self { engine: Some(CompressorWriter::new(Vec::new(), CHUNK, quality, WINDOW)) }
    }
}

impl Stream for BrotliCompressor {
    fn write(&mut self, data: &[u8]) -> Result<Vec<u8>, Fail> {
        let Some(engine) = self.engine.as_mut() else {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        };
        engine
            .write_all(data)
            .map_err(|error| Fail("invalid_state", format!("The compressor failed: {error}")))?;
        Ok(std::mem::take(engine.get_mut()))
    }

    fn finish(&mut self) -> Result<Vec<u8>, Fail> {
        let Some(engine) = self.engine.take() else {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        };
        Ok(engine.into_inner())
    }

    fn ended(&self) -> bool {
        self.engine.is_none()
    }

    fn remainder(&self) -> Vec<u8> {
        Vec::new()
    }
}

type State = BrotliState<HeapAlloc<u8>, HeapAlloc<u32>, HeapAlloc<HuffmanCode>>;

pub(super) struct BrotliDecompressor {
    engine: Box<State>,
    ended: bool,
    produced: u64,
    limit: u64,
    remainder: Vec<u8>,
}

impl BrotliDecompressor {
    pub(super) fn new(limit: u64) -> Self {
        let engine = State::new_strict(
            HeapAlloc::new(0),
            HeapAlloc::new(0),
            HeapAlloc::new(HuffmanCode { bits: 2, value: 1 }),
        );
        Self { engine: Box::new(engine), ended: false, produced: 0, limit, remainder: Vec::new() }
    }
}

impl Stream for BrotliDecompressor {
    fn write(&mut self, source: &[u8]) -> Result<Vec<u8>, Fail> {
        if self.ended {
            self.remainder.extend_from_slice(source);
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut buffer = vec![0u8; CHUNK];
        let mut available_in = source.len();
        let mut at = 0usize;
        loop {
            let mut available_out = buffer.len();
            let mut written = 0usize;
            let mut total = 0usize;
            let result = BrotliDecompressStream(
                &mut available_in,
                &mut at,
                source,
                &mut available_out,
                &mut written,
                &mut buffer,
                &mut total,
                &mut self.engine,
            );
            self.produced += written as u64;
            if self.produced > self.limit {
                return Err(exceeded(self.limit));
            }
            out.extend_from_slice(&buffer[..written]);
            match result {
                BrotliResult::ResultSuccess => {
                    self.ended = true;
                    self.remainder.extend_from_slice(&source[at..]);
                    return Ok(out);
                }
                BrotliResult::NeedsMoreInput => return Ok(out),
                BrotliResult::NeedsMoreOutput => {}
                BrotliResult::ResultFailure => return Err(invalid("The compressed data is damaged")),
            }
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, Fail> {
        if !self.ended {
            return Err(invalid("The compressed stream is truncated"));
        }
        Ok(Vec::new())
    }

    fn ended(&self) -> bool {
        self.ended
    }

    fn remainder(&self) -> Vec<u8> {
        self.remainder.clone()
    }
}
