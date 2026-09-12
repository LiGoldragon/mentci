//! The daemon's window onto the introspect component.
//!
//! `signal-introspect` 2.0 speaks portable rkyv Signal frames of its own
//! `Query` and `Response` contract; there is no envelope, no route and no
//! sub-reply layer. What the daemon shows in its introspect pane is the
//! contract's own datom text, projected straight off the replies.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use datom_codec::{Compositional, Datomizable};
use signal_introspect::{
    ComponentTraceQuery, IntrospectionTarget, PrototypeWitnessObservationQuery, Query, Response,
};
use signal_mentci::{ContextBody, PaneContent, PaneLabel};

use crate::Error;
use crate::datom_text::textualize;
use crate::frame_codec::FrameCodec;

const INTROSPECT_PANE_LABEL: &str = "introspect";
const PROTOTYPE_ENGINE: &str = "prototype";

#[derive(Debug, Clone)]
pub struct IntrospectionBridge {
    socket_path: PathBuf,
    codec: FrameCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrospectionPane {
    content: PaneContent,
}

/// What one introspect pane carries, as a value. The pane body is this value's
/// canonical datom text and nothing else, so every pane the daemon renders can
/// be read back into this type.
#[derive(Compositional, Datomizable, Clone, Debug, PartialEq)]
pub enum IntrospectionObservation {
    IntrospectOverview(Vec<Response>),
    IntrospectUnavailable(String),
}

impl IntrospectionBridge {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            codec: FrameCodec::new(),
        }
    }

    pub fn prototype_overview_pane(&self) -> crate::Result<IntrospectionPane> {
        let responses = vec![
            self.submit(&Self::prototype_witness_query())?,
            self.submit(&Self::prototype_signal_trace_query())?,
        ];
        Ok(IntrospectionPane::from(
            IntrospectionObservation::IntrospectOverview(responses),
        ))
    }

    fn submit(&self, query: &Query) -> crate::Result<Response> {
        let mut stream = UnixStream::connect(&self.socket_path)?;
        self.codec.write_introspection_signal(&mut stream, query)?;
        self.codec.read_introspection_signal(&mut stream)
    }

    fn prototype_witness_query() -> Query {
        Query::PrototypeWitnessObservation(PrototypeWitnessObservationQuery {
            engine_identifier: PROTOTYPE_ENGINE.to_owned(),
        })
    }

    fn prototype_signal_trace_query() -> Query {
        Query::ComponentTrace(ComponentTraceQuery {
            engine_identifier: PROTOTYPE_ENGINE.to_owned(),
            introspection_target: IntrospectionTarget::Signal,
            optional_trace_event_name: None,
        })
    }
}

impl IntrospectionPane {
    pub fn from_error(error: &Error) -> Self {
        Self::from(IntrospectionObservation::IntrospectUnavailable(
            error.to_string(),
        ))
    }

    pub fn into_content(self) -> PaneContent {
        self.content
    }
}

impl From<IntrospectionObservation> for IntrospectionPane {
    fn from(observation: IntrospectionObservation) -> Self {
        Self {
            content: PaneContent {
                pane: PaneLabel::new(INTROSPECT_PANE_LABEL),
                body: ContextBody::new(textualize(&observation)),
            },
        }
    }
}

impl From<Response> for IntrospectionPane {
    fn from(response: Response) -> Self {
        Self::from(IntrospectionObservation::IntrospectOverview(vec![response]))
    }
}
