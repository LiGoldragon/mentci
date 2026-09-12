use std::path::PathBuf;

use meta_signal_mentci::ComponentSocketKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("expected exactly one binary startup path argument")]
    StartupArgumentCount,

    #[error("expected exactly one Mentci request argument")]
    ClientArgumentCount,

    #[error("startup argument must be a binary signal file, not DOTOS: {0}")]
    StartupDotosRejected(PathBuf),

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

    #[error("configuration is missing required component socket: {kind:?}")]
    MissingComponentSocket { kind: ComponentSocketKind },

    #[error("daemon IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("frame error: {0}")]
    Frame(#[from] signal_frame::FrameError),

    #[error("criome: {0}")]
    Criome(#[from] criome::Error),

    #[error("unexpected criome meta reply")]
    UnexpectedCriomeMetaReply,

    #[error("introspection Signal frame did not decode: {0}")]
    IntrospectionSignal(String),

    #[error("frame length {found} exceeds the {limit} byte limit")]
    FrameLength { limit: usize, found: usize },

    #[error("shared observation model did not produce a mentci request")]
    ClientObservationCommandUnavailable,

    #[error("unexpected mentci reply: {0}")]
    UnexpectedMentciReply(String),

    #[error("signal-mentci frame error: {0}")]
    SignalMentci(#[from] signal_mentci::SignalFrameError),

    #[error("meta-signal-mentci frame error: {0}")]
    MetaSignalMentci(#[from] meta_signal_mentci::SignalFrameError),

    #[error("signal-mentci DOTOS input error: {0}")]
    SignalMentciDotos(#[from] signal_mentci::DotosDecodeError),

    #[error("actor call failed: {0}")]
    ActorCall(String),

    #[error("frame body is not a request")]
    ExpectedRequest,

    #[error("datom text did not compose the expected value: {0}")]
    Datom(String),

    #[error("preflight API failed: {0}")]
    PreflightApi(String),

    #[error("unverified model for {slot}: profile {profile} requires {required_identifier}")]
    UnverifiedModel {
        slot: &'static str,
        profile: String,
        required_identifier: String,
    },

    #[error("preflight launch rejected: {0}")]
    PreflightLaunch(String),
}

pub type Result<T> = std::result::Result<T, Error>;
