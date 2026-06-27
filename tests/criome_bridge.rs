use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

use criome::daemon::{BoundCriomeDaemon, CriomeDaemon, CriomeDaemonConfiguration};
use criome::tables::{CriomeTables, InterceptPolicyDraft, StoreLocation};
use criome::transport::CriomeClient;
use mentci::configuration::DaemonConfiguration;
use mentci::criome_bridge::{CriomeApprovalBridge, CriomeApprovalSubmission};
use mentci::daemon::{BoundDaemon, Daemon};
use mentci::frame_codec::FrameCodec;
use mentci_lib::CriomeVerdict;
use meta_signal_mentci::{
    ComponentKind as MentciComponentKind, ComponentSocket, ComponentSocketKind,
    MentciDaemonConfiguration, NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName,
    StandardSocket,
};
use signal_criome::{
    AttestedMoment, AttestedMomentProposition, AuthorizationEvaluation, AuthorizationMode,
    AuthorizationRequestSlot, AuthorizationScope, AuthorizationStatus, AuthorizedObjectInterest,
    AuthorizedObjectKind, AuthorizedObjectObservation, ComponentKind, ContractName,
    ContractOperationHead, CriomeReply, CriomeRequest, EvaluationDecision, Evidence, Identity,
    InterceptPolicyCancellation, InterceptPolicyProposal, InterceptTargetSelector,
    MentciSessionSlot, ObjectDigest, OperationDigest, ParkedAuthorization, ParkedRequestAnswer,
    ParkedRequestDecision, ParkedRequestQuery, ParkedSpiritRequest, PolicyDurationNanos,
    PolicyOverlapMode, PolicyPriority, RawSpiritOperationPayload, ReplayNonce,
    RequiredSignatureThreshold, SignalCallAuthorization, SignatureScheme,
    SpiritAuthorizationContext, SpiritOperationName, SpiritOperationNames, SpiritProcessKey,
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

fn spirit_signal_authorization(seed: &[u8], nonce: &str) -> SignalCallAuthorization {
    SignalCallAuthorization::new(
        ObjectDigest::from_bytes(seed),
        ContractName::new("spirit-local-head"),
        ContractOperationHead::new("AuthorizeHead"),
        AuthorizationScope::new("spirit-head-fanout"),
        Identity::host("spirit".to_string()),
        ReplayNonce::new(nonce),
        None,
    )
}

fn intercept_policy_proposal(
    session: &str,
    target: &str,
    operation: &str,
    priority: u64,
    overlap_mode: PolicyOverlapMode,
) -> InterceptPolicyProposal {
    InterceptPolicyProposal {
        session_slot: MentciSessionSlot::new(session),
        target: InterceptTargetSelector::new(SpiritProcessKey::new(target)),
        spirit_operation_names: SpiritOperationNames::from_names(vec![SpiritOperationName::new(
            operation,
        )]),
        duration: PolicyDurationNanos::new(9_000_000_000_000_000_000),
        expiry_action: signal_criome::ExpiryAction::LeaveParked,
        priority: PolicyPriority::new(priority),
        overlap_mode,
    }
}

fn spirit_context(target: &str, operation: &str, payload: &str) -> SpiritAuthorizationContext {
    SpiritAuthorizationContext {
        operation_name: SpiritOperationName::new(operation),
        raw_payload: RawSpiritOperationPayload::new(payload),
        target_key: SpiritProcessKey::new(target),
    }
}

fn all_parked_requests() -> ParkedRequestQuery {
    ParkedRequestQuery {
        session_slot: None,
        target: None,
    }
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

fn send_mentci_with_criome_meta(
    criome: &BoundCriomeDaemon,
    mentci: &BoundDaemon,
    mentci_socket: &Path,
    request: MentciRequest,
) -> (MentciReply, meta_signal_criome::Output) {
    let (reply, mut meta_replies) =
        send_mentci_with_criome_meta_replies(criome, mentci, mentci_socket, request, 1);
    (reply, meta_replies.remove(0))
}

fn send_mentci_with_criome_meta_replies(
    criome: &BoundCriomeDaemon,
    mentci: &BoundDaemon,
    mentci_socket: &Path,
    request: MentciRequest,
    meta_reply_count: usize,
) -> (MentciReply, Vec<meta_signal_criome::Output>) {
    thread::scope(|scope| {
        let criome_meta_server = scope.spawn(|| {
            let mut replies = Vec::new();
            for _ in 0..meta_reply_count {
                replies.push(criome.serve_next_meta().expect("serve criome meta"));
            }
            replies
        });
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve mentci"));
        let reply = send_mentci(mentci_socket, request);
        let meta_replies = criome_meta_server.join().expect("join criome meta");
        mentci_server.join().expect("join mentci");
        (reply, meta_replies)
    })
}

fn prepopulate_parked_spirit_requests(
    store: StoreLocation,
    requests: &[(&str, &str, &str)],
) -> Vec<ParkedSpiritRequest> {
    let tables = CriomeTables::open(&store).expect("open criome tables");
    tables
        .put_intercept_policy(InterceptPolicyDraft::create(
            intercept_policy_proposal(
                "mentci-policy-session",
                "spirit-process-main",
                "Record",
                1,
                PolicyOverlapMode::RejectSamePriorityOverlap,
            ),
            TimestampNanos::new(10),
        ))
        .expect("store intercept policy");
    requests
        .iter()
        .map(|(target, operation, payload)| {
            tables
                .put_parked_spirit_request(
                    spirit_context(target, operation, payload),
                    TimestampNanos::new(11),
                )
                .expect("intercept spirit authorization")
                .expect("policy parks request")
                .request()
                .clone()
        })
        .collect()
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
fn mentci_daemon_manages_intercept_policies_over_criome_meta_socket() {
    let workspace = fixture_path("intercept-policy-control");
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

    let created = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::CreateInterceptPolicy(intercept_policy_proposal(
            "mentci-a",
            "spirit-process-main",
            "Record",
            1,
            PolicyOverlapMode::RejectSamePriorityOverlap,
        )),
    )
    .0;
    let MentciReply::InterceptPolicyCreated(created) = created else {
        panic!("expected InterceptPolicyCreated, got {created:?}");
    };
    assert_eq!(created.session_slot.as_str(), "mentci-a");

    let listed = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ListInterceptPolicies(signal_mentci::InterceptPolicyObservation::new()),
    )
    .0;
    let MentciReply::InterceptPoliciesListed(listed) = listed else {
        panic!("expected InterceptPoliciesListed, got {listed:?}");
    };
    assert_eq!(listed.policies(), std::slice::from_ref(&created));

    let replaced = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ReplaceInterceptPolicy(intercept_policy_proposal(
            "mentci-b",
            "spirit-process-main",
            "Record",
            1,
            PolicyOverlapMode::ReplaceSamePriorityOverlap,
        )),
    )
    .0;
    let MentciReply::InterceptPolicyReplaced(replaced) = replaced else {
        panic!("expected InterceptPolicyReplaced, got {replaced:?}");
    };
    assert_eq!(replaced.session_slot.as_str(), "mentci-b");

    let cancelled = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::CancelInterceptPolicy(InterceptPolicyCancellation::new(
            replaced.identifier.clone(),
        )),
    )
    .0;
    assert!(matches!(
        cancelled,
        MentciReply::InterceptPolicyCancelled(identifier) if identifier == replaced.identifier
    ));

    let listed_after_cancel = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ListInterceptPolicies(signal_mentci::InterceptPolicyObservation::new()),
    )
    .0;
    let MentciReply::InterceptPoliciesListed(listed_after_cancel) = listed_after_cancel else {
        panic!("expected InterceptPoliciesListed after cancel, got {listed_after_cancel:?}");
    };
    assert!(listed_after_cancel.policies().is_empty());

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
}

