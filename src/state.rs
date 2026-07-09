use std::collections::{BTreeMap, BTreeSet};

use mentci_lib::CriomeVerdict;
use signal_criome::{
    ParkedAuthorization, ParkedRequestAnswer, ParkedRequestDecision, ParkedSpiritRequest,
};
use signal_mentci::{
    AnswerProposal, AnswerProposalAdmitted, AnswerText, ApprovalDecision, ApprovalQuestion,
    ApprovalSource, ApprovalVerdict, ContextBody, ContextLabel, CriomeAccess, ExplanationText,
    InterfaceInterest, InterfaceMutation, InterfaceObservationOpened, InterfaceProjection,
    InterfaceState, InterfaceStateObservation, MentciReply, MentciRequest, NotificationSlice,
    NotificationText, PaneContent, PendingQuestionsView, ProjectedInterfaceState, PromptText,
    ProposalDigest, ProposalIdentifier, QuestionContext, QuestionIdentifier, QuestionPresented,
    Rejection, RejectionReason, RevisionCounter, StatusText, SubscriptionToken, TimestampNanos,
    UpdateAccepted,
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
    criome_parked_request_identifiers: BTreeSet<String>,
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
pub struct CriomeParkedInterception {
    parked: ParkedSpiritRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriomeEffect {
    AuthorizationVerdict(CriomeVerdict),
    ParkedRequestAnswer(ParkedRequestAnswer),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateApplication {
    reply: MentciReply,
    criome_effect: Option<CriomeEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateApplicationContext {
    criome_write_available: bool,
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
            criome_parked_request_identifiers: BTreeSet::new(),
        }
    }
}

impl State {
    pub fn apply(&mut self, request: MentciRequest) -> MentciReply {
        self.apply_with_effects(request).into_reply()
    }

    pub fn apply_with_effects(&mut self, request: MentciRequest) -> StateApplication {
        self.apply_with_context(request, StateApplicationContext::write_enabled())
    }

    pub fn apply_with_context(
        &mut self,
        request: MentciRequest,
        context: StateApplicationContext,
    ) -> StateApplication {
        match request {
            MentciRequest::PresentQuestion(proposal) => {
                StateApplication::reply(self.present_question(proposal))
            }
            MentciRequest::PushUpdate(update) => {
                let identifier = update.update_identifier.clone();
                self.apply_mutation(update.interface_mutation);
                StateApplication::reply(MentciReply::UpdateAccepted(UpdateAccepted {
                    update_identifier: identifier,
                    revision_counter: self.current_revision(),
                }))
            }
            MentciRequest::ObserveInterfaceState(observation) => {
                StateApplication::reply(self.observe(observation, context))
            }
            MentciRequest::AnswerQuestion(verdict) => self.answer(verdict, context),
            MentciRequest::ProposeEditedAnswer(proposal) => {
                StateApplication::reply(self.propose_answer(proposal))
            }
            MentciRequest::RetractInterfaceObservation(token) => {
                StateApplication::reply(self.retract(token))
            }
            MentciRequest::CreateInterceptPolicy(_)
            | MentciRequest::ReplaceInterceptPolicy(_)
            | MentciRequest::CancelInterceptPolicy(_)
            | MentciRequest::ListInterceptPolicies(_)
            | MentciRequest::FetchParkedRequests(_)
            | MentciRequest::AnswerParkedRequest(_) => StateApplication::reply(
                MentciReply::Rejection(Rejection::new(RejectionReason::UnsupportedMutation)),
            ),
        }
    }

    pub fn full_state(&self, context: StateApplicationContext) -> InterfaceState {
        InterfaceState::new(
            self.current_revision(),
            self.status.clone(),
            self.notification.clone(),
            self.panes.clone(),
            self.pending_questions.clone(),
            context.criome_access(),
        )
    }

    pub fn absorb_criome_parked_authorizations(&mut self, parked: Vec<ParkedAuthorization>) {
        for authorization in parked {
            let approval = CriomeParkedApproval::new(authorization);
            if self.criome_request_slots.insert(approval.slot_key()) {
                let question = self.mint_question_identifier();
                self.pending_questions.push(ApprovalQuestion {
                    question_identifier: question,
                    question_proposal: approval.into_question_proposal(),
                });
                self.bump_revision();
            }
        }
    }

    pub fn absorb_criome_parked_requests(&mut self, parked: Vec<ParkedSpiritRequest>) {
        for request in parked {
            let interception = CriomeParkedInterception::new(request);
            if self
                .criome_parked_request_identifiers
                .insert(interception.identifier_key())
            {
                let question = self.mint_question_identifier();
                self.pending_questions.push(ApprovalQuestion {
                    question_identifier: question,
                    question_proposal: interception.into_question_proposal(),
                });
                self.bump_revision();
            }
        }
    }

    pub fn refresh_pane(&mut self, content: PaneContent) {
        if self.set_pane(content) {
            self.bump_revision();
        }
    }

    fn present_question(&mut self, proposal: signal_mentci::QuestionProposal) -> MentciReply {
        let question = self.mint_question_identifier();
        self.pending_questions.push(ApprovalQuestion {
            question_identifier: question.clone(),
            question_proposal: proposal,
        });
        self.bump_revision();
        MentciReply::QuestionPresented(QuestionPresented {
            question_identifier: question,
            revision_counter: self.current_revision(),
            timestamp_nanos: self.current_time(),
        })
    }

    fn apply_mutation(&mut self, mutation: InterfaceMutation) {
        match mutation {
            InterfaceMutation::SetStatus(status) => self.status = status,
            InterfaceMutation::PostNotification(notification) => {
                self.notification = Some(notification);
            }
            InterfaceMutation::SetPaneContent(content) => {
                let _ = self.set_pane(content);
            }
            InterfaceMutation::ClearPane(pane) => {
                self.panes.retain(|content| content.pane_label != pane);
            }
            InterfaceMutation::PresentApprovalQuestion(proposal) => {
                let question = self.mint_question_identifier();
                self.pending_questions.push(ApprovalQuestion {
                    question_identifier: question,
                    question_proposal: proposal,
                });
            }
            InterfaceMutation::WithdrawApprovalQuestion(identifier) => {
                self.pending_questions
                    .retain(|question| question.question_identifier != identifier);
            }
        }
        self.bump_revision();
    }

    fn observe(
        &mut self,
        observation: InterfaceStateObservation,
        context: StateApplicationContext,
    ) -> MentciReply {
        let token = self.mint_subscription_token();
        self.subscriptions
            .insert(token.as_str().to_owned(), observation.interface_interest);
        MentciReply::InterfaceObservationOpened(InterfaceObservationOpened {
            subscription_token: token,
            projected_interface_state: self.project(observation.interface_interest, context),
        })
    }

    fn answer(
        &mut self,
        verdict: ApprovalVerdict,
        context: StateApplicationContext,
    ) -> StateApplication {
        if matches!(verdict.approval_decision, ApprovalDecision::Defer) {
            return StateApplication::reply(MentciReply::VerdictAccepted(
                signal_mentci::VerdictAccepted {
                    question_identifier: verdict.question_identifier,
                    approval_decision: verdict.approval_decision,
                    timestamp_nanos: self.current_time(),
                },
            ));
        }
        let Some(index) = self
            .pending_questions
            .iter()
            .position(|question| question.question_identifier == verdict.question_identifier)
        else {
            return StateApplication::reply(MentciReply::Rejection(Rejection::new(
                RejectionReason::UnknownQuestion,
            )));
        };
        if self.pending_questions[index]
            .question_proposal
            .approval_source
            .criome_slot()
            .is_some()
            || self.pending_questions[index]
                .question_proposal
                .approval_source
                .parked_request()
                .is_some()
        {
            if !context.criome_write_available() {
                return StateApplication::reply(MentciReply::Rejection(Rejection::new(
                    RejectionReason::UnauthorizedProjection,
                )));
            }
        }
        let answered = self.pending_questions.remove(index);
        let criome_effect =
            CriomeEffect::from_answered_question(&answered, verdict.approval_decision);
        self.decisions.push(verdict.clone());
        self.bump_revision();
        StateApplication::with_criome_effect(
            MentciReply::VerdictAccepted(signal_mentci::VerdictAccepted {
                question_identifier: verdict.question_identifier,
                approval_decision: verdict.approval_decision,
                timestamp_nanos: self.current_time(),
            }),
            criome_effect,
        )
    }

    fn propose_answer(&mut self, proposal: AnswerProposal) -> MentciReply {
        if !self
            .pending_questions
            .iter()
            .any(|question| question.question_identifier == proposal.question_identifier)
        {
            return MentciReply::Rejection(Rejection::new(RejectionReason::UnknownQuestion));
        }
        let proposal_identifier = self.mint_proposal_identifier();
        let digest = ProposalDigest::new(format!(
            "answer-proposal-{}-{}",
            proposal.question_identifier.as_str(),
            proposal_identifier.as_str()
        ));
        self.answer_proposals.push(AnswerProposalRecord {
            proposal: proposal_identifier.clone(),
            body: proposal.clone(),
            digest: digest.clone(),
        });
        self.bump_revision();
        MentciReply::AnswerProposalAdmitted(AnswerProposalAdmitted {
            proposal_identifier,
            question_identifier: proposal.question_identifier,
            proposal_digest: digest,
            revision_counter: self.current_revision(),
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

    fn set_pane(&mut self, content: PaneContent) -> bool {
        match self
            .panes
            .iter_mut()
            .find(|existing| existing.pane_label == content.pane_label)
        {
            Some(existing) => {
                if *existing == content {
                    return false;
                }
                existing.context_body = content.context_body;
                true
            }
            None => {
                self.panes.push(content);
                true
            }
        }
    }

    fn project(
        &self,
        interest: InterfaceInterest,
        context: StateApplicationContext,
    ) -> ProjectedInterfaceState {
        let projection = match interest {
            InterfaceInterest::FullInterfaceState => {
                InterfaceProjection::FullProjection(self.full_state(context))
            }
            InterfaceInterest::StatusOnly => {
                InterfaceProjection::StatusProjection(self.status.clone())
            }
            InterfaceInterest::Notifications => InterfaceProjection::NotificationProjection(
                NotificationSlice::from_current(self.notification.clone()),
            ),
            InterfaceInterest::PendingQuestions => InterfaceProjection::PendingQuestionsProjection(
                PendingQuestionsView::from_questions(self.pending_questions.clone()),
            ),
        };
        ProjectedInterfaceState {
            revision_counter: self.current_revision(),
            interface_projection: projection,
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

impl CriomeEffect {
    pub fn from_answered_question(
        answered: &ApprovalQuestion,
        decision: ApprovalDecision,
    ) -> Option<Self> {
        if let Some(slot) = answered.question_proposal.approval_source.criome_slot() {
            return Some(Self::AuthorizationVerdict(CriomeVerdict::from_decision(
                slot.clone(),
                decision,
            )));
        }
        answered
            .question_proposal
            .approval_source
            .parked_request()
            .map(|identifier| {
                Self::ParkedRequestAnswer(ParkedRequestAnswer {
                    parked_request_identifier: identifier.clone(),
                    parked_request_decision: Self::parked_request_decision(decision),
                })
            })
    }

    fn parked_request_decision(decision: ApprovalDecision) -> ParkedRequestDecision {
        match decision {
            ApprovalDecision::ApproveSuggestedAnswer => ParkedRequestDecision::Approve,
            ApprovalDecision::Reject | ApprovalDecision::Defer => ParkedRequestDecision::Reject,
        }
    }
}

impl StateApplicationContext {
    pub fn write_enabled() -> Self {
        Self {
            criome_write_available: true,
        }
    }

    pub fn read_only() -> Self {
        Self {
            criome_write_available: false,
        }
    }

    pub fn criome_write_available(&self) -> bool {
        self.criome_write_available
    }

    pub fn criome_access(&self) -> CriomeAccess {
        if self.criome_write_available {
            CriomeAccess::ReadWrite
        } else {
            CriomeAccess::ReadOnly
        }
    }
}

impl StateApplication {
    pub fn reply(reply: MentciReply) -> Self {
        Self {
            reply,
            criome_effect: None,
        }
    }

    pub fn with_criome_effect(reply: MentciReply, criome_effect: Option<CriomeEffect>) -> Self {
        Self {
            reply,
            criome_effect,
        }
    }

    pub fn criome_effect(&self) -> Option<&CriomeEffect> {
        self.criome_effect.as_ref()
    }

    pub fn into_reply(self) -> MentciReply {
        self.reply
    }

    pub fn into_parts(self) -> (MentciReply, Option<CriomeEffect>) {
        (self.reply, self.criome_effect)
    }
}

impl CriomeParkedApproval {
    pub fn new(parked: ParkedAuthorization) -> Self {
        Self { parked }
    }

    pub fn slot_key(&self) -> String {
        self.parked.authorization_request_slot.payload().clone()
    }

    pub fn into_question_proposal(self) -> signal_mentci::QuestionProposal {
        let slot = self.parked.authorization_request_slot.payload().clone();
        let mut context = vec![QuestionContext {
            context_label: ContextLabel::new("criome-request-slot"),
            context_body: ContextBody::new(slot.clone()),
        }];
        let explanation = self.explanation_with_context(&mut context);
        signal_mentci::QuestionProposal::new(
            ApprovalSource::CriomeEscalation(self.parked.authorization_request_slot.clone()),
            PromptText::new(format!("Authorize criome request {slot}")),
            Some(AnswerText::new("approve")),
            explanation,
            context,
        )
    }

    fn explanation_with_context(&self, context: &mut Vec<QuestionContext>) -> ExplanationText {
        if let Some(evaluation) = self.parked.evaluation() {
            let object = &evaluation.authorized_object_reference;
            context.extend([
                QuestionContext {
                    context_label: ContextLabel::new("criome-kind"),
                    context_body: ContextBody::new("authorization-evaluation"),
                },
                QuestionContext {
                    context_label: ContextLabel::new("contract"),
                    context_body: ContextBody::new(evaluation.contract_digest.as_str()),
                },
                QuestionContext {
                    context_label: ContextLabel::new("object"),
                    context_body: ContextBody::new(format!(
                        "{:?}:{:?}:{}",
                        object.component_kind,
                        object.authorized_object_kind,
                        object.object_digest.as_str()
                    )),
                },
            ]);
            return ExplanationText::new(
                "criome parked an authorization evaluation in ClientApproval mode",
            );
        }
        if let Some(authorization) = self.parked.signal_authorization() {
            context.extend([
                QuestionContext {
                    context_label: ContextLabel::new("criome-kind"),
                    context_body: ContextBody::new("signal-call-authorization"),
                },
                QuestionContext {
                    context_label: ContextLabel::new("request-digest"),
                    context_body: ContextBody::new(authorization.request_digest().as_str()),
                },
                QuestionContext {
                    context_label: ContextLabel::new("component"),
                    context_body: ContextBody::new(format!(
                        "{:?}",
                        authorization.authorized_object_reference.component_kind
                    )),
                },
                QuestionContext {
                    context_label: ContextLabel::new("object-kind"),
                    context_body: ContextBody::new(format!(
                        "{:?}",
                        authorization
                            .authorized_object_reference
                            .authorized_object_kind
                    )),
                },
                QuestionContext {
                    context_label: ContextLabel::new("object-digest"),
                    context_body: ContextBody::new(
                        authorization
                            .authorized_object_reference
                            .object_digest
                            .as_str(),
                    ),
                },
                QuestionContext {
                    context_label: ContextLabel::new("requester"),
                    context_body: ContextBody::new(format!("{:?}", authorization.identity)),
                },
            ]);
            if let Some(spirit_context) = authorization.optional_spirit_authorization_context() {
                context.extend(CriomeParkedInterception::spirit_context_rows(
                    spirit_context,
                ));
            }
            return ExplanationText::new(
                "criome parked a signal-call authorization in ClientApproval mode",
            );
        }
        ExplanationText::new("criome parked an authorization request without a projected payload")
    }
}

impl CriomeParkedInterception {
    pub fn new(parked: ParkedSpiritRequest) -> Self {
        Self { parked }
    }

    pub fn identifier_key(&self) -> String {
        self.parked.parked_request_identifier.payload().clone()
    }

    pub fn into_question_proposal(self) -> signal_mentci::QuestionProposal {
        let identifier = self.parked.parked_request_identifier.payload().clone();
        let operation = self
            .parked
            .spirit_authorization_context
            .spirit_operation_name
            .as_str()
            .to_owned();
        let target = self
            .parked
            .spirit_authorization_context
            .spirit_process_key
            .as_str()
            .to_owned();
        let mut context = vec![
            QuestionContext {
                context_label: ContextLabel::new("criome-kind"),
                context_body: ContextBody::new("parked-spirit-request"),
            },
            QuestionContext {
                context_label: ContextLabel::new("parked-request"),
                context_body: ContextBody::new(identifier.clone()),
            },
            QuestionContext {
                context_label: ContextLabel::new("matched-policy"),
                context_body: ContextBody::new(self.parked.intercept_policy_identifier.as_str()),
            },
            QuestionContext {
                context_label: ContextLabel::new("session-slot"),
                context_body: ContextBody::new(self.parked.mentci_session_slot.as_str()),
            },
            QuestionContext {
                context_label: ContextLabel::new("parked-at"),
                context_body: ContextBody::new(self.parked.parked_at.payload().to_string()),
            },
            QuestionContext {
                context_label: ContextLabel::new("expires-at"),
                context_body: ContextBody::new(self.parked.expires_at.payload().to_string()),
            },
            QuestionContext {
                context_label: ContextLabel::new("expiry-action"),
                context_body: ContextBody::new(format!("{:?}", self.parked.expiry_action)),
            },
        ];
        context.extend(Self::spirit_context_rows(
            &self.parked.spirit_authorization_context,
        ));
        signal_mentci::QuestionProposal::new(
            ApprovalSource::CriomeInterception(self.parked.parked_request_identifier),
            PromptText::new(format!("Authorize Spirit {operation} for {target}")),
            Some(AnswerText::new("approve")),
            ExplanationText::new("criome parked a Spirit operation matched by intercept policy"),
            context,
        )
    }

    fn spirit_context_rows(
        spirit_context: &signal_criome::SpiritAuthorizationContext,
    ) -> Vec<QuestionContext> {
        vec![
            QuestionContext {
                context_label: ContextLabel::new("spirit-target"),
                context_body: ContextBody::new(spirit_context.spirit_process_key.as_str()),
            },
            QuestionContext {
                context_label: ContextLabel::new("spirit-operation"),
                context_body: ContextBody::new(spirit_context.spirit_operation_name.as_str()),
            },
            QuestionContext {
                context_label: ContextLabel::new("raw-spirit-payload"),
                context_body: ContextBody::new(
                    spirit_context.raw_spirit_operation_payload.as_str(),
                ),
            },
        ]
    }
}
