use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use mentci_lib::{
    Cmd, ComponentSocketKind, EngineEvent, ObservationModel, ObservationView, RenderNota,
    RenderOrigin, RenderedObject, SocketLiveness, UserEvent,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use signal_mentci::{
    ApprovalDecision, ApprovalVerdict, InterfaceInterest, MentciFrame, MentciFrameBody,
    MentciReply, MentciRequest, NotaSource, QuestionIdentifier, SubscriberName,
};

use crate::frame_codec::FrameCodec;
use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCommand {
    arguments: Vec<String>,
    socket_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientObservationCommand {
    interest: InterfaceInterest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientAnswerCommand {
    verdict: ApprovalVerdict,
}

#[derive(Clone, Debug)]
pub struct ClientObservationSession {
    model: ObservationModel,
    interest: InterfaceInterest,
}

#[derive(Clone, Debug)]
pub struct ClientObservationRender {
    view: ObservationView,
    reply: RenderedObject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientReplyRender {
    reply: RenderedObject,
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
        let mut stdout = io::stdout().lock();
        self.run_with_writer(&mut stdout)
    }

    pub fn run_with_writer(&self, writer: &mut impl Write) -> Result<()> {
        if let Some(command) = self.observation_command()? {
            return command.run(&self.socket_path, writer);
        }
        if let Some(command) = self.answer_command()? {
            return command.run(&self.socket_path, writer);
        }
        let frame = self.request_frame()?;
        let codec = FrameCodec::new();
        let mut stream = UnixStream::connect(&self.socket_path)?;
        codec.write_mentci_frame(&mut stream, &frame)?;
        let reply = codec.read_mentci_frame(&mut stream)?;
        let bytes = reply.encode_length_prefixed()?;
        writer.write_all(&bytes)?;
        Ok(())
    }

    fn input_argument(&self) -> Result<&str> {
        match self.arguments.as_slice() {
            [argument] => Ok(argument.as_str()),
            _ => Err(Error::ClientArgumentCount),
        }
    }

    pub fn observation_command(&self) -> Result<Option<ClientObservationCommand>> {
        let argument = self.input_argument()?;
        Ok(ClientObservationCommand::from_argument(argument))
    }

    pub fn answer_command(&self) -> Result<Option<ClientAnswerCommand>> {
        let argument = self.input_argument()?;
        Ok(ClientAnswerCommand::from_argument(argument))
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

impl ClientObservationCommand {
    pub fn from_argument(argument: &str) -> Option<Self> {
        let interest = match argument {
            "observe" | "observe:full" => InterfaceInterest::FullInterfaceState,
            "observe:pending" => InterfaceInterest::PendingQuestions,
            "observe:status" => InterfaceInterest::StatusOnly,
            "observe:notifications" => InterfaceInterest::Notifications,
            _ => return None,
        };
        Some(Self { interest })
    }

    fn run(&self, socket_path: &Path, writer: &mut impl Write) -> Result<()> {
        let mut session = ClientObservationSession::new(self.interest);
        let frame = session.request_frame()?;
        let codec = FrameCodec::new();
        let mut stream = UnixStream::connect(socket_path)?;
        codec.write_mentci_frame(&mut stream, &frame)?;
        let reply = codec.read_mentci_frame(&mut stream)?;
        let rendered = session.absorb_frame(reply)?;
        rendered.write_to(writer)
    }
}

impl ClientAnswerCommand {
    pub fn from_argument(argument: &str) -> Option<Self> {
        if let Some(question) = argument.strip_prefix("answer:approve:") {
            return Some(Self::new(
                question,
                ApprovalDecision::ApproveSuggestedAnswer,
            ));
        }
        if let Some(question) = argument.strip_prefix("answer:reject:") {
            return Some(Self::new(question, ApprovalDecision::Reject));
        }
        if let Some(question) = argument.strip_prefix("answer:defer:") {
            return Some(Self::new(question, ApprovalDecision::Defer));
        }
        None
    }

    pub fn request_frame(&self) -> MentciFrame {
        MentciFrame::new(MentciFrameBody::Request {
            exchange: ClientObservationSession::exchange(),
            request: MentciRequest::AnswerQuestion(self.verdict.clone()).into_request(),
        })
    }

    fn new(question: &str, decision: ApprovalDecision) -> Self {
        Self {
            verdict: ApprovalVerdict {
                question: QuestionIdentifier::new(question),
                decision,
                answered_by: SubscriberName::new("mentci-cli"),
            },
        }
    }

    fn run(&self, socket_path: &Path, writer: &mut impl Write) -> Result<()> {
        let frame = self.request_frame();
        let codec = FrameCodec::new();
        let mut stream = UnixStream::connect(socket_path)?;
        codec.write_mentci_frame(&mut stream, &frame)?;
        let reply = codec.read_mentci_frame(&mut stream)?;
        ClientReplyRender::from_frame(reply)?.write_to(writer)
    }
}

impl ClientObservationSession {
    pub fn new(interest: InterfaceInterest) -> Self {
        Self {
            model: ObservationModel::new(SubscriberName::new("mentci-cli")),
            interest,
        }
    }

    pub fn request_frame(&mut self) -> Result<MentciFrame> {
        let commands = self.model.on_user_event(UserEvent::Observe {
            socket: ComponentSocketKind::Mentci,
            interest: self.interest,
        });
        let Some(Cmd::SendRequest { request, .. }) = commands.into_iter().next() else {
            return Err(Error::ClientObservationCommandUnavailable);
        };
        Ok(MentciFrame::new(MentciFrameBody::Request {
            exchange: Self::exchange(),
            request: request.into_request(),
        }))
    }

    pub fn absorb_frame(&mut self, frame: MentciFrame) -> Result<ClientObservationRender> {
        let reply = Self::reply_output(frame)?;
        let rendered = reply.render_nota(RenderOrigin::Reply);
        match &reply {
            MentciReply::InterfaceObservationOpened(opened) => {
                self.model.on_engine_event(EngineEvent::ObservationOpened {
                    socket: ComponentSocketKind::Mentci,
                    opened: opened.clone(),
                });
            }
            MentciReply::Rejection(rejection) => {
                self.model.on_engine_event(EngineEvent::Rejected {
                    socket: ComponentSocketKind::Mentci,
                    rejection: rejection.clone(),
                });
            }
            _ => {
                self.model.on_engine_event(EngineEvent::ConnectionChanged {
                    socket: ComponentSocketKind::Mentci,
                    liveness: SocketLiveness::Connected,
                });
            }
        }
        Ok(ClientObservationRender {
            view: self.model.view(),
            reply: rendered,
        })
    }

    fn reply_output(frame: MentciFrame) -> Result<MentciReply> {
        match frame.into_body() {
            MentciFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(output) => Ok(output),
                    other => Err(Error::UnexpectedMentciReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(Error::UnexpectedMentciReply(format!(
                    "rejected: {reason:?}"
                ))),
            },
            other => Err(Error::UnexpectedMentciReply(format!("{other:?}"))),
        }
    }

    fn exchange() -> ExchangeIdentifier {
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }
}

impl ClientReplyRender {
    pub fn from_frame(frame: MentciFrame) -> Result<Self> {
        let reply = ClientObservationSession::reply_output(frame)?;
        Ok(Self {
            reply: reply.render_nota(RenderOrigin::Reply),
        })
    }

    pub fn reply(&self) -> &RenderedObject {
        &self.reply
    }

    pub fn write_to(&self, writer: &mut impl Write) -> Result<()> {
        writeln!(
            writer,
            "{} {}",
            self.reply.origin().label(),
            self.reply.body()
        )?;
        Ok(())
    }
}

impl ClientObservationRender {
    pub fn view(&self) -> &ObservationView {
        &self.view
    }

    pub fn reply(&self) -> &RenderedObject {
        &self.reply
    }

    pub fn write_to(&self, writer: &mut impl Write) -> Result<()> {
        for socket in &self.view.sockets {
            write!(writer, "socket {}", socket.socket.as_str())?;
            write!(writer, " {:?}", socket.liveness)?;
            if let Some(revision) = &socket.revision {
                write!(writer, " rev {}", revision.value())?;
            }
            writeln!(writer)?;
        }
        writeln!(
            writer,
            "approval pending {} answered {} subscriptions {}",
            self.view.approval.pending_count,
            self.view.approval.answered_count,
            self.view.approval.subscription_count
        )?;
        for pane in &self.view.panes {
            writeln!(writer, "pane {}", pane.pane.as_str())?;
            writeln!(writer, "{}", pane.body.as_str())?;
        }
        writeln!(
            writer,
            "{} {}",
            self.reply.origin().label(),
            self.reply.body()
        )?;
        Ok(())
    }
}
