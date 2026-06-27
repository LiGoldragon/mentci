use std::path::PathBuf;

use criome::transport::CriomeMetaClient;
use mentci_lib::CriomeVerdict;
use meta_signal_criome::{AuthorizationApproval, AuthorizationApprovalDecision};
use signal_criome::{
    ActiveInterceptPolicies, AuthorizationRequestSlot, CriomeDaemonConfiguration, InterceptPolicy,
    InterceptPolicyCancellation, InterceptPolicyIdentifier, InterceptPolicyProposal,
    ParkedAuthorizationObservation, ParkedAuthorizationSnapshot, ParkedRequestAnswer,
    ParkedRequestQuery, ParkedRequestResolution, ParkedRequestSnapshot,
};

use crate::Result;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriomeApprovalBridge {
    meta_socket: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriomeApprovalSubmission {
    verdict: CriomeVerdict,
    output: meta_signal_criome::Output,
}

impl CriomeApprovalBridge {
    pub fn new(meta_socket: impl Into<PathBuf>) -> Self {
        Self {
            meta_socket: meta_socket.into(),
        }
    }

    pub fn configure(
        &self,
        configuration: CriomeDaemonConfiguration,
    ) -> Result<meta_signal_criome::Output> {
        CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::Configure(configuration))
            .map_err(Into::into)
    }

    pub fn submit_criome_verdict(
        &self,
        criome_verdict: &CriomeVerdict,
    ) -> Result<CriomeApprovalSubmission> {
        let output = self.submit_decision(
            criome_verdict.request_slot().clone(),
            criome_verdict.decision(),
        )?;
        Ok(CriomeApprovalSubmission::new(
            criome_verdict.clone(),
            output,
        ))
    }

    pub fn submit_decision(
        &self,
        request_slot: AuthorizationRequestSlot,
        decision: AuthorizationApprovalDecision,
    ) -> Result<meta_signal_criome::Output> {
        CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::SubmitAuthorizationApproval(
                AuthorizationApproval {
                    request_slot,
                    decision,
                },
            ))
            .map_err(Into::into)
    }

    pub fn parked_authorizations(&self) -> Result<ParkedAuthorizationSnapshot> {
        let reply = CriomeMetaClient::new(&self.meta_socket).send(
            meta_signal_criome::Input::ObserveParkedAuthorizations(
                ParkedAuthorizationObservation::new(),
            ),
        )?;
        let meta_signal_criome::Output::ParkedAuthorizationSnapshot(snapshot) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(snapshot)
    }

    pub fn create_intercept_policy(
        &self,
        proposal: InterceptPolicyProposal,
    ) -> Result<InterceptPolicy> {
        let reply = CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::CreateInterceptPolicy(proposal))?;
        let meta_signal_criome::Output::InterceptPolicyCreated(policy) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(policy)
    }

    pub fn replace_intercept_policy(
        &self,
        proposal: InterceptPolicyProposal,
    ) -> Result<InterceptPolicy> {
        let reply = CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::ReplaceInterceptPolicy(proposal))?;
        let meta_signal_criome::Output::InterceptPolicyReplaced(policy) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(policy)
    }

    pub fn cancel_intercept_policy(
        &self,
        cancellation: InterceptPolicyCancellation,
    ) -> Result<InterceptPolicyIdentifier> {
        let reply = CriomeMetaClient::new(&self.meta_socket).send(
            meta_signal_criome::Input::CancelInterceptPolicy(cancellation),
        )?;
        let meta_signal_criome::Output::InterceptPolicyCancelled(identifier) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(identifier)
    }

    pub fn list_intercept_policies(&self) -> Result<ActiveInterceptPolicies> {
        let reply = CriomeMetaClient::new(&self.meta_socket).send(
            meta_signal_criome::Input::ListInterceptPolicies(
                meta_signal_criome::InterceptPolicyObservation::new(),
            ),
        )?;
        let meta_signal_criome::Output::InterceptPoliciesListed(policies) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(policies)
    }

    pub fn fetch_parked_requests(
        &self,
        query: ParkedRequestQuery,
    ) -> Result<ParkedRequestSnapshot> {
        let reply = CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::FetchParkedRequests(query))?;
        let meta_signal_criome::Output::ParkedRequestsFetched(snapshot) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(snapshot)
    }

    pub fn answer_parked_request(
        &self,
        answer: ParkedRequestAnswer,
    ) -> Result<ParkedRequestResolution> {
        let reply = CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::AnswerParkedRequest(answer))?;
        let meta_signal_criome::Output::ParkedRequestAnswered(resolution) = reply else {
            return Err(crate::Error::UnexpectedCriomeMetaReply);
        };
        Ok(resolution)
    }
}

impl CriomeApprovalSubmission {
    pub fn new(verdict: CriomeVerdict, output: meta_signal_criome::Output) -> Self {
        Self { verdict, output }
    }

    pub fn is_recorded(&self) -> bool {
        let meta_signal_criome::Output::AuthorizationApprovalRecorded(recorded) = &self.output
        else {
            return false;
        };
        recorded.request_slot == *self.verdict.request_slot()
            && recorded.decision == self.verdict.decision()
    }

    pub fn output(&self) -> &meta_signal_criome::Output {
        &self.output
    }
}
