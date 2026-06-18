use std::fs;
use std::os::unix::net::UnixListener;

use kameo::Actor;
use kameo::actor::{ActorRef, Spawn};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_frame::{NonEmpty, Reply, SubReply};
use signal_mentci::{MentciFrame, MentciFrameBody, MentciReply, MentciRequest};

use crate::configuration::DaemonConfiguration;
use crate::frame_codec::FrameCodec;
use crate::state::State;
use crate::{Error, Result};

#[derive(Debug)]
pub struct Daemon {
    configuration: DaemonConfiguration,
}

#[derive(Debug)]
pub struct StateOwner {
    state: State,
}

#[derive(Debug)]
pub struct ApplyRequest {
    request: MentciRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ApplyReply {
    reply: MentciReply,
}

impl Daemon {
    pub fn from_configuration(configuration: DaemonConfiguration) -> Result<Self> {
        Ok(Self { configuration })
    }

    pub fn run(&self) -> Result<()> {
        let socket_path = self.configuration.socket_path()?.to_path_buf();
        if socket_path.exists() {
            fs::remove_file(&socket_path)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        let runtime = tokio::runtime::Runtime::new()?;
        let state =
            runtime.block_on(async { StateOwner::spawn(StateOwner::new(State::default())) });
        let codec = FrameCodec::new();
        for incoming in listener.incoming() {
            let mut stream = incoming?;
            self.handle_connection(&runtime, &state, &codec, &mut stream)?;
        }
        Ok(())
    }

    fn handle_connection(
        &self,
        runtime: &tokio::runtime::Runtime,
        state: &ActorRef<StateOwner>,
        codec: &FrameCodec,
        stream: &mut std::os::unix::net::UnixStream,
    ) -> Result<()> {
        let frame = codec.read_mentci_frame(stream)?;
        let MentciFrameBody::Request { exchange, request } = frame.into_body() else {
            return Err(Error::ExpectedRequest);
        };
        let request = request.payloads.into_head();
        let reply = runtime
            .block_on(state.ask(ApplyRequest { request }).send())
            .map_err(|error| Error::ActorCall(error.to_string()))?
            .into_reply();
        let frame = MentciFrame::new(MentciFrameBody::Reply {
            exchange,
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        });
        codec.write_mentci_frame(stream, &frame)
    }
}

impl StateOwner {
    pub fn new(state: State) -> Self {
        Self { state }
    }
}

impl Actor for StateOwner {
    type Args = Self;
    type Error = Infallible;

    async fn on_start(
        actor: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(actor)
    }
}

impl Message<ApplyRequest> for StateOwner {
    type Reply = ApplyReply;

    async fn handle(
        &mut self,
        message: ApplyRequest,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        ApplyReply {
            reply: self.state.apply(message.request),
        }
    }
}

impl ApplyReply {
    pub fn into_reply(self) -> MentciReply {
        self.reply
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;

    use meta_signal_mentci::{
        ComponentKind, MentciDaemonConfiguration, NotificationClient, PersonaIdentity,
        PersonaKeyLabel, PersonaName, StandardSocket,
    };
    use signal_frame::{
        ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch,
    };
    use signal_mentci::{
        InterfaceMutation, InterfaceUpdate, MentciFrame, MentciFrameBody, MentciRequest,
        StatusText, UpdateIdentifier,
    };

    use super::*;

    fn configuration() -> DaemonConfiguration {
        DaemonConfiguration::new(MentciDaemonConfiguration::new(
            StandardSocket::unix("/tmp/mentci-test.socket"),
            StandardSocket::unix("/tmp/criome-test.socket"),
            PersonaIdentity::new(
                PersonaName::new("psyche"),
                ComponentKind::Persona,
                PersonaKeyLabel::new("home-verdict"),
            ),
            vec![NotificationClient::StatusBar],
        ))
    }

    fn exchange() -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(1),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    #[test]
    fn connection_handler_returns_signal_reply_frame() {
        let daemon = Daemon::from_configuration(configuration()).expect("daemon");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let state =
            runtime.block_on(async { StateOwner::spawn(StateOwner::new(State::default())) });
        let codec = FrameCodec::new();
        let (mut client, mut server) = UnixStream::pair().expect("stream pair");
        let frame = MentciFrame::new(MentciFrameBody::Request {
            exchange: exchange(),
            request: MentciRequest::PushUpdate(InterfaceUpdate {
                identifier: UpdateIdentifier::new("update-1"),
                mutation: InterfaceMutation::SetStatus(StatusText::new("waiting")),
            })
            .into_request(),
        });

        codec
            .write_mentci_frame(&mut client, &frame)
            .expect("write request");
        daemon
            .handle_connection(&runtime, &state, &codec, &mut server)
            .expect("handle connection");
        let reply = codec
            .read_mentci_frame(&mut client)
            .expect("read reply frame");

        match reply.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { .. } => {}
                Reply::Rejected { reason } => panic!("unexpected rejected reply: {reason:?}"),
            },
            other => panic!("expected reply frame, got {other:?}"),
        }
    }
}
