use std::{error::Error, fmt};

use serde::{Serialize, de::DeserializeOwned};

use crate::proto::PROTOCOL;

pub const MAX_FRAME: usize = 16 * 1024 * 1024;

const HEADER: usize = 4;

#[derive(Debug)]
pub enum CodecError {
    Oversize(usize),
    VersionFrame(usize),
    Postcard(postcard::Error),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Oversize(len) => {
                write!(f, "frame of {len} bytes exceeds the limit of {MAX_FRAME}")
            }
            CodecError::VersionFrame(len) => {
                write!(f, "version frame has {len} bytes, expected {HEADER}")
            }
            CodecError::Postcard(error) => write!(f, "postcard: {error}"),
        }
    }
}

impl Error for CodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CodecError::Oversize(_) | CodecError::VersionFrame(_) => None,
            CodecError::Postcard(error) => Some(error),
        }
    }
}

impl From<postcard::Error> for CodecError {
    fn from(error: postcard::Error) -> CodecError {
        CodecError::Postcard(error)
    }
}

pub fn encode_version(out: &mut Vec<u8>) {
    out.extend_from_slice(&PROTOCOL.to_le_bytes());
}

pub fn decode_version(bytes: &[u8]) -> Result<u32, CodecError> {
    let header: [u8; HEADER] = bytes
        .try_into()
        .map_err(|_| CodecError::VersionFrame(bytes.len()))?;
    Ok(u32::from_le_bytes(header))
}

pub fn encode<T: Serialize>(msg: &T, out: &mut Vec<u8>) -> Result<(), CodecError> {
    let payload = postcard::to_allocvec(msg)?;
    let len = u32::try_from(payload.len())
        .ok()
        .filter(|len| *len as usize <= MAX_FRAME)
        .ok_or(CodecError::Oversize(payload.len()))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(())
}

#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn next_frame<T: DeserializeOwned>(&mut self) -> Option<Result<T, CodecError>> {
        let header: [u8; HEADER] = self.buf.get(..HEADER)?.try_into().ok()?;
        let len = u32::from_le_bytes(header) as usize;
        if len > MAX_FRAME {
            self.buf.clear();
            return Some(Err(CodecError::Oversize(len)));
        }
        let payload = self.buf.get(HEADER..HEADER + len)?;
        let msg = postcard::from_bytes(payload).map_err(CodecError::from);
        self.buf.drain(..HEADER + len);
        Some(msg)
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
