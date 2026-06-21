use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use kameo::Actor;
use kameo::actor::{ActorRef, Spawn};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_frame::{NonEmpty, Reply, SubReply};
use signal_mentci::{
    MentciFrame, MentciFrameBody, MentciReply, MentciRequest, Rejection, RejectionReason,
};

use crate::configuration::DaemonConfiguration;
use crate::criome_bridge::CriomeApprovalBridge;
use crate::frame_codec::FrameCodec;
use crate::introspection_bridge::{IntrospectionBridge, IntrospectionPane};
use crate::state::{State, StateApplication, StateApplicationContext};
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
    criome_bridge: Option<CriomeApprovalBridge>,
    introspection_bridge: Option<IntrospectionBridge>,
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
    introspection_pane: Option<signal_mentci::PaneContent>,
    context: StateApplicationContext,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ApplyReply {
    reply: MentciReply,
    criome_verdict: Option<mentci_lib::CriomeVerdict>,
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
        let criome_bridge = self
            .configuration
            .criome_meta_socket_path()
            .ok()
            .map(CriomeApprovalBridge::new);
        let introspection_bridge = self
            .configuration
            .introspect_socket_path()
            .ok()
            .map(IntrospectionBridge::new);
        let state =
            runtime.block_on(async { StateOwner::spawn(StateOwner::new(State::default())) });
        let codec = FrameCodec::new();
        Ok(BoundDaemon {
            socket_path,
            listener,
            runtime,
            state,
            criome_bridge,
            introspection_bridge,
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
        let introspection_pane = self.introspection_pane_for_request(&request);
        let context = self.application_context();
        let applied = self
            .runtime
            .block_on(
                self.state
                    .ask(ApplyRequest {
                        request,
                        parked_authorizations,
                        introspection_pane,
                        context,
                    })
                    .send(),
            )
            .map_err(|error| Error::ActorCall(error.to_string()))?
            .into_application();
        let (mut reply, criome_verdict) = applied.into_parts();
        if let Some(verdict) = criome_verdict {
            let Some(bridge) = &self.criome_bridge else {
                return Err(Error::MissingComponentSocket {
                    kind: meta_signal_mentci::ComponentSocketKind::MetaCriome,
                });
            };
            let submission = bridge.submit_criome_verdict(&verdict)?;
            if !submission.is_recorded() {
                reply =
                    MentciReply::Rejection(Rejection::new(RejectionReason::UnauthorizedProjection));
            }
        }
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
        let Some(bridge) = &self.criome_bridge else {
            return Vec::new();
        };
        bridge
            .parked_authorizations()
            .map(|snapshot| snapshot.into_parked())
            .unwrap_or_default()
    }

    fn introspection_pane_for_request(
        &self,
        request: &MentciRequest,
    ) -> Option<signal_mentci::PaneContent> {
        if !matches!(request, MentciRequest::ObserveInterfaceState(_)) {
            return None;
        }
        self.introspection_bridge
            .as_ref()
            .map(|bridge| match bridge.prototype_witness_pane() {
                Ok(pane) => pane,
                Err(error) => IntrospectionPane::from_error(&error),
            })
            .map(IntrospectionPane::into_content)
    }

    fn application_context(&self) -> StateApplicationContext {
        if self.criome_bridge.is_some() {
            StateApplicationContext::write_enabled()
        } else {
            StateApplicationContext::read_only()
        }
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
        if let Some(pane) = message.introspection_pane {
            self.state.refresh_pane(pane);
        }
        ApplyReply::from_application(
            self.state
                .apply_with_context(message.request, message.context),
        )
    }
}

impl ApplyReply {
    pub fn from_application(application: StateApplication) -> Self {
        let (reply, criome_verdict) = application.into_parts();
        Self {
            reply,
            criome_verdict,
        }
    }

    pub fn into_application(self) -> StateApplication {
        StateApplication::with_criome_verdict(self.reply, self.criome_verdict)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::thread;

    use meta_signal_mentci::{
        ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
        NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
    };
    use signal_frame::{
        ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch,
    };
    use signal_introspect::{
        IntrospectionFrame, IntrospectionFrameBody, IntrospectionReply, IntrospectionRequest,
        PrototypeWitness, PrototypeWitnessQuery,
    };
    use signal_mentci::{
        InterfaceInterest, InterfaceMutation, InterfaceObservationOpened, InterfaceProjection,
        InterfaceStateObservation, InterfaceUpdate, MentciFrame, MentciFrameBody, MentciRequest,
        StatusText, SubscriberName, UpdateIdentifier,
    };
    use signal_persona::EngineIdentifier;

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

    fn read_only_configuration() -> DaemonConfiguration {
        DaemonConfiguration::new(MentciDaemonConfiguration::new(
            vec![ComponentSocket::new(
                ComponentSocketKind::Mentci,
                StandardSocket::unix("/tmp/mentci-read-only-test.socket"),
            )],
            PersonaIdentity::new(
                PersonaName::new("psyche"),
                ComponentKind::Persona,
                PersonaKeyLabel::new("home-verdict"),
            ),
            vec![NotificationClient::StatusBar],
        ))
    }

    fn introspect_configuration(directory: &Path, introspect_socket: &Path) -> DaemonConfiguration {
        DaemonConfiguration::new(MentciDaemonConfiguration::new(
            vec![
                ComponentSocket::new(
                    ComponentSocketKind::Mentci,
                    StandardSocket::unix(directory.join("mentci.socket").display().to_string()),
                ),
                ComponentSocket::new(
                    ComponentSocketKind::Introspect,
                    StandardSocket::unix(introspect_socket.display().to_string()),
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

    #[test]
    fn observe_projects_introspection_witness_into_daemon_pane() {
        let directory = tempfile::tempdir().expect("tempdir");
        let introspect_socket = directory.path().join("introspect.socket");
        let listener = std::os::unix::net::UnixListener::bind(&introspect_socket)
            .expect("bind fake introspect");
        let server = thread::spawn(move || {
            let (mut stream, _address) = listener.accept().expect("accept introspect");
            let codec = FrameCodec::new();
            let frame = codec
                .read_introspection_frame(&mut stream)
                .expect("read introspect request");
            match frame.into_body() {
                IntrospectionFrameBody::Request { request, exchange } => {
                    assert!(matches!(
                        request.payloads.into_head(),
                        IntrospectionRequest::PrototypeWitness(PrototypeWitnessQuery { .. })
                    ));
                    let reply = IntrospectionFrame::new(IntrospectionFrameBody::Reply {
                        exchange,
                        reply: Reply::committed(NonEmpty::single(SubReply::Ok(
                            IntrospectionReply::PrototypeWitness(PrototypeWitness {
                                engine: EngineIdentifier::new("prototype"),
                                manager_seen: None,
                                router_seen: None,
                                terminal_seen: None,
                                delivery_status: None,
                            }),
                        ))),
                    });
                    codec
                        .write_introspection_frame(&mut stream, &reply)
                        .expect("write introspect reply");
                }
                other => panic!("expected introspect request, got {other:?}"),
            }
        });
        let daemon = Daemon::from_configuration(introspect_configuration(
            directory.path(),
            &introspect_socket,
        ))
        .expect("daemon");
        let bound = daemon.bind().expect("bound daemon");
        let (mut client, mut server_stream) = UnixStream::pair().expect("stream pair");
        let frame = MentciFrame::new(MentciFrameBody::Request {
            exchange: exchange(),
            request: MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
                subscriber: SubscriberName::new("test-client"),
                interest: InterfaceInterest::FullInterfaceState,
            })
            .into_request(),
        });

        bound
            .codec
            .write_mentci_frame(&mut client, &frame)
            .expect("write request");
        bound
            .handle_connection(&mut server_stream)
            .expect("handle connection");
        let reply = bound
            .codec
            .read_mentci_frame(&mut client)
            .expect("read reply frame");

        match reply.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(MentciReply::InterfaceObservationOpened(
                        InterfaceObservationOpened { state, .. },
                    )) => match state.projection {
                        InterfaceProjection::FullProjection(full) => {
                            assert_eq!(full.panes().len(), 1);
                            assert_eq!(full.panes()[0].pane.as_str(), "introspect");
                            assert!(full.panes()[0].body.as_str().contains("PrototypeWitness"));
                        }
                        other => panic!("expected full projection, got {other:?}"),
                    },
                    other => panic!("expected observation reply, got {other:?}"),
                },
                Reply::Rejected { reason } => panic!("unexpected rejected reply: {reason:?}"),
            },
            other => panic!("expected reply frame, got {other:?}"),
        }
        server.join().expect("introspect server joined");
        bound.shutdown().expect("shutdown daemon");
    }

    #[test]
    fn daemon_binds_without_criome_meta_socket_for_read_only_mode() {
        let daemon = Daemon::from_configuration(read_only_configuration()).expect("daemon");
        let bound = daemon.bind().expect("bound read-only daemon");

        assert!(bound.criome_bridge.is_none());

        bound.shutdown().expect("shutdown daemon");
    }
}
