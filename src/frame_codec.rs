use std::io::{Read, Write};

use signal_mentci::MentciFrame;

use crate::Result;

#[derive(Debug, Clone, Copy)]
pub struct FrameCodec {
    maximum_frame_bytes: usize,
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCodec {
    pub const fn new() -> Self {
        Self {
            maximum_frame_bytes: 16 * 1024 * 1024,
        }
    }

    pub fn read_mentci_frame<Reader>(&self, reader: &mut Reader) -> Result<MentciFrame>
    where
        Reader: Read,
    {
        let mut length_bytes = [0_u8; 4];
        reader.read_exact(&mut length_bytes)?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        if length > self.maximum_frame_bytes {
            return Err(signal_frame::FrameError::LengthMismatch {
                expected: self.maximum_frame_bytes,
                found: length,
            }
            .into());
        }
        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&length_bytes);
        let start = bytes.len();
        bytes.resize(start + length, 0);
        reader.read_exact(&mut bytes[start..])?;
        Ok(MentciFrame::decode_length_prefixed(&bytes)?)
    }

    pub fn write_mentci_frame<Writer>(&self, writer: &mut Writer, frame: &MentciFrame) -> Result<()>
    where
        Writer: Write,
    {
        let bytes = frame.encode_length_prefixed()?;
        writer.write_all(&bytes)?;
        writer.flush()?;
        Ok(())
    }
}
