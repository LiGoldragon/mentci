use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

use criome::daemon::{CriomeDaemon, CriomeDaemonConfiguration};
use criome::tables::StoreLocation;
use criome::transport::CriomeClient;
use mentci::configuration::DaemonConfiguration;
use mentci::criome_bridge::{CriomeApprovalBridge, CriomeApprovalSubmission};
use mentci::daemon::Daemon;
use mentci::frame_codec::FrameCodec;
use mentci_lib::CriomeVerdict;
use meta_signal_mentci::{
    ComponentKind as MentciComponentKind, ComponentSocket, ComponentSocketKind,
    MentciDaemonConfiguration, NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName,
    StandardSocket,
};
use signal_criome::{
    AttestedMoment, AttestedMomentProposition, AuthorizationEvaluation, AuthorizationMode,
    AuthorizationRequestSlot, AuthorizedObjectInterest, AuthorizedObjectKind,
    AuthorizedObjectObservation, ComponentKind, CriomeReply, CriomeRequest, EvaluationDecision,
    Evidence, Identity, OperationDigest, ParkedAuthorization, RequiredSignatureThreshold,
    TimeWindow, TimestampNanos,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use signal_mentci::{
    ApprovalDecision, ApprovalQuestion, ApprovalSource, ApprovalVerdict, InterfaceInterest,
    InterfaceObservationOpened, InterfaceProjection, MentciFrame, MentciFrameBody, MentciReply,
    MentciRequest, PendingQuestionsView, ProjectedInterfaceState, QuestionIdentifier, Rejection,
    RejectionReason, RevisionCounter, SubscriberName, SubscriptionToken,
};

fn fixture_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("mentci-criome-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create fixture dir");
    path
}

fn wait_for_socket(socket: &Path) {
    for _ in 0..100 {
        if socket.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("socket did not appear: {}", socket.display());
}

fn exchange() -> ExchangeIdentifier {
    ExchangeIdentifier::new(
        SessionEpoch::new(1),
        ExchangeLane::Connector,
        LaneSequence::first(),
    )
}

fn operation_digest(seed: &[u8]) -> OperationDigest {
    OperationDigest::from_bytes(seed)
}

fn unproven_evidence(seed: &[u8]) -> Evidence {
    let operation = operation_digest(seed);
    Evidence::new(
        ComponentKind::Spirit,
        operation,
        AttestedMoment::new(
            AttestedMomentProposition::new(
                TimeWindow {
                    opens_at: TimestampNanos::new(1),
                    closes_at: TimestampNanos::new(2),
                },
                RequiredSignatureThreshold::new(1),
                Vec::new(),
            ),
            Vec::new(),
        ),
        Vec::new(),
        Vec::new(),
    )
}

fn mentci_configuration(socket: &Path, criome_meta_socket: &Path) -> DaemonConfiguration {
    DaemonConfiguration::new(MentciDaemonConfiguration::new(
        vec![
            ComponentSocket::new(
                ComponentSocketKind::Mentci,
                StandardSocket::unix(socket.display().to_string()),
            ),
            ComponentSocket::new(
                ComponentSocketKind::MetaCriome,
                StandardSocket::unix(criome_meta_socket.display().to_string()),
            ),
        ],
        PersonaIdentity::new(
            PersonaName::new("psyche"),
            MentciComponentKind::Persona,
            PersonaKeyLabel::new("home-verdict"),
        ),
        vec![NotificationClient::StatusBar],
    ))
}

fn send_mentci(socket: &Path, request: MentciRequest) -> MentciReply {
    let codec = FrameCodec::new();
    let mut stream = UnixStream::connect(socket).expect("connect mentci");
    let frame = MentciFrame::new(MentciFrameBody::Request {
        exchange: exchange(),
        request: request.into_request(),
    });
    codec
        .write_mentci_frame(&mut stream, &frame)
        .expect("write mentci request");
    let reply = codec
        .read_mentci_frame(&mut stream)
        .expect("read mentci reply");
    match reply.into_body() {
        MentciFrameBody::Reply { reply, .. } => match reply {
            Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                SubReply::Ok(payload) => payload,
                other => panic!("expected Mentci Ok reply, got {other:?}"),
            },
            Reply::Rejected { reason } => panic!("unexpected Mentci rejection: {reason:?}"),
        },
        other => panic!("expected Mentci reply frame, got {other:?}"),
    }
}

