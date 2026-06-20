use std::path::PathBuf;

use criome::transport::CriomeMetaClient;
use meta_signal_criome::{AuthorizationApproval, AuthorizationApprovalDecision};
use signal_criome::AuthorizationEvaluation;
use signal_mentci::{ApprovalDecision, ApprovalVerdict};

use crate::Result;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriomeApprovalBridge {
    meta_socket: PathBuf,
}

impl CriomeApprovalBridge {
    pub fn new(meta_socket: impl Into<PathBuf>) -> Self {
        Self {
            meta_socket: meta_socket.into(),
        }
    }

    pub fn submit_verdict(
        &self,
        evaluation: AuthorizationEvaluation,
        verdict: &ApprovalVerdict,
    ) -> Result<meta_signal_criome::Output> {
        CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::SubmitAuthorizationApproval(
                AuthorizationApproval {
                    evaluation,
                    decision: Self::map_decision(verdict.decision),
                },
            ))
            .map_err(Into::into)
    }

    fn map_decision(decision: ApprovalDecision) -> AuthorizationApprovalDecision {
        match decision {
            ApprovalDecision::ApproveSuggestedAnswer => AuthorizationApprovalDecision::Approve,
            ApprovalDecision::Reject => AuthorizationApprovalDecision::Reject,
            ApprovalDecision::Defer => AuthorizationApprovalDecision::Defer,
        }
    }
}
