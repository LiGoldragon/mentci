use mentci::client::{ClientCommand, ClientObservationSession};
use mentci::configuration::DaemonConfiguration;
use mentci::daemon::Daemon;
use meta_signal_mentci::{
    ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
    NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, NonEmpty, Reply, SessionEpoch, SubReply,
};
use signal_mentci::{
    AnswerText, ApprovalDecision, ApprovalQuestion, ApprovalSource, ApprovalVerdict, ContextBody,
    ContextLabel, ExplanationText, InterfaceInterest, InterfaceMutation,
    InterfaceObservationOpened, InterfaceProjection, InterfaceStateObservation, InterfaceUpdate,
    MentciFrame, MentciFrameBody, MentciReply, MentciRequest, NotaEncode, PendingQuestionsView,
    ProjectedInterfaceState, PromptText, QuestionContext, QuestionIdentifier, QuestionProposal,
    RevisionCounter, StatusText, SubscriberName, SubscriptionToken, UpdateIdentifier,
};

fn update_request() -> MentciRequest {
    MentciRequest::PushUpdate(InterfaceUpdate {
        identifier: UpdateIdentifier::new("update-1"),
        mutation: InterfaceMutation::SetStatus(StatusText::new("waiting")),
    })
}

fn daemon_configuration(socket_path: &str, criome_path: &str) -> DaemonConfiguration {
    DaemonConfiguration::new(MentciDaemonConfiguration::new(
        vec![
            ComponentSocket::new(
                ComponentSocketKind::Mentci,
                StandardSocket::unix(socket_path),
            ),
            ComponentSocket::new(
                ComponentSocketKind::MetaCriome,
                StandardSocket::unix(criome_path),
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

fn question_proposal() -> QuestionProposal {
    QuestionProposal::new(
        ApprovalSource::AgentQuestion,
        PromptText::new("approve-spirit-record"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("agent-proposed-answer"),
        vec![QuestionContext {
            label: ContextLabel::new("record"),
            body: ContextBody::new("content-addressed-preimage"),
        }],
    )
}

fn approval_question() -> ApprovalQuestion {
    ApprovalQuestion {
        identifier: QuestionIdentifier::new("question-1"),
        proposal: question_proposal(),
    }
}

fn observation_reply_frame(reply: MentciReply) -> MentciFrame {
    MentciFrame::new(MentciFrameBody::Reply {
        exchange: exchange(),
        reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
    })
}

#[test]
fn client_recognizes_observation_command_atom() {
    let command = ClientCommand::from_arguments(["observe:pending"], "/tmp/unused-mentci.socket");

    assert!(
        command
            .observation_command()
            .expect("observation command")
            .is_some()
    );
}

#[test]
fn client_recognizes_answer_command_atom() {
    let command =
        ClientCommand::from_arguments(["answer:approve:question-1"], "/tmp/unused-mentci.socket");

    let answer = command
        .answer_command()
        .expect("answer command")
        .expect("recognized answer command");
    let frame = answer.request_frame();

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(
                request.payloads().head(),
                &MentciRequest::AnswerQuestion(ApprovalVerdict {
                    question: QuestionIdentifier::new("question-1"),
                    decision: ApprovalDecision::ApproveSuggestedAnswer,
                    answered_by: SubscriberName::new("mentci-cli"),
                })
            );
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}

#[test]
fn client_builds_request_frame_from_inline_nota() {
    let request = update_request();
    let command = ClientCommand::from_arguments([request.to_nota()], "/tmp/unused-mentci.socket");

    let frame = command.request_frame().expect("request frame");

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(request.payloads().head(), &update_request());
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}

#[test]
fn observation_session_builds_request_through_shared_model() {
    let mut session = ClientObservationSession::new(InterfaceInterest::PendingQuestions);

    let frame = session.request_frame().expect("request frame");

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(
                request.payloads().head(),
                &MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
                    subscriber: SubscriberName::new("mentci-cli"),
                    interest: InterfaceInterest::PendingQuestions,
                })
            );
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}

#[test]
fn observation_session_folds_reply_into_shared_model_and_renders_nota() {
    let mut session = ClientObservationSession::new(InterfaceInterest::PendingQuestions);
    let _ = session.request_frame().expect("request frame");
    let reply = MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
        token: SubscriptionToken::new("subscription-1"),
        state: ProjectedInterfaceState {
            revision: RevisionCounter::new(7),
            projection: InterfaceProjection::PendingQuestionsProjection(
                PendingQuestionsView::from_questions(vec![approval_question()]),
            ),
        },
    });

    let rendered = session
        .absorb_frame(observation_reply_frame(reply))
        .expect("rendered observation");

    assert_eq!(rendered.view().approval.pending_count, 1);
    assert_eq!(rendered.view().sockets.len(), 1);
    assert_eq!(
        rendered.reply().origin().label(),
        mentci_lib::RenderOrigin::Reply.label()
    );
    assert!(
        rendered
            .reply()
            .body()
            .contains("InterfaceObservationOpened")
    );

    let mut output = Vec::new();
    rendered
        .write_to(&mut output)
        .expect("write rendered output");
    let text = String::from_utf8(output).expect("utf8 output");
    assert!(text.contains("socket Mentci Connected rev 7"));
    assert!(text.contains("approval pending 1 answered 0 subscriptions 0"));
    assert!(text.contains("reply (InterfaceObservationOpened"));
}

#[test]
fn client_observe_command_reads_live_daemon_through_shared_model() {
    let directory = tempfile::tempdir().expect("tempdir");
    let socket = directory.path().join("mentci.socket");
    let criome = directory.path().join("criome-meta.socket");
    let daemon = Daemon::from_configuration(daemon_configuration(
        socket.to_str().expect("socket utf8"),
        criome.to_str().expect("criome utf8"),
    ))
    .expect("daemon");
    let bound = daemon.bind().expect("bound daemon");
    let command = ClientCommand::from_arguments(["observe:pending"], socket.clone());
    let mut output = Vec::new();

    std::thread::scope(|scope| {
        let server = scope.spawn(|| bound.serve_next().expect("serve observe"));
        command
            .run_with_writer(&mut output)
            .expect("run observe client");
        server.join().expect("join server");
    });

    let text = String::from_utf8(output).expect("utf8 output");
    assert!(text.contains("socket Mentci Connected rev 0"));
    assert!(text.contains("approval pending 0 answered 0 subscriptions 0"));
    assert!(text.contains("reply (InterfaceObservationOpened"));

    bound.shutdown().expect("shutdown daemon");
}

#[test]
fn client_answer_command_answers_live_daemon_question() {
    let directory = tempfile::tempdir().expect("tempdir");
    let socket = directory.path().join("mentci.socket");
    let criome = directory.path().join("criome-meta.socket");
    let daemon = Daemon::from_configuration(daemon_configuration(
        socket.to_str().expect("socket utf8"),
        criome.to_str().expect("criome utf8"),
    ))
    .expect("daemon");
    let bound = daemon.bind().expect("bound daemon");
    let present = ClientCommand::from_arguments(
        [MentciRequest::PresentQuestion(question_proposal()).to_nota()],
        socket.clone(),
    );
    let answer = ClientCommand::from_arguments(["answer:approve:question-1"], socket.clone());
    let observe = ClientCommand::from_arguments(["observe:pending"], socket.clone());
    let mut present_output = Vec::new();
    let mut answer_output = Vec::new();
    let mut observe_output = Vec::new();

    std::thread::scope(|scope| {
        let server = scope.spawn(|| bound.serve_next().expect("serve present"));
        present
            .run_with_writer(&mut present_output)
            .expect("run present client");
        server.join().expect("join present server");

        let server = scope.spawn(|| bound.serve_next().expect("serve answer"));
        answer
            .run_with_writer(&mut answer_output)
            .expect("run answer client");
        server.join().expect("join answer server");

        let server = scope.spawn(|| bound.serve_next().expect("serve observe"));
        observe
            .run_with_writer(&mut observe_output)
            .expect("run observe client");
        server.join().expect("join observe server");
    });

    let answer_text = String::from_utf8(answer_output).expect("utf8 answer output");
    assert!(answer_text.contains("reply (VerdictAccepted"));
    assert!(answer_text.contains("question-1"));
    assert!(answer_text.contains("ApproveSuggestedAnswer"));

    let observe_text = String::from_utf8(observe_output).expect("utf8 observe output");
    assert!(observe_text.contains("approval pending 0 answered 0 subscriptions 0"));
    assert!(observe_text.contains("PendingQuestionsProjection []"));

    bound.shutdown().expect("shutdown daemon");
}

#[test]
fn client_builds_request_frame_from_nota_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("request.nota");
    std::fs::write(&path, update_request().to_nota()).expect("write request");
    let command = ClientCommand::from_arguments(
        [path.to_string_lossy().to_string()],
        "/tmp/unused-mentci.socket",
    );

    let frame = command.request_frame().expect("request frame");

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(request.payloads().head(), &update_request());
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}