fn criome_escalation_question(slot: AuthorizationRequestSlot) -> signal_mentci::QuestionProposal {
    signal_mentci::QuestionProposal::new(
        ApprovalSource::CriomeEscalation(slot.clone()),
        signal_mentci::PromptText::new("approve-criome-request"),
        Some(signal_mentci::AnswerText::new("approve")),
        signal_mentci::ExplanationText::new("criome escalation"),
        vec![signal_mentci::QuestionContext {
            label: signal_mentci::ContextLabel::new("slot"),
            body: signal_mentci::ContextBody::new(slot.as_str()),
        }],
    )
}

#[test]
fn criome_submission_requires_recorded_matching_output() {
    let verdict = CriomeVerdict::from_decision(
        AuthorizationRequestSlot::new("slot-1"),
        ApprovalDecision::ApproveSuggestedAnswer,
    );
    let recorded = CriomeApprovalSubmission::new(
        verdict.clone(),
        meta_signal_criome::Output::AuthorizationApprovalRecorded(
            meta_signal_criome::AuthorizationApprovalRecorded {
                request_slot: AuthorizationRequestSlot::new("slot-1"),
                decision: meta_signal_criome::AuthorizationApprovalDecision::Approve,
            },
        ),
    );
    assert!(recorded.is_recorded());

    let wrong_slot = CriomeApprovalSubmission::new(
        verdict.clone(),
        meta_signal_criome::Output::AuthorizationApprovalRecorded(
            meta_signal_criome::AuthorizationApprovalRecorded {
                request_slot: AuthorizationRequestSlot::new("slot-2"),
                decision: meta_signal_criome::AuthorizationApprovalDecision::Approve,
            },
        ),
    );
    assert!(!wrong_slot.is_recorded());

    let wrong_decision = CriomeApprovalSubmission::new(
        verdict.clone(),
        meta_signal_criome::Output::AuthorizationApprovalRecorded(
            meta_signal_criome::AuthorizationApprovalRecorded {
                request_slot: AuthorizationRequestSlot::new("slot-1"),
                decision: meta_signal_criome::AuthorizationApprovalDecision::Reject,
            },
        ),
    );
    assert!(!wrong_decision.is_recorded());

    let not_recorded = CriomeApprovalSubmission::new(
        verdict,
        meta_signal_criome::Output::RequestUnimplemented(
            meta_signal_criome::RequestUnimplemented {
                operation: meta_signal_criome::OperationKind::SubmitAuthorizationApproval,
                reason: meta_signal_criome::UnimplementedReason::DependencyNotReady,
            },
        ),
    );
    assert!(!not_recorded.is_recorded());
}

#[test]
fn mentci_rejects_verdict_when_criome_does_not_record_it() {
    let workspace = fixture_path("missing-slot");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let mentci_socket = workspace.join("mentci.sock");
    let store = StoreLocation::new(workspace.join("criome.sema"));
    let criome = CriomeDaemon::new(&criome_socket, store)
        .with_meta_socket(&criome_meta_socket)
        .bind()
        .expect("bind criome");
    let mentci =
        Daemon::from_configuration(mentci_configuration(&mentci_socket, &criome_meta_socket))
            .expect("mentci daemon")
            .bind()
            .expect("bind mentci");
    wait_for_socket(&criome_meta_socket);
    wait_for_socket(&mentci_socket);

    let missing_slot = AuthorizationRequestSlot::new("999");
    let presented = thread::scope(|scope| {
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve present"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::PresentQuestion(criome_escalation_question(missing_slot)),
        );
        mentci_server.join().expect("join present server");
        reply
    });
    assert!(matches!(presented, MentciReply::QuestionPresented(_)));

    let rejected = thread::scope(|scope| {
        let criome_meta_server =
            scope.spawn(|| criome.serve_next_meta().expect("serve missing approval"));
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve answer"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::AnswerQuestion(ApprovalVerdict {
                question: QuestionIdentifier::new("question-1"),
                decision: ApprovalDecision::ApproveSuggestedAnswer,
                answered_by: SubscriberName::new("psyche"),
            }),
        );
        let meta_reply = criome_meta_server.join().expect("join missing approval");
        let submitted = CriomeVerdict::from_decision(
            AuthorizationRequestSlot::new("999"),
            ApprovalDecision::ApproveSuggestedAnswer,
        );
        let submission = CriomeApprovalSubmission::new(submitted, meta_reply);
        assert!(!submission.is_recorded());
        mentci_server.join().expect("join answer server");
        reply
    });

    assert_eq!(
        rejected,
        MentciReply::Rejection(Rejection::new(RejectionReason::UnauthorizedProjection))
    );

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
}

