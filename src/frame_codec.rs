use std::io::{Read, Write};

use signal_introspect::{ByteViewable, Restorable, Signal, Signalizable};
use signal_mentci::MentciFrame;

use crate::{Error, Result};

/// Length-prefixed transport for the two wires the daemon speaks.
///
/// The Mentci wire is a `signal-frame` envelope, which carries its own length
/// prefix inside the bytes it decodes. The introspect wire is a portable rkyv
/// `Signal` frame: four big-endian length bytes followed by exactly that many
/// contract bytes, with no envelope of any kind.
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
        let bytes = self.read_length_prefixed_bytes(reader)?;
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

    /// Write one contract value as a portable rkyv Signal frame.
    pub fn write_introspection_signal<Writer, Value>(
        &self,
        writer: &mut Writer,
        value: &Value,
    ) -> Result<()>
    where
        Writer: Write,
        Value: Signalizable,
    {
        let signal = value
            .signalize()
            .map_err(|error| Error::IntrospectionSignal(error.to_string()))?;
        let bytes = signal.bytes();
        let length = u32::try_from(bytes.len()).map_err(|_| Error::FrameLength {
            limit: self.maximum_frame_bytes,
            found: bytes.len(),
        })?;
        writer.write_all(&length.to_be_bytes())?;
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Read one contract value back out of a portable rkyv Signal frame.
    pub fn read_introspection_signal<Reader, Value>(&self, reader: &mut Reader) -> Result<Value>
    where
        Reader: Read,
        Signal<Value>: Restorable<Value>,
    {
        let bytes = self.read_payload_bytes(reader)?;
        Signal::<Value>::from(bytes)
            .restore()
            .map_err(|error| Error::IntrospectionSignal(error.to_string()))
    }

    fn read_payload_bytes<Reader>(&self, reader: &mut Reader) -> Result<Vec<u8>>
    where
        Reader: Read,
    {
        let length = self.read_length(reader)?;
        let mut bytes = vec![0_u8; length];
        reader.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn read_length_prefixed_bytes<Reader>(&self, reader: &mut Reader) -> Result<Vec<u8>>
    where
        Reader: Read,
    {
        let length = self.read_length(reader)?;
        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&(length as u32).to_be_bytes());
        let start = bytes.len();
        bytes.resize(start + length, 0);
        reader.read_exact(&mut bytes[start..])?;
        Ok(bytes)
    }

    fn read_length<Reader>(&self, reader: &mut Reader) -> Result<usize>
    where
        Reader: Read,
    {
        let mut length_bytes = [0_u8; 4];
        reader.read_exact(&mut length_bytes)?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        if length > self.maximum_frame_bytes {
            return Err(Error::FrameLength {
                limit: self.maximum_frame_bytes,
                found: length,
            });
        }
        Ok(length)
    }
}
