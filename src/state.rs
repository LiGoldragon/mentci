use std::collections::{BTreeMap, BTreeSet};

use mentci_lib::CriomeVerdict;
use signal_criome::ParkedAuthorization;
use signal_mentci::{
    AnswerProposal, AnswerProposalAdmitted, AnswerText, ApprovalDecision, ApprovalQuestion,
    ApprovalSource, ApprovalVerdict, ContextBody, ContextLabel, ExplanationText, InterfaceInterest,
    InterfaceMutation, InterfaceObservationOpened, InterfaceProjection, InterfaceState,
    InterfaceStateObservation, MentciReply, MentciRequest, NotificationText, PaneContent,
    PendingQuestionsView, ProjectedInterfaceState, PromptText, ProposalDigest, ProposalIdentifier,
    QuestionContext, QuestionIdentifier, QuestionPresented, Rejection, RejectionReason,
    RevisionCounter, StatusText, SubscriptionToken, TimestampNanos, UpdateAccepted,
};

#[derive(Debug, Clone)]
pub struct State {
    pending_questions: Vec<ApprovalQuestion>,
    decisions: Vec<ApprovalVerdict>,
    answer_proposals: Vec<AnswerProposalRecord>,
    subscriptions: BTreeMap<String, InterfaceInterest>,
    revision: u64,
    logical_time: u64,
    next_question: u64,
    next_proposal: u64,
    next_subscription: u64,
    status: StatusText,
    notification: Option<NotificationText>,
    panes: Vec<PaneContent>,
    criome_request_slots: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerProposalRecord {
    pub proposal: ProposalIdentifier,
    pub body: AnswerProposal,
    pub digest: ProposalDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriomeParkedApproval {
    parked: ParkedAuthorization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateApplication {
    reply: MentciReply,
    criome_verdict: Option<CriomeVerdict>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pending_questions: Vec::new(),
            decisions: Vec::new(),
            answer_proposals: Vec::new(),
            subscriptions: BTreeMap::new(),
            revision: 0,
            logical_time: 0,
            next_question: 1,
            next_proposal: 1,
            next_subscription: 1,
            status: StatusText::new("ready"),
            notification: None,
            panes: Vec::new(),
            criome_request_slots: BTreeSet::new(),
        }
    }
}

impl State {
    pub fn apply(&mut self, request: MentciRequest) -> MentciReply {
        self.apply_with_effects(request).into_reply()
    }

    pub fn apply_with_effects(&mut self, request: MentciRequest) -> StateApplication {
        match request {
            MentciRequest::PresentQuestion(proposal) => {
                StateApplication::reply(self.present_question(proposal))
            }
            MentciRequest::PushUpdate(update) => {
                let identifier = update.identifier.clone();
                self.apply_mutation(update.mutation);
                StateApplication::reply(MentciReply::UpdateAccepted(UpdateAccepted {
                    identifier,
                    revision: self.current_revision(),
                }))
            }
            MentciRequest::ObserveInterfaceState(observation) => {
                StateApplication::reply(self.observe(observation))
            }
            MentciRequest::AnswerQuestion(verdict) => self.answer(verdict),
            MentciRequest::ProposeEditedAnswer(proposal) => {
                StateApplication::reply(self.propose_answer(proposal))
            }
            MentciRequest::RetractInterfaceObservation(token) => {
                StateApplication::reply(self.retract(token))
            }
        }
    }

    pub fn full_state(&self) -> InterfaceState {
        InterfaceState::new(
            self.current_revision(),
            self.status.clone(),
            self.notification.clone(),
            self.panes.clone(),
            self.pending_questions.clone(),
        )
    }

    pub fn absorb_criome_parked_authorizations(&mut self, parked: Vec<ParkedAuthorization>) {
        for authorization in parked {
            let approval = CriomeParkedApproval::new(authorization);
            if self.criome_request_slots.insert(approval.slot_key()) {
                let question = self.mint_question_identifier();
                self.pending_questions.push(ApprovalQuestion {
                    identifier: question,
                    proposal: approval.into_question_proposal(),
                });
                self.bump_revision();
            }
        }
    }