#[test]
fn mentci_bridge_configures_criome_auto_approve_over_meta_socket() {
    let workspace = fixture_path("configured-auto-approve");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let store = StoreLocation::new(workspace.join("criome.sema"));
    let criome = CriomeDaemon::new(&criome_socket, store.clone())
        .with_meta_socket(&criome_meta_socket)
        .bind()
        .expect("bind criome");
    wait_for_socket(&criome_socket);
    wait_for_socket(&criome_meta_socket);

    let bridge = CriomeApprovalBridge::new(&criome_meta_socket);
    let configured = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next_meta().expect("serve meta configure"));
        let configuration = CriomeDaemonConfiguration::new(
            criome_socket.display().to_string(),
            store.as_path().display().to_string(),
        )
        .with_meta_socket_path(criome_meta_socket.display().to_string())
        .with_authorization_mode(AuthorizationMode::AutoApprove);
        let reply = bridge.configure(configuration).expect("configure criome");
        assert_eq!(server.join().expect("join meta configure server"), reply);
        reply
    });
    let meta_signal_criome::Output::Configured(configured) = configured else {
        panic!("expected Configured, got {configured:?}");
    };
    assert_eq!(configured.payload().value(), 1);

    let evidence = unproven_evidence(b"mentci-configured-auto-approved-head");
    let object = signal_criome::AuthorizedObjectReference {
        component: ComponentKind::Spirit,
        digest: evidence.operation.object_digest().clone(),
        kind: AuthorizedObjectKind::Head,
    };
    let contract = signal_criome::ContractDigest::from_bytes(b"mentci-auto-approve-contract");
    let evaluation = AuthorizationEvaluation {
        contract: contract.clone(),
        object: object.clone(),
        evidence: evidence.clone(),
    };

    let approved = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve auto approve"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::EvaluateAuthorization(evaluation))
            .expect("evaluate auto approve");
        assert_eq!(server.join().expect("join auto approve server"), reply);
        reply
    });
    let CriomeReply::AuthorizationEvaluated(approved) = approved else {
        panic!("expected AuthorizationEvaluated, got {approved:?}");
    };
    assert_eq!(approved.decision, EvaluationDecision::Authorized);

    let snapshot = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve authorized observation"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::ObserveAuthorizedObjects(
                AuthorizedObjectObservation {
                    subscriber: Identity::agent("mentci-auto-approve-observer".to_string()),
                    interest: AuthorizedObjectInterest::Component(ComponentKind::Spirit),
                },
            ))
            .expect("observe authorized objects");
        assert_eq!(server.join().expect("join observation server"), reply);
        reply
    });
    let CriomeReply::AuthorizedObjectUpdateSnapshot(snapshot) = snapshot else {
        panic!("expected AuthorizedObjectUpdateSnapshot, got {snapshot:?}");
    };
    let updates = snapshot.into_updates();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].object, object);
    assert_eq!(updates[0].contract, contract);
    assert_eq!(updates[0].decision, EvaluationDecision::Authorized);
    assert_eq!(updates[0].stamp, evidence.stamp);

    criome.shutdown().expect("shutdown criome");
}

