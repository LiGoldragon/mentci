use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use kameo::Actor;
use kameo::actor::{ActorRef, Spawn};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_frame::{NonEmpty, Reply, SubReply};
use signal_mentci::{MentciFrame, MentciFrameBody, MentciReply, MentciRequest};

use crate::configuration::DaemonConfiguration;
use crate::criome_bridge::CriomeApprovalBridge;
use crate::frame_codec::FrameCodec;
use crate::state::State;
use crate::{Error, Result};

#[derive(Debug)]
pub struct Daemon {
    configuration: DaemonConfiguration,
}

pub struct BoundDaemon {
    socket_path: PathBuf,
    listener: UnixListener,
    runtime: tokio::runtime::Runtime,
    state: ActorRef<StateOwner>,
    criome_bridge: CriomeApprovalBridge,
    codec: FrameCodec,
}

#[derive(Debug)]
pub struct StateOwner {
    state: State,
}

#[derive(Debug)]
pub struct ApplyRequest {
    request: MentciRequest,
    parked_authorizations: Vec<signal_criome::ParkedAuthorization>,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ApplyReply {
    reply: MentciReply,
}

impl Daemon {
    pub fn from_configuration(configuration: DaemonConfiguration) -> Result<Self> {
        Ok(Self { configuration })
    }

    pub fn bind(&self) -> Result<BoundDaemon> {
        let socket_path = self.configuration.socket_path()?.to_path_buf();
        if socket_path.exists() {
            fs::remove_file(&socket_path)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
        let runtime = tokio::runtime::Runtime::new()?;
        let criome_bridge =
            CriomeApprovalBridge::new(self.configuration.criome_meta_socket_path()?);
        let state =
            runtime.block_on(async { StateOwner::spawn(StateOwner::new(State::default())) });
        let codec = FrameCodec::new();
        Ok(BoundDaemon {
            socket_path,
            listener,
            runtime,
            state,
            criome_bridge,
            codec,
        })
    }

    pub fn run(&self) -> Result<()> {
        self.bind()?.serve_forever()
    }
}

impl BoundDaemon {
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn serve_forever(self) -> Result<()> {
        loop {
            let (mut stream, _address) = self.listener.accept()?;
            self.handle_connection(&mut stream)?;
        }
    }

    pub fn serve_next(&self) -> Result<()> {
        let (mut stream, _address) = self.listener.accept()?;
        self.handle_connection(&mut stream)
    }

    pub fn shutdown(self) -> Result<()> {
        let _ = fs::remove_file(&self.socket_path);
        Ok(())
    }

    fn handle_connection(&self, stream: &mut std::os::unix::net::UnixStream) -> Result<()> {
        let frame = self.codec.read_mentci_frame(stream)?;
        let MentciFrameBody::Request { exchange, request } = frame.into_body() else {
            return Err(Error::ExpectedRequest);
        };
        let request = request.payloads.into_head();
        let parked_authorizations = self.parked_authorizations_for_request(&request);
        let reply = self
            .runtime
            .block_on(
                self.state
                    .ask(ApplyRequest {
                        request,
                        parked_authorizations,
                    })
                    .send(),
            )
            .map_err(|error| Error::ActorCall(error.to_string()))?
            .into_reply();
        let frame = MentciFrame::new(MentciFrameBody::Reply {
            exchange,
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        });
        self.codec.write_mentci_frame(stream, &frame)
    }

    fn parked_authorizations_for_request(
        &self,
        request: &MentciRequest,
    ) -> Vec<signal_criome::ParkedAuthorization> {
        if !matches!(request, MentciRequest::ObserveInterfaceState(_)) {
            return Vec::new();
        }
        self.criome_bridge
            .parked_authorizations()
            .map(|snapshot| snapshot.into_parked())
            .unwrap_or_default()
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
        self.state
            .absorb_criome_parked_authorizations(message.parked_authorizations);
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
        ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
        NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
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
            vec![
                ComponentSocket::new(
                    ComponentSocketKind::Mentci,
                    StandardSocket::unix("/tmp/mentci-test.socket"),
                ),
                ComponentSocket::new(
                    ComponentSocketKind::MetaCriome,
                    StandardSocket::unix("/tmp/criome-test.socket"),
                ),
            ],
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
        let bound = daemon.bind().expect("bound daemon");
        let (mut client, mut server) = UnixStream::pair().expect("stream pair");
        let frame = MentciFrame::new(MentciFrameBody::Request {
            exchange: exchange(),
            request: MentciRequest::PushUpdate(InterfaceUpdate {
                identifier: UpdateIdentifier::new("update-1"),
                mutation: InterfaceMutation::SetStatus(StatusText::new("waiting")),
            })
            .into_request(),
        });

        bound
            .codec
            .write_mentci_frame(&mut client, &frame)
            .expect("write request");
        bound
            .handle_connection(&mut server)
            .expect("handle connection");
        let reply = bound
            .codec
            .read_mentci_frame(&mut client)
            .expect("read reply frame");

        match reply.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { .. } => {}
                Reply::Rejected { reason } => panic!("unexpected rejected reply: {reason:?}"),
            },
            other => panic!("expected reply frame, got {other:?}"),
        }
        bound.shutdown().expect("shutdown daemon");
    }
}
