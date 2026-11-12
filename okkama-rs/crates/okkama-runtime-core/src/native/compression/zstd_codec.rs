//! zstd (RFC 8878) on the reference C library (#184). A compressor writes one
//! frame with a content checksum; a decompressor reads one frame, and the
//! bytes after it are the remainder, as with gzip. The decoder keeps zstd's
//! own window cap (128 MiB), so a frame asking for more is refused.

use super::{CHUNK, Fail, Stream, exceeded, invalid};
use zstd::stream::raw::{CParameter, Decoder, Encoder, InBuffer, Operation, OutBuffer};

fn broken(error: std::io::Error) -> Fail {
    Fail("invalid_state", format!("The compressor failed: {error}"))
}

pub(super) struct ZstdCompressor {
    engine: Encoder<'static>,
    finished: bool,
}

impl ZstdCompressor {
    pub(super) fn new(level: i32) -> Result<Self, Fail> {
        let mut engine = Encoder::new(level).map_err(broken)?;
        engine.set_parameter(CParameter::ChecksumFlag(true)).map_err(broken)?;
        Ok(Self { engine, finished: false })
    }
}

impl Stream for ZstdCompressor {
    fn write(&mut self, data: &[u8]) -> Result<Vec<u8>, Fail> {
        if self.finished {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        }
        let mut input = InBuffer::around(data);
        let mut out = Vec::new();
        while input.pos() < data.len() {
            out.reserve(CHUNK);
            let start = out.len();
            let mut output = OutBuffer::around_pos(&mut out, start);
            self.engine.run(&mut input, &mut output).map_err(broken)?;
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<u8>, Fail> {
        if self.finished {
            return Err(Fail("invalid_state", "The compressor has finished".into()));
        }
        let mut out = Vec::new();
        loop {
            out.reserve(CHUNK);
            let start = out.len();
            let mut output = OutBuffer::around_pos(&mut out, start);
            if self.engine.finish(&mut output, true).map_err(broken)? == 0 {
                break;
            }
        }
        self.finished = true;
        Ok(out)
    }

    fn ended(&self) -> bool {
        self.finished
    }

    fn remainder(&self) -> Vec<u8> {
        Vec::new()
    }
}

pub(super) struct ZstdDecompressor {
    engine: Decoder<'static>,
    ended: bool,
    produced: u64,
    limit: u64,
    remainder: Vec<u8>,
}

impl ZstdDecompressor {
    pub(super) fn new(limit: u64) -> Result<Self, Fail> {
        Ok(Self {
            engine: Decoder::new().map_err(broken)?,
            ended: false,
            produced: 0,
            limit,
            remainder: Vec::new(),
        })
    }
}

impl Stream for ZstdDecompressor {
    fn write(&mut self, source: &[u8]) -> Result<Vec<u8>, Fail> {
        if self.ended {
            self.remainder.extend_from_slice(source);
            return Ok(Vec::new());
        }
        let mut input = InBuffer::around(source);
        let mut out = Vec::new();
        loop {
            out.reserve(CHUNK);
            let start = out.len();
            let mut output = OutBuffer::around_pos(&mut out, start);
            let hint = self
                .engine
                .run(&mut input, &mut output)
                .map_err(|_| invalid("The compressed data is damaged"))?;
            let full = output.pos() == output.capacity();
            let written = output.pos() - start;
            self.produced += written as u64;
            if self.produced > self.limit {
                return Err(exceeded(self.limit));
            }
            if hint == 0 {
                self.ended = true;
                self.remainder.extend_from_slice(&source[input.pos()..]);
                return Ok(out);
            }
            if input.pos() == source.len() && !full {
                return Ok(out);
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
