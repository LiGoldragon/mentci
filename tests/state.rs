use mentci::state::{State, StateApplicationContext};
use signal_criome::{
    AuthorizationRequestSlot, AuthorizedObjectKind, AuthorizedObjectReference, ComponentKind,
    Identity, ObjectDigest, ParkedAuthorization, ReplayNonce, SignalCallAuthorization,
};
use signal_mentci::{
    AnswerProposal, AnswerProposalAdmitted, AnswerText, ApprovalDecision, ApprovalQuestion,
    ApprovalSource, ApprovalVerdict, ContextBody, ContextLabel, ExplanationText, InterfaceInterest,
    InterfaceObservationOpened, InterfaceProjection, InterfaceStateObservation, MentciReply,
    MentciRequest, PendingQuestionsView, ProjectedInterfaceState, ProposalDigest,
    ProposalIdentifier, QuestionContext, QuestionIdentifier, QuestionPresented, QuestionProposal,
    Rejection, RejectionReason, RevisionCounter, SubscriberName, SubscriptionToken, TimestampNanos,
};

fn question_proposal() -> QuestionProposal {
    QuestionProposal::new(
        ApprovalSource::AgentQuestion,
        signal_mentci::PromptText::new("approve-record"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("agent-suggested-answer"),
        vec![QuestionContext {
            context_label: ContextLabel::new("record"),
            context_body: ContextBody::new("content-addressed-preimage"),
        }],
    )
}

fn criome_question_proposal() -> QuestionProposal {
    QuestionProposal::new(
        ApprovalSource::CriomeEscalation(AuthorizationRequestSlot::new("authorization-slot-1")),
        signal_mentci::PromptText::new("approve-criome-request"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("criome-parked-authorization"),
        vec![QuestionContext {
            context_label: ContextLabel::new("slot"),
            context_body: ContextBody::new("authorization-slot-1"),
        }],
    )
}

fn question_identifier() -> QuestionIdentifier {
    QuestionIdentifier::new("question-1")
}

fn psyche() -> SubscriberName {
    SubscriberName::new("psyche")
}

fn signal_call_authorization() -> SignalCallAuthorization {
    SignalCallAuthorization::new(
        AuthorizedObjectReference {
            component_kind: ComponentKind::Spirit,
            object_digest: ObjectDigest::from_bytes(b"spirit-record-request"),
            authorized_object_kind: AuthorizedObjectKind::Operation,
        },
        Identity::developer("operator".to_string()),
        ReplayNonce::new("signal-call-nonce-1"),
        None,
    )
}

#[test]
fn present_question_mints_question_and_revision() {
    let mut state = State::default();
    let reply = state.apply(MentciRequest::PresentQuestion(question_proposal()));
    assert_eq!(
        reply,
        MentciReply::QuestionPresented(QuestionPresented {
            question_identifier: question_identifier(),
            revision_counter: RevisionCounter::new(1),
            timestamp_nanos: TimestampNanos::new(1),
        })
    );
}

#[test]
fn observe_returns_subscription_token_and_current_projection() {
    let mut state = State::default();
    let proposal = question_proposal();
    state.apply(MentciRequest::PresentQuestion(proposal.clone()));

    let reply = state.apply(MentciRequest::ObserveInterfaceState(
        InterfaceStateObservation {
            subscriber_name: SubscriberName::new("status-bar"),
            interface_interest: InterfaceInterest::PendingQuestions,
        },
    ));

    assert_eq!(
        reply,
        MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
            subscription_token: SubscriptionToken::new("subscription-1"),
            projected_interface_state: ProjectedInterfaceState {
                revision_counter: RevisionCounter::new(1),
                interface_projection: InterfaceProjection::PendingQuestionsProjection(
                    PendingQuestionsView::from_questions(vec![ApprovalQuestion {
                        question_identifier: question_identifier(),
                        question_proposal: proposal,
                    }]),
                ),
            },
        })
    );
}

#[test]
fn full_projection_mirrors_criome_access_mode_from_context() {
    let mut state = State::default();

    let write_reply = state
        .apply_with_context(
            MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
                subscriber_name: SubscriberName::new("status-bar"),
                interface_interest: InterfaceInterest::FullInterfaceState,
            }),
            StateApplicationContext::write_enabled(),
        )
        .into_reply();
    let MentciReply::InterfaceObservationOpened(write_opened) = write_reply else {
        panic!("expected write opened observation");
    };
    assert_eq!(
        write_opened.projected_interface_state.criome_access(),
        Some(signal_mentci::CriomeAccess::ReadWrite)
    );

    let read_reply = state
        .apply_with_context(
            MentciRequest::ObserveInterfaceState(InterfaceStateObservation {
                subscriber_name: SubscriberName::new("status-bar"),
                interface_interest: InterfaceInterest::FullInterfaceState,
            }),
            StateApplicationContext::read_only(),
        )
        .into_reply();
    let MentciReply::InterfaceObservationOpened(read_opened) = read_reply else {
        panic!("expected read-only opened observation");
    };
    assert_eq!(
        read_opened.projected_interface_state.criome_access(),
        Some(signal_mentci::CriomeAccess::ReadOnly)
    );
}