#[test]
fn mentci_observe_picks_up_parked_criome_client_approval_request() {
    let workspace = fixture_path("picked-up");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let mentci_socket = workspace.join("mentci.sock");
    let store = StoreLocation::new(workspace.join("criome.sema"));
    let criome = CriomeDaemon::new(&criome_socket, store.clone())
        .with_meta_socket(&criome_meta_socket)
        .bind()
        .expect("bind criome");
    let mentci =
        Daemon::from_configuration(mentci_configuration(&mentci_socket, &criome_meta_socket))
            .expect("mentci daemon")
            .bind()
            .expect("bind mentci");
    wait_for_socket(&criome_socket);
    wait_for_socket(&criome_meta_socket);
    wait_for_socket(&mentci_socket);

    let bridge = CriomeApprovalBridge::new(&criome_meta_socket);
    thread::scope(|scope| {
        let server = scope.spawn(|| {
            criome
                .serve_next_meta()
                .expect("serve client approval mode")
        });
        let configuration = CriomeDaemonConfiguration::new(
            criome_socket.display().to_string(),
            store.as_path().display().to_string(),
        )
        .with_meta_socket_path(criome_meta_socket.display().to_string())
        .with_authorization_mode(AuthorizationMode::ClientApproval);
        let reply = bridge.configure(configuration).expect("configure criome");
        assert_eq!(server.join().expect("join meta configure server"), reply);
    });

    let evidence = unproven_evidence(b"mentci-picked-up-head");
    let object = signal_criome::AuthorizedObjectReference {
        component: ComponentKind::Spirit,
        digest: evidence.operation.object_digest().clone(),
        kind: AuthorizedObjectKind::Head,
    };
    let contract = signal_criome::ContractDigest::from_bytes(b"mentci-picked-up-contract");
    let evaluation = AuthorizationEvaluation {
        contract,
        object,
        evidence,
    };

    let pending = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve client approval park"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::EvaluateAuthorization(evaluation.clone()))
            .expect("evaluate authorization");
        assert_eq!(server.join().expect("join park server"), reply);
        reply
    });
    let CriomeReply::AuthorizationPending(pending) = pending else {
        panic!("expected AuthorizationPending, got {pending:?}");
    };

    let observed = thread::scope(|scope| {
        let criome_meta_server =
            scope.spawn(|| criome.serve_next_meta().expect("serve parked list"));
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve observe"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
                subscriber: SubscriberName::new("mentci-egui"),
                interest: InterfaceInterest::PendingQuestions,
            }),
        );
        assert!(matches!(
            criome_meta_server.join().expect("join parked list"),
            meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
        ));
        mentci_server.join().expect("join observe server");
        reply
    });

    let parked = ParkedAuthorization {
        request_slot: pending.request_slot,
        evaluation,
    };
    let expected_question = ApprovalQuestion {
        identifier: QuestionIdentifier::new("question-1"),
        proposal: mentci::state::CriomeParkedApproval::new(parked).into_question_proposal(),
    };
    assert_eq!(
        observed,
        MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
            token: SubscriptionToken::new("subscription-1"),
            state: ProjectedInterfaceState {
                revision: RevisionCounter::new(1),
                projection: InterfaceProjection::PendingQuestionsProjection(
                    PendingQuestionsView::from_questions(vec![expected_question]),
                ),
            },
        })
    );

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
}

