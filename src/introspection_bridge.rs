use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use nota::NotaEncode;
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use signal_introspect::{
    ComponentSnapshot, ComponentTrace, ComponentTraceQuery, DeliveryTrace, EngineSnapshot,
    IntrospectionDenied, IntrospectionFrame, IntrospectionFrameBody, IntrospectionReply,
    IntrospectionRequest, IntrospectionTarget, IntrospectionUnimplemented, PrototypeWitness,
    PrototypeWitnessQuery,
};
use signal_mentci::{ContextBody, PaneContent, PaneLabel};
use signal_persona::EngineIdentifier;

use crate::frame_codec::FrameCodec;
use crate::{Error, Result};

const INTROSPECT_PANE_LABEL: &str = "introspect";

#[derive(Debug, Clone)]
pub struct IntrospectionBridge {
    socket_path: PathBuf,
    codec: FrameCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrospectionPane {
    content: PaneContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IntrospectionObservation {
    request: IntrospectionRequest,
}

impl IntrospectionBridge {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            codec: FrameCodec::new(),
        }
    }

    pub fn prototype_overview_pane(&self) -> Result<IntrospectionPane> {
        let replies = vec![
            self.submit(IntrospectionObservation::prototype_witness())?,
            self.submit(IntrospectionObservation::prototype_signal_trace())?,
        ];
        Ok(IntrospectionPane::from_replies(replies))
    }

    fn submit(&self, observation: IntrospectionObservation) -> Result<IntrospectionReply> {
        let mut stream = UnixStream::connect(&self.socket_path)?;
        self.codec
            .write_introspection_frame(&mut stream, &observation.into_frame())?;
        let frame = self.codec.read_introspection_frame(&mut stream)?;
        match frame.into_body() {
            IntrospectionFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(output) => Ok(output),
                    other => Err(Error::UnexpectedIntrospectionReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(Error::UnexpectedIntrospectionReply(format!(
                    "rejected: {reason:?}"
                ))),
            },
            other => Err(Error::UnexpectedIntrospectionReply(format!("{other:?}"))),
        }
    }
}

impl IntrospectionObservation {
    fn prototype_witness() -> Self {
        Self {
            request: IntrospectionRequest::PrototypeWitness(PrototypeWitnessQuery {
                engine: EngineIdentifier::new("prototype"),
            }),
        }
    }

    fn prototype_signal_trace() -> Self {
        Self {
            request: IntrospectionRequest::ComponentTrace(ComponentTraceQuery::new(
                EngineIdentifier::new("prototype"),
                IntrospectionTarget::Signal,
                None,
            )),
        }
    }

    fn into_frame(self) -> IntrospectionFrame {
        IntrospectionFrame::new(IntrospectionFrameBody::Request {
            exchange: Self::exchange(),
            request: self.request.into_request(),
        })
    }

    fn exchange() -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }
}

impl IntrospectionPane {
    pub fn from_error(error: &Error) -> Self {
        Self::new(format!("(IntrospectUnavailable [{}])", error))
    }

    pub fn into_content(self) -> PaneContent {
        self.content
    }

    fn new(body: impl Into<String>) -> Self {
        Self {
            content: PaneContent {
                pane: PaneLabel::new(INTROSPECT_PANE_LABEL),
                body: ContextBody::new(body.into()),
            },
        }
    }

    fn from_replies(replies: Vec<IntrospectionReply>) -> Self {
        let rendered = replies
            .into_iter()
            .map(IntrospectionReplyRendering::from)
            .map(IntrospectionReplyRendering::into_body)
            .collect::<Vec<_>>()
            .join(" ");
        Self::new(format!("(IntrospectOverview {rendered})"))
    }

    fn wrap(head: &str, payload: String) -> String {
        format!("({head} {payload})")
    }
}

impl From<IntrospectionReply> for IntrospectionPane {
    fn from(reply: IntrospectionReply) -> Self {
        Self::new(IntrospectionReplyRendering::from(reply).into_body())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IntrospectionReplyRendering {
    body: String,
}

impl IntrospectionReplyRendering {
    fn into_body(self) -> String {
        self.body
    }
}

impl From<IntrospectionReply> for IntrospectionReplyRendering {
    fn from(reply: IntrospectionReply) -> Self {
        let body = match reply {
            IntrospectionReply::EngineSnapshot(snapshot) => snapshot.render_payload(),
            IntrospectionReply::ComponentSnapshot(snapshot) => snapshot.render_payload(),
            IntrospectionReply::DeliveryTrace(trace) => trace.render_payload(),
            IntrospectionReply::ComponentTrace(trace) => trace.render_payload(),
            IntrospectionReply::PrototypeWitness(witness) => witness.render_payload(),
            IntrospectionReply::Unimplemented(unimplemented) => unimplemented.render_payload(),
            IntrospectionReply::Denied(denied) => denied.render_payload(),
        };
        Self { body }
    }
}

trait IntrospectionPanePayload {
    fn render_payload(&self) -> String;
}

impl IntrospectionPanePayload for EngineSnapshot {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("EngineSnapshot", self.to_nota())
    }
}

impl IntrospectionPanePayload for ComponentSnapshot {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("ComponentSnapshot", self.to_nota())
    }
}

impl IntrospectionPanePayload for DeliveryTrace {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("DeliveryTrace", self.to_nota())
    }
}

impl IntrospectionPanePayload for ComponentTrace {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("ComponentTrace", self.to_nota())
    }
}

impl IntrospectionPanePayload for PrototypeWitness {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("PrototypeWitness", self.to_nota())
    }
}

impl IntrospectionPanePayload for IntrospectionUnimplemented {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("Unimplemented", self.to_nota())
    }
}

impl IntrospectionPanePayload for IntrospectionDenied {
    fn render_payload(&self) -> String {
        IntrospectionPane::wrap("Denied", self.to_nota())
    }
}
