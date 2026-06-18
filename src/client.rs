use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use signal_frame::{ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch};
use signal_mentci::{MentciFrame, MentciFrameBody, MentciRequest, NotaSource};

use crate::frame_codec::FrameCodec;
use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCommand {
    arguments: Vec<String>,
    socket_path: PathBuf,
}

impl ClientCommand {
    pub fn from_environment() -> Self {
        Self {
            arguments: std::env::args().skip(1).collect(),
            socket_path: Self::default_socket_path(),
        }
    }

    pub fn from_arguments<Arguments, Argument>(
        arguments: Arguments,
        socket_path: impl Into<PathBuf>,
    ) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self {
            arguments: arguments.into_iter().map(Into::into).collect(),
            socket_path: socket_path.into(),
        }
    }

    pub fn request_frame(&self) -> Result<MentciFrame> {
        let argument = self.input_argument()?;
        let path = Path::new(argument);
        if path.is_file() {
            self.request_frame_from_path(path)
        } else {
            self.request_frame_from_nota(argument)
        }
    }

    pub fn run(&self) -> Result<()> {
        let frame = self.request_frame()?;
        let codec = FrameCodec::new();
        let mut stream = UnixStream::connect(&self.socket_path)?;
        codec.write_mentci_frame(&mut stream, &frame)?;
        let reply = codec.read_mentci_frame(&mut stream)?;
        let bytes = reply.encode_length_prefixed()?;
        io::stdout().lock().write_all(&bytes)?;
        Ok(())
    }

    fn input_argument(&self) -> Result<&str> {
        match self.arguments.as_slice() {
            [argument] => Ok(argument.as_str()),
            _ => Err(Error::ClientArgumentCount),
        }
    }

    fn request_frame_from_path(&self, path: &Path) -> Result<MentciFrame> {
        if path
            .extension()
            .is_some_and(|extension| extension == "nota")
        {
            let source = std::fs::read_to_string(path)?;
            self.request_frame_from_nota(&source)
        } else {
            let bytes = std::fs::read(path)?;
            Ok(MentciFrame::decode_length_prefixed(&bytes)?)
        }
    }

    fn request_frame_from_nota(&self, source: &str) -> Result<MentciFrame> {
        let request = NotaSource::new(source).parse::<MentciRequest>()?;
        Ok(MentciFrame::new(MentciFrameBody::Request {
            exchange: self.exchange(),
            request: request.into_request(),
        }))
    }

    fn exchange(&self) -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    fn default_socket_path() -> PathBuf {
        match std::env::var_os("MENTCI_SOCKET") {
            Some(path) => PathBuf::from(path),
            None => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(directory) => PathBuf::from(directory).join("mentci.socket"),
                None => PathBuf::from("/tmp/mentci.socket"),
            },
        }
    }
}