#[test]
fn mentci_fetches_projects_and_answers_policy_parked_spirit_requests() {
    let workspace = fixture_path("policy-parked-spirit");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let mentci_socket = workspace.join("mentci.sock");
    let store = StoreLocation::new(workspace.join("criome.sema"));
    let parked = prepopulate_parked_spirit_requests(
        store.clone(),
        &[
            (
                "spirit-process-main",
                "Record",
                "(Record first-policy-parked)",
            ),
            (
                "spirit-process-main",
                "Record",
                "(Record second-policy-parked)",
            ),
        ],
    );
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

    let fetched = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::FetchParkedRequests(all_parked_requests()),
    )
    .0;
    let MentciReply::ParkedRequestsFetched(fetched) = fetched else {
        panic!("expected ParkedRequestsFetched, got {fetched:?}");
    };
    assert_eq!(fetched.requests(), parked.as_slice());
    assert_eq!(
        fetched.requests()[0].context.raw_payload.as_str(),
        "(Record first-policy-parked)"
    );

    let direct_answered = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::AnswerParkedRequest(ParkedRequestAnswer {
            identifier: parked[0].identifier.clone(),
            decision: ParkedRequestDecision::Reject,
        }),
    )
    .0;
    let MentciReply::ParkedRequestAnswered(direct_answered) = direct_answered else {
        panic!("expected ParkedRequestAnswered, got {direct_answered:?}");
    };
    assert_eq!(direct_answered.identifier, parked[0].identifier);
    assert_eq!(
        direct_answered.audit_source,
        signal_criome::ApprovalAuditSource::Manual
    );

    let observed = send_mentci_with_criome_meta_replies(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest: InterfaceInterest::PendingQuestions,
        }),
        2,
    )
    .0;
    let MentciReply::InterfaceObservationOpened(opened) = observed else {
        panic!("expected InterfaceObservationOpened, got {observed:?}");
    };
    let questions = opened.state.pending_questions();
    assert_eq!(questions.len(), 1);
    assert_eq!(
        questions[0].proposal.source.parked_request(),
        Some(&parked[1].identifier)
    );
    assert!(
        questions[0]
            .proposal
            .context()
            .iter()
            .any(|context| context.body.as_str() == "(Record second-policy-parked)")
    );
    assert!(
        questions[0]
            .proposal
            .context()
            .iter()
            .any(|context| context.body.as_str() == "spirit-process-main")
    );

    let verdict = ApprovalVerdict {
        question: questions[0].identifier.clone(),
        decision: ApprovalDecision::ApproveSuggestedAnswer,
        answered_by: SubscriberName::new("psyche"),
    };
    let (accepted, meta_reply) = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::AnswerQuestion(verdict),
    );
    assert!(matches!(accepted, MentciReply::VerdictAccepted(_)));
    let meta_signal_criome::Output::ParkedRequestAnswered(answered) = meta_reply else {
        panic!("expected criome ParkedRequestAnswered, got {meta_reply:?}");
    };
    assert_eq!(answered.identifier, parked[1].identifier);
    assert_eq!(
        answered.outcome,
        signal_criome::ParkedRequestOutcome::Approved
    );
    assert_eq!(
        answered.audit_source,
        signal_criome::ApprovalAuditSource::Manual
    );

    let fetched_after_answer = send_mentci_with_criome_meta(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::FetchParkedRequests(all_parked_requests()),
    )
    .0;
    let MentciReply::ParkedRequestsFetched(fetched_after_answer) = fetched_after_answer else {
        panic!("expected ParkedRequestsFetched after answer, got {fetched_after_answer:?}");
    };
    assert!(fetched_after_answer.requests().is_empty());

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
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

    let (observed, observe_meta_replies) = send_mentci_with_criome_meta_replies(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest: InterfaceInterest::PendingQuestions,
        }),
        2,
    );
    assert!(matches!(
        observe_meta_replies[0],
        meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
    ));
    assert!(matches!(
        observe_meta_replies[1],
        meta_signal_criome::Output::ParkedRequestsFetched(_)
    ));

    let parked = ParkedAuthorization::from_evaluation(pending.request_slot, evaluation);
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
fn mentci_observes_spirit_signal_authorization_bypassing_guardian() {
    let workspace = fixture_path("spirit-signal-authorization");
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

    let authorization =
        spirit_signal_authorization(b"spirit-record-head-through-criome", "spirit-nonce-1");
    let request_digest = authorization.request_digest.clone();
    let pending = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve spirit signal park"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::AuthorizeSignalCall(authorization.clone()))
            .expect("submit spirit signal authorization");
        assert_eq!(server.join().expect("join spirit signal park"), reply);
        reply
    });
    let CriomeReply::AuthorizationPending(pending) = pending else {
        panic!("expected AuthorizationPending, got {pending:?}");
    };
    assert_eq!(pending.request_digest, request_digest);
    println!(
        "PROOF (a) a spirit-shaped AuthorizeSignalCall bypasses the guardian and parks in criome"
    );

    let (observed, observe_meta_replies) = send_mentci_with_criome_meta_replies(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest: InterfaceInterest::PendingQuestions,
        }),
        2,
    );
    assert!(matches!(
        observe_meta_replies[0],
        meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
    ));
    assert!(matches!(
        observe_meta_replies[1],
        meta_signal_criome::Output::ParkedRequestsFetched(_)
    ));
    let MentciReply::InterfaceObservationOpened(opened) = observed else {
        panic!("expected InterfaceObservationOpened, got {observed:?}");
    };
    let questions = opened.state.pending_questions();
    assert_eq!(questions.len(), 1);
    let question = &questions[0];
    assert_eq!(
        question.proposal.source.criome_slot(),
        Some(&pending.request_slot)
    );
    assert!(
        question
            .proposal
            .context()
            .iter()
            .any(|context| context.body.as_str() == "signal-call-authorization")
    );
    assert!(
        question
            .proposal
            .context()
            .iter()
            .any(|context| context.body.as_str() == "AuthorizeHead")
    );
    println!(
        "PROOF (b) mentci observes the parked spirit request as question {:?} carrying slot {:?}",
        question.identifier, pending.request_slot
    );

    let verdict = ApprovalVerdict {
        question: question.identifier.clone(),
        decision: ApprovalDecision::ApproveSuggestedAnswer,
        answered_by: SubscriberName::new("psyche"),
    };
    let approved = thread::scope(|scope| {
        let criome_meta_server =
            scope.spawn(|| criome.serve_next_meta().expect("serve meta approval"));
        let mentci_server = scope.spawn(|| mentci.serve_next().expect("serve verdict"));
        let reply = send_mentci(&mentci_socket, MentciRequest::AnswerQuestion(verdict));
        let meta_reply = criome_meta_server.join().expect("join meta approval");
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
    println!("PROOF (c) mentci answers through the daemon, and criome records approval by slot");

    let snapshot = thread::scope(|scope| {
        let server = scope.spawn(|| {
            criome
                .serve_next()
                .expect("serve authorization observation")
        });
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::ObserveAuthorization(
                signal_criome::AuthorizationObservation::new(pending.request_slot.clone()),
            ))
            .expect("observe authorization");
        assert_eq!(
            server.join().expect("join authorization observation"),
            reply
        );
        reply
    });
    let CriomeReply::AuthorizationObservationSnapshot(snapshot) = snapshot else {
        panic!("expected AuthorizationObservationSnapshot, got {snapshot:?}");
    };
    let states = snapshot.into_states();
    assert_eq!(states.len(), 1);
    let state = &states[0];
    assert_eq!(state.status, AuthorizationStatus::Granted);
    assert_eq!(state.signal_authorization(), Some(&authorization));
    let grant = state.grant().expect("criome approval stores signed grant");
    assert_eq!(grant.request_slot, pending.request_slot);
    assert_eq!(grant.authorized_object_digest, request_digest);
    assert_eq!(grant.issued_by, Identity::host("criome".to_string()));
    assert_eq!(grant.signatures().len(), 1);
    assert_eq!(
        grant.signatures()[0].envelope.scheme,
        SignatureScheme::Bls12_381MinPk
    );
    assert!(
        !grant.signatures()[0].envelope.signature.as_str().is_empty(),
        "criome approval signs the grant"
    );
    println!("PROOF (d) criome signs a real AuthorizationGrant after mentci approval");

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

    let (observed, observe_meta_replies) = send_mentci_with_criome_meta_replies(
        &criome,
        &mentci,
        &mentci_socket,
        MentciRequest::ObserveInterfaceState(signal_mentci::InterfaceStateObservation {
            subscriber: SubscriberName::new("mentci-egui"),
            interest: InterfaceInterest::PendingQuestions,
        }),
        2,
    );
    assert!(matches!(
        observe_meta_replies[0],
        meta_signal_criome::Output::ParkedAuthorizationSnapshot(_)
    ));
    assert!(matches!(
        observe_meta_replies[1],
        meta_signal_criome::Output::ParkedRequestsFetched(_)
    ));
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