    fn present_question(&mut self, proposal: signal_mentci::QuestionProposal) -> MentciReply {
        let question = self.mint_question_identifier();
        self.pending_questions.push(ApprovalQuestion {
            identifier: question.clone(),
            proposal,
        });
        self.bump_revision();
        MentciReply::QuestionPresented(QuestionPresented {
            question,
            revision: self.current_revision(),
            accepted_at: self.current_time(),
        })
    }

    fn apply_mutation(&mut self, mutation: InterfaceMutation) {
        match mutation {
            InterfaceMutation::SetStatus(status) => self.status = status,
            InterfaceMutation::PostNotification(notification) => {
                self.notification = Some(notification);
            }
            InterfaceMutation::SetPaneContent(content) => self.set_pane(content),
            InterfaceMutation::ClearPane(pane) => {
                self.panes.retain(|content| content.pane != pane);
            }
            InterfaceMutation::PresentApprovalQuestion(proposal) => {
                let question = self.mint_question_identifier();
                self.pending_questions.push(ApprovalQuestion {
                    identifier: question,
                    proposal,
                });
            }
            InterfaceMutation::WithdrawApprovalQuestion(identifier) => {
                self.pending_questions
                    .retain(|question| question.identifier != identifier);
            }
        }
        self.bump_revision();
    }

    fn observe(&mut self, observation: InterfaceStateObservation) -> MentciReply {
        let token = self.mint_subscription_token();
        self.subscriptions
            .insert(token.as_str().to_owned(), observation.interest);
        MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
            token,
            state: self.project(observation.interest),
        })
    }

    fn answer(&mut self, verdict: ApprovalVerdict) -> StateApplication {
        if matches!(verdict.decision, ApprovalDecision::Defer) {
            return StateApplication::reply(MentciReply::VerdictAccepted(
                signal_mentci::VerdictAccepted {
                    question: verdict.question,
                    decision: verdict.decision,
                    accepted_at: self.current_time(),
                },
            ));
        }
        let Some(index) = self
            .pending_questions
            .iter()
            .position(|question| question.identifier == verdict.question)
        else {
            return StateApplication::reply(MentciReply::Rejection(Rejection::new(
                RejectionReason::UnknownQuestion,
            )));
        };
        let answered = self.pending_questions.remove(index);
        let criome_verdict = answered
            .proposal
            .source
            .criome_slot()
            .map(|slot| CriomeVerdict::from_decision(slot.clone(), verdict.decision));
        self.decisions.push(verdict.clone());
        self.bump_revision();
        StateApplication::with_criome_verdict(
            MentciReply::VerdictAccepted(signal_mentci::VerdictAccepted {
                question: verdict.question,
                decision: verdict.decision,
                accepted_at: self.current_time(),
            }),
            criome_verdict,
        )
    }

    fn propose_answer(&mut self, proposal: AnswerProposal) -> MentciReply {
        if !self
            .pending_questions
            .iter()
            .any(|question| question.identifier == proposal.question)
        {
            return MentciReply::Rejection(Rejection::new(RejectionReason::UnknownQuestion));
        }
        let proposal_identifier = self.mint_proposal_identifier();
        let digest = ProposalDigest::new(format!(
            "answer-proposal-{}-{}",
            proposal.question.as_str(),
            proposal_identifier.as_str()
        ));
        self.answer_proposals.push(AnswerProposalRecord {
            proposal: proposal_identifier.clone(),
            body: proposal.clone(),
            digest: digest.clone(),
        });
        self.bump_revision();
        MentciReply::AnswerProposalAdmitted(AnswerProposalAdmitted {
            proposal: proposal_identifier,
            question: proposal.question,
            digest,
            revision: self.current_revision(),
        })
    }

    fn retract(&mut self, token: SubscriptionToken) -> MentciReply {
        if self.subscriptions.remove(token.as_str()).is_some() {
            MentciReply::InterfaceObservationRetracted(
                signal_mentci::InterfaceObservationRetracted::new(token),
            )
        } else {
            MentciReply::Rejection(Rejection::new(RejectionReason::UnknownSubscriber))
        }
    }

    fn set_pane(&mut self, content: PaneContent) {
        match self
            .panes
            .iter_mut()
            .find(|existing| existing.pane == content.pane)
        {
            Some(existing) => {
                existing.body = content.body;
            }
            None => self.panes.push(content),
        }
    }

    fn project(&self, interest: InterfaceInterest) -> ProjectedInterfaceState {
        let projection = match interest {
            InterfaceInterest::FullInterfaceState => {
                InterfaceProjection::FullProjection(self.full_state())
            }
            InterfaceInterest::StatusOnly => {
                InterfaceProjection::StatusProjection(self.status.clone())
            }
            InterfaceInterest::Notifications => {
                InterfaceProjection::NotificationProjection(self.notification.clone())
            }
            InterfaceInterest::PendingQuestions => InterfaceProjection::PendingQuestionsProjection(
                PendingQuestionsView::from_questions(self.pending_questions.clone()),
            ),
        };
        ProjectedInterfaceState {
            revision: self.current_revision(),
            projection,
        }
    }

    fn current_revision(&self) -> RevisionCounter {
        RevisionCounter::new(self.revision)
    }

    fn current_time(&self) -> TimestampNanos {
        TimestampNanos::new(self.logical_time)
    }

    fn bump_revision(&mut self) {
        self.revision += 1;
        self.logical_time += 1;
    }

    fn mint_question_identifier(&mut self) -> QuestionIdentifier {
        let identifier = QuestionIdentifier::new(format!("question-{}", self.next_question));
        self.next_question += 1;
        identifier
    }

    fn mint_proposal_identifier(&mut self) -> ProposalIdentifier {
        let identifier = ProposalIdentifier::new(format!("proposal-{}", self.next_proposal));
        self.next_proposal += 1;
        identifier
    }

    fn mint_subscription_token(&mut self) -> SubscriptionToken {
        let token = SubscriptionToken::new(format!("subscription-{}", self.next_subscription));
        self.next_subscription += 1;
        token
    }
}