#[test]
fn absorbing_signal_call_parked_authorization_projects_question_context() {
    let mut state = State::default();
    state.absorb_criome_parked_authorizations(vec![
        ParkedAuthorization::from_signal_authorization(
            AuthorizationRequestSlot::new("authorization-slot-1"),
            signal_call_authorization(),
        ),
    ]);

    let reply = state.apply(MentciRequest::ObserveInterfaceState(
        InterfaceStateObservation {
            subscriber_name: SubscriberName::new("status-bar"),
            interface_interest: InterfaceInterest::PendingQuestions,
        },
    ));

    let MentciReply::InterfaceObservationOpened(opened) = reply else {
        panic!("expected opened observation");
    };
    let InterfaceProjection::PendingQuestionsProjection(pending) =
        opened.projected_interface_state.interface_projection
    else {
        panic!("expected pending projection");
    };
    let questions = pending.questions();
    assert_eq!(questions.len(), 1);
    let question = &questions[0];
    assert_eq!(
        question.question_proposal.approval_source.criome_slot(),
        Some(&AuthorizationRequestSlot::new("authorization-slot-1"))
    );
    assert_eq!(
        question.question_proposal.explanation_text,
        ExplanationText::new("criome parked a signal-call authorization in ClientApproval mode")
    );
    assert!(question.question_proposal.context().iter().any(|context| {
        context.context_label == ContextLabel::new("criome-kind")
            && context.context_body == ContextBody::new("signal-call-authorization")
    }));
    assert!(question.question_proposal.context().iter().any(|context| {
        context.context_label == ContextLabel::new("component")
            && context.context_body == ContextBody::new("Spirit")
    }));
}

#[test]
fn defer_keeps_question_open_for_later_answer_proposal() {
    let mut state = State::default();
    state.apply(MentciRequest::PresentQuestion(question_proposal()));

    let defer_reply = state.apply(MentciRequest::AnswerQuestion(ApprovalVerdict {
        question_identifier: question_identifier(),
        approval_decision: ApprovalDecision::Defer,
        subscriber_name: psyche(),
    }));

    assert!(matches!(defer_reply, MentciReply::VerdictAccepted(_)));

    let edited_answer = AnswerProposal {
        question_identifier: question_identifier(),
        answer_text: AnswerText::new("replacement-nota-object"),
        subscriber_name: psyche(),
    };
    let proposal_reply = state.apply(MentciRequest::ProposeEditedAnswer(edited_answer));

    assert_eq!(
        proposal_reply,
        MentciReply::AnswerProposalAdmitted(AnswerProposalAdmitted {
            proposal_identifier: ProposalIdentifier::new("proposal-1"),
            question_identifier: question_identifier(),
            proposal_digest: ProposalDigest::new("answer-proposal-question-1-proposal-1"),
            revision_counter: RevisionCounter::new(2),
        })
    );
}

#[test]
fn approving_question_closes_it_against_later_edits() {
    let mut state = State::default();
    state.apply(MentciRequest::PresentQuestion(question_proposal()));

    let approve_reply = state.apply(MentciRequest::AnswerQuestion(ApprovalVerdict {
        question_identifier: question_identifier(),
        approval_decision: ApprovalDecision::ApproveSuggestedAnswer,
        subscriber_name: psyche(),
    }));

    assert!(matches!(approve_reply, MentciReply::VerdictAccepted(_)));

    let proposal_reply = state.apply(MentciRequest::ProposeEditedAnswer(AnswerProposal {
        question_identifier: question_identifier(),
        answer_text: AnswerText::new("replacement-nota-object"),
        subscriber_name: psyche(),
    }));

    assert_eq!(
        proposal_reply,
        MentciReply::Rejection(Rejection::new(RejectionReason::UnknownQuestion))
    );
}

#[test]
fn read_only_context_rejects_criome_write_without_closing_question() {
    let mut state = State::default();
    state.apply(MentciRequest::PresentQuestion(criome_question_proposal()));

    let application = state.apply_with_context(
        MentciRequest::AnswerQuestion(ApprovalVerdict {
            question_identifier: question_identifier(),
            approval_decision: ApprovalDecision::ApproveSuggestedAnswer,
            subscriber_name: psyche(),
        }),
        StateApplicationContext::read_only(),
    );

    assert_eq!(
        application.into_reply(),
        MentciReply::Rejection(Rejection::new(RejectionReason::UnauthorizedProjection))
    );

    let reply = state.apply(MentciRequest::ObserveInterfaceState(
        InterfaceStateObservation {
            subscriber_name: SubscriberName::new("status-bar"),
            interface_interest: InterfaceInterest::PendingQuestions,
        },
    ));

    let MentciReply::InterfaceObservationOpened(opened) = reply else {
        panic!("expected opened observation");
    };
    let InterfaceProjection::PendingQuestionsProjection(pending) =
        opened.projected_interface_state.interface_projection
    else {
        panic!("expected pending projection");
    };
    assert_eq!(pending.questions().len(), 1);
}