#[test]
fn mentci_closed_verdict_approves_criome_escalation_over_meta_socket() {
    let workspace = fixture_path("approved");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let mentci_socket = workspace.join("mentci.sock");
    let store = StoreLocation::new(workspace.join("criome.sema"));
    let criome = CriomeDaemon::new(&criome_socket, store.clone())
        .with_meta_socket(&criome_meta_socket)
        .bind()
        .expect("bind criome");
    let mentci =
        Daemon::from_configuration(mentci_configuration(&mentci_socket, &criome_meta_socket))
            .expect("mentci daemon")
            .bind()
            .expect("bind mentci");
    wait_for_socket(&criome_socket);
    wait_for_socket(&criome_meta_socket);
    wait_for_socket(&mentci_socket);

    let bridge = CriomeApprovalBridge::new(&criome_meta_socket);
    let configured = thread::scope(|scope| {
        let server = scope.spawn(|| {
            criome
                .serve_next_meta()
                .expect("serve client approval mode")
        });
        let configuration = CriomeDaemonConfiguration::new(
            criome_socket.display().to_string(),
            store.as_path().display().to_string(),
        )
        .with_meta_socket_path(criome_meta_socket.display().to_string())
        .with_authorization_mode(AuthorizationMode::ClientApproval);
        let reply = bridge.configure(configuration).expect("configure criome");
        assert_eq!(server.join().expect("join meta configure server"), reply);
        reply
    });
    let meta_signal_criome::Output::Configured(configured) = configured else {
        panic!("expected Configured, got {configured:?}");
    };
    assert_eq!(configured.payload().value(), 1);

    let evidence = unproven_evidence(b"mentci-bridged-head");
    let object = signal_criome::AuthorizedObjectReference {
        component: ComponentKind::Spirit,
        digest: evidence.operation.object_digest().clone(),
        kind: AuthorizedObjectKind::Head,
    };
    let contract = signal_criome::ContractDigest::from_bytes(b"mentci-bridged-contract");
    let evaluation = AuthorizationEvaluation {
        contract: contract.clone(),
        object: object.clone(),
        evidence: evidence.clone(),
    };

    let pending = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve client approval park"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::EvaluateAuthorization(evaluation.clone()))
            .expect("evaluate authorization");
        assert_eq!(server.join().expect("join park server"), reply);
        reply
    });
    let CriomeReply::AuthorizationPending(pending) = pending else {
        panic!("expected AuthorizationPending, got {pending:?}");
    };
    println!("PROOF (a) criome ordinary socket parked the head for client approval");

    let parked = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next_meta().expect("serve parked list"));
        let snapshot = bridge.parked_authorizations().expect("list parked");
        let reply = server.join().expect("join parked list server");
        assert!(matches!(
            reply,
            meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
        ));
        snapshot
    });
    assert_eq!(parked.parked().len(), 1);
    assert_eq!(parked.parked()[0].request_slot, pending.request_slot);
    println!("PROOF (b) mentci bridge listed the parked criome request by slot");

    let observed = thread::scope(|scope| {
        let criome_meta_server =
            scope.spawn(|| criome.serve_next_meta().expect("serve parked list"));
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve observe"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
                subscriber: SubscriberName::new("mentci-egui"),
                interest: InterfaceInterest::PendingQuestions,
            }),
        );
        assert!(matches!(
            criome_meta_server.join().expect("join parked list"),
            meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
        ));
        mentci_server.join().expect("join observe server");
        reply
    });
    let MentciReply::InterfaceObservationOpened(opened) = observed else {
        panic!("expected InterfaceObservationOpened, got {observed:?}");
    };
    let questions = opened.state.pending_questions();
    assert_eq!(questions.len(), 1);
    assert_eq!(
        questions[0].proposal.source.criome_slot(),
        Some(&pending.request_slot)
    );
    println!(
        "PROOF (c) mentci daemon observed criome question {:?} carrying slot {:?}",
        questions[0].identifier, pending.request_slot
    );
    let verdict = ApprovalVerdict {
        question: questions[0].identifier.clone(),
        decision: ApprovalDecision::ApproveSuggestedAnswer,
        answered_by: SubscriberName::new("psyche"),
    };
    let approved = thread::scope(|scope| {
        let criome_meta_server =
            scope.spawn(|| criome.serve_next_meta().expect("serve meta approval"));
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve verdict"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::AnswerQuestion(verdict.clone()),
        );
        let meta_reply = criome_meta_server.join().expect("join meta server");
        mentci_server.join().expect("join verdict server");
        assert!(matches!(reply, MentciReply::VerdictAccepted(_)));
        meta_reply
    });
    let meta_signal_criome::Output::AuthorizationApprovalRecorded(approved) = approved else {
        panic!("expected AuthorizationApprovalRecorded, got {approved:?}");
    };
    assert_eq!(approved.request_slot, pending.request_slot);
    assert_eq!(
        approved.decision,
        meta_signal_criome::AuthorizationApprovalDecision::Approve
    );
    println!("PROOF (d) mentci daemon submitted approval to criome meta socket by slot");

    let snapshot = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve authorized observation"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::ObserveAuthorizedObjects(
                AuthorizedObjectObservation {
                    subscriber: Identity::agent("mentci-status".to_string()),
                    interest: AuthorizedObjectInterest::Component(ComponentKind::Spirit),
                },
            ))
            .expect("observe authorized objects");
        assert_eq!(server.join().expect("join observation server"), reply);
        reply
    });
    let CriomeReply::AuthorizedObjectUpdateSnapshot(snapshot) = snapshot else {
        panic!("expected AuthorizedObjectUpdateSnapshot, got {snapshot:?}");
    };
    let updates = snapshot.into_updates();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].object, object);
    assert_eq!(updates[0].decision, EvaluationDecision::Authorized);
    println!("PROOF (f) criome ordinary socket exposes the authorized head pulse");

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
}