impl StateApplication {
    pub fn reply(reply: MentciReply) -> Self {
        Self {
            reply,
            criome_verdict: None,
        }
    }

    pub fn with_criome_verdict(reply: MentciReply, criome_verdict: Option<CriomeVerdict>) -> Self {
        Self {
            reply,
            criome_verdict,
        }
    }

    pub fn criome_verdict(&self) -> Option<&CriomeVerdict> {
        self.criome_verdict.as_ref()
    }

    pub fn into_reply(self) -> MentciReply {
        self.reply
    }

    pub fn into_parts(self) -> (MentciReply, Option<CriomeVerdict>) {
        (self.reply, self.criome_verdict)
    }
}

impl CriomeParkedApproval {
    pub fn new(parked: ParkedAuthorization) -> Self {
        Self { parked }
    }

    pub fn slot_key(&self) -> String {
        self.parked.request_slot.payload().clone()
    }

    pub fn into_question_proposal(self) -> signal_mentci::QuestionProposal {
        let slot = self.parked.request_slot.payload().clone();
        let contract = format!("{:?}", self.parked.evaluation.contract);
        let object = &self.parked.evaluation.object;
        signal_mentci::QuestionProposal::new(
            ApprovalSource::CriomeEscalation(self.parked.request_slot.clone()),
            PromptText::new(format!("Authorize criome request {slot}")),
            Some(AnswerText::new("approve")),
            ExplanationText::new("criome parked an authorization request in ClientApproval mode"),
            vec![
                QuestionContext {
                    label: ContextLabel::new("criome-request-slot"),
                    body: ContextBody::new(slot),
                },
                QuestionContext {
                    label: ContextLabel::new("contract"),
                    body: ContextBody::new(contract),
                },
                QuestionContext {
                    label: ContextLabel::new("object"),
                    body: ContextBody::new(format!(
                        "{:?}:{:?}:{:?}",
                        object.component, object.kind, object.digest
                    )),
                },
            ],
        )
    }
}
