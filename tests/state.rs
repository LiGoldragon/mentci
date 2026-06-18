use mentci::state::State;
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
        ApprovalSource::CriomeEscalation,
        signal_mentci::PromptText::new("approve-record"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("agent-suggested-answer"),
        vec![QuestionContext {
            label: ContextLabel::new("record"),
            body: ContextBody::new("content-addressed-preimage"),
        }],
    )
}

fn question_identifier() -> QuestionIdentifier {
    QuestionIdentifier::new("question-1")
}

fn psyche() -> SubscriberName {
    SubscriberName::new("psyche")
}

#[test]
fn present_question_mints_question_and_revision() {
    let mut state = State::default();
    let reply = state.apply(MentciRequest::PresentQuestion(question_proposal()));
    assert_eq!(
        reply,
        MentciReply::QuestionPresented(QuestionPresented {
            question: question_identifier(),
            revision: RevisionCounter::new(1),
            accepted_at: TimestampNanos::new(1),
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
            subscriber: SubscriberName::new("status-bar"),
            interest: InterfaceInterest::PendingQuestions,
        },
    ));

    assert_eq!(
        reply,
        MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
            token: SubscriptionToken::new("subscription-1"),
            state: ProjectedInterfaceState {
                revision: RevisionCounter::new(1),
                projection: InterfaceProjection::PendingQuestionsProjection(
                    PendingQuestionsView::from_questions(vec![ApprovalQuestion {
                        identifier: question_identifier(),
                        proposal,
                    }]),
                ),
            },
        })
    );
}

#[test]
fn defer_keeps_question_open_for_later_answer_proposal() {
    let mut state = State::default();
    state.apply(MentciRequest::PresentQuestion(question_proposal()));

    let defer_reply = state.apply(MentciRequest::AnswerQuestion(ApprovalVerdict {
        question: question_identifier(),
        decision: ApprovalDecision::Defer,
        answered_by: psyche(),
    }));

    assert!(matches!(defer_reply, MentciReply::VerdictAccepted(_)));

    let edited_answer = AnswerProposal {
        question: question_identifier(),
        body: AnswerText::new("replacement-nota-object"),
        authored_by: psyche(),
    };
    let proposal_reply = state.apply(MentciRequest::ProposeEditedAnswer(edited_answer));

    assert_eq!(
        proposal_reply,
        MentciReply::AnswerProposalAdmitted(AnswerProposalAdmitted {
            proposal: ProposalIdentifier::new("proposal-1"),
            question: question_identifier(),
            digest: ProposalDigest::new("answer-proposal-question-1-proposal-1"),
            revision: RevisionCounter::new(2),
        })
    );
}

#[test]
fn approving_question_closes_it_against_later_edits() {
    let mut state = State::default();
    state.apply(MentciRequest::PresentQuestion(question_proposal()));

    let approve_reply = state.apply(MentciRequest::AnswerQuestion(ApprovalVerdict {
        question: question_identifier(),
        decision: ApprovalDecision::ApproveSuggestedAnswer,
        answered_by: psyche(),
    }));

    assert!(matches!(approve_reply, MentciReply::VerdictAccepted(_)));

    let proposal_reply = state.apply(MentciRequest::ProposeEditedAnswer(AnswerProposal {
        question: question_identifier(),
        body: AnswerText::new("replacement-nota-object"),
        authored_by: psyche(),
    }));

    assert_eq!(
        proposal_reply,
        MentciReply::Rejection(Rejection::new(RejectionReason::UnknownQuestion))
    );
}
