use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;

use criome::daemon::{CriomeDaemon, CriomeDaemonConfiguration};
use criome::language::AttestedMomentStatement;
use criome::master_key::MasterKey;
use criome::tables::StoreLocation;
use criome::transport::CriomeClient;
use mentci::configuration::DaemonConfiguration;
use mentci::criome_bridge::CriomeApprovalBridge;
use mentci::daemon::Daemon;
use mentci::frame_codec::FrameCodec;
use meta_signal_mentci::{
    ComponentKind as MentciComponentKind, MentciDaemonConfiguration, NotificationClient,
    PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
};
use signal_criome::{
    AttestedMoment, AttestedMomentProposition, AuthorizationEvaluation, AuthorizationMode,
    AuthorizedObjectInterest, AuthorizedObjectKind, AuthorizedObjectObservation, ComponentKind,
    Contract, CriomeReply, CriomeRequest, EvaluationDecision, Evidence, Identity,
    IdentityRegistration, KeyPurpose, OperationDigest, PublicKeyFingerprint,
    RequiredSignatureThreshold, Rule, SignatureEnvelope, SignatureScheme, TimeSignature,
    TimeWindow, TimestampNanos,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use signal_mentci::{
    AnswerText, ApprovalDecision, ApprovalSource, ApprovalVerdict, ContextBody, ContextLabel,
    ExplanationText, MentciFrame, MentciFrameBody, MentciReply, MentciRequest, PromptText,
    QuestionContext, QuestionProposal, SubscriberName,
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

fn signed_time_evidence(seed: &[u8], timekeeper: &MasterKey, signer: Identity) -> Evidence {
    let operation = operation_digest(seed);
    let proposition = AttestedMomentProposition::new(
        TimeWindow {
            opens_at: TimestampNanos::new(1),
            closes_at: TimestampNanos::new(2),
        },
        RequiredSignatureThreshold::new(1),
        vec![signer.clone()],
    );
    Evidence::new(
        ComponentKind::Spirit,
        operation,
        AttestedMoment::new(
            proposition.clone(),
            vec![TimeSignature {
                signer,
                envelope: SignatureEnvelope {
                    scheme: SignatureScheme::Bls12_381MinPk,
                    public_key: timekeeper.public_key(),
                    signature: timekeeper.sign(
                        AttestedMomentStatement::new(&proposition)
                            .to_signing_bytes()
                            .expect("moment statement")
                            .as_slice(),
                    ),
                },
            }],
        ),
        Vec::new(),
        Vec::new(),
    )
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
        StandardSocket::unix(socket.display().to_string()),
        StandardSocket::unix(criome_meta_socket.display().to_string()),
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

fn question_proposal() -> QuestionProposal {
    QuestionProposal::new(
        ApprovalSource::CriomeEscalation,
        PromptText::new("authorize-spirit-head"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("criome-escalated-policy"),
        vec![QuestionContext {
            label: ContextLabel::new("object"),
            body: ContextBody::new("authorized-head-reference"),
        }],
    )
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
fn mentci_closed_verdict_approves_criome_escalation_over_meta_socket() {
    let workspace = fixture_path("approved");
    let criome_socket = workspace.join("criome.sock");
    let criome_meta_socket = workspace.join("criome-meta.sock");
    let mentci_socket = workspace.join("mentci.sock");
    let criome = CriomeDaemon::new(
        &criome_socket,
        StoreLocation::new(workspace.join("criome.sema")),
    )
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

    let timekeeper = MasterKey::generate().expect("timekeeper key");
    let timekeeper_identity = Identity::cluster("timekeeper".to_string());
    thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve timekeeper registration"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::RegisterIdentity(IdentityRegistration::new(
                timekeeper_identity.clone(),
                timekeeper.public_key(),
                PublicKeyFingerprint::new("timekeeper-fingerprint".to_string()),
                KeyPurpose::ReleaseAuthorization,
                None,
            )))
            .expect("register timekeeper");
        assert_eq!(server.join().expect("join registration server"), reply);
    });

    let contract_reply = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve contract admission"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::AdmitContract(Contract::new(
                Rule::EscalateToPsyche,
            )))
            .expect("admit contract");
        assert_eq!(server.join().expect("join contract server"), reply);
        reply
    });
    let CriomeReply::ContractAdmitted(admitted) = contract_reply else {
        panic!("expected ContractAdmitted, got {contract_reply:?}");
    };
    let contract = admitted.into_payload();
    let evidence = signed_time_evidence(
        b"mentci-bridged-head",
        &timekeeper,
        timekeeper_identity.clone(),
    );
    let object = signal_criome::AuthorizedObjectReference {
        component: ComponentKind::Spirit,
        digest: evidence.operation.object_digest().clone(),
        kind: AuthorizedObjectKind::Head,
    };
    let evaluation = AuthorizationEvaluation {
        contract: contract.clone(),
        object: object.clone(),
        evidence: evidence.clone(),
    };

    let escalated = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next().expect("serve escalation"));
        let reply = CriomeClient::new(&criome_socket)
            .send(CriomeRequest::EvaluateAuthorization(evaluation.clone()))
            .expect("evaluate authorization");
        assert_eq!(server.join().expect("join escalation server"), reply);
        reply
    });
    let CriomeReply::AuthorizationEvaluated(escalated) = escalated else {
        panic!("expected AuthorizationEvaluated, got {escalated:?}");
    };
    assert_eq!(escalated.decision, EvaluationDecision::EscalateToPsyche);
    println!("PROOF (a) criome ordinary socket escalated the head to psyche");

    let question = thread::scope(|scope| {
        let server = scope.spawn(|| mentci.serve_next().expect("serve question"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::PresentQuestion(question_proposal()),
        );
        server.join().expect("join question server");
        reply
    });
    let MentciReply::QuestionPresented(presented) = question else {
        panic!("expected QuestionPresented, got {question:?}");
    };
    println!(
        "PROOF (b) mentci daemon presented question {:?}",
        presented.question
    );
    let verdict = ApprovalVerdict {
        question: presented.question,
        decision: ApprovalDecision::ApproveSuggestedAnswer,
        answered_by: SubscriberName::new("psyche"),
    };
    thread::scope(|scope| {
        let server = scope.spawn(|| mentci.serve_next().expect("serve verdict"));
        let reply = send_mentci(
            &mentci_socket,
            MentciRequest::AnswerQuestion(verdict.clone()),
        );
        server.join().expect("join verdict server");
        assert!(matches!(reply, MentciReply::VerdictAccepted(_)));
        println!("PROOF (c) mentci daemon accepted closed approve verdict");
    });

    let approved = thread::scope(|scope| {
        let server = scope.spawn(|| criome.serve_next_meta().expect("serve meta approval"));
        let reply = CriomeApprovalBridge::new(&criome_meta_socket)
            .submit_verdict(evaluation.clone(), &verdict)
            .expect("submit criome approval");
        assert_eq!(server.join().expect("join meta server"), reply);
        reply
    });
    let meta_signal_criome::Output::AuthorizationApprovalRecorded(approved) = approved else {
        panic!("expected AuthorizationApprovalRecorded, got {approved:?}");
    };
    assert_eq!(approved.payload().decision, EvaluationDecision::Authorized);
    println!("PROOF (d) mentci bridge submitted approval to criome meta socket");

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
    println!("PROOF (e) criome ordinary socket exposes the authorized head pulse");

    mentci.shutdown().expect("shutdown mentci");
    criome.shutdown().expect("shutdown criome");
}
