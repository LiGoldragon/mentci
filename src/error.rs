use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("expected exactly one binary startup path argument")]
    StartupArgumentCount,

    #[error("expected exactly one Mentci request argument")]
    ClientArgumentCount,

    #[error("startup argument must be a binary signal file, not NOTA: {0}")]
    StartupNotaRejected(PathBuf),

    #[error("read configuration {path}: {source}")]
    ConfigurationRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("write configuration {path}: {source}")]
    ConfigurationWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("configuration archive did not decode")]
    ConfigurationArchiveDecode,

    #[error("configuration input must be Configure")]
    ConfigurationInputNotConfigure,

    #[error("configuration archive did not encode")]
    ConfigurationArchiveEncode,

    #[error("unsupported socket: only Unix sockets are implemented in this slice")]
    UnsupportedSocket,

    #[error("daemon IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("frame error: {0}")]
    Frame(#[from] signal_frame::FrameError),

    #[error("signal-mentci frame error: {0}")]
    SignalMentci(#[from] signal_mentci::SignalFrameError),

    #[error("meta-signal-mentci frame error: {0}")]
    MetaSignalMentci(#[from] meta_signal_mentci::SignalFrameError),

    #[error("signal-mentci NOTA input error: {0}")]
    SignalMentciNota(#[from] signal_mentci::NotaDecodeError),

    #[error("actor call failed: {0}")]
    ActorCall(String),

    #[error("frame body is not a request")]
    ExpectedRequest,
}

pub type Result<T> = std::result::Result<T, Error>;
