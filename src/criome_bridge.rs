use std::path::PathBuf;

use criome::transport::CriomeMetaClient;
use mentci_lib::CriomeVerdict;
use meta_signal_criome::{AuthorizationApproval, AuthorizationApprovalDecision};
use signal_criome::{
    AuthorizationRequestSlot, CriomeDaemonConfiguration, ParkedAuthorizationObservation,
    ParkedAuthorizationSnapshot,
};
use signal_mentci::ApprovalVerdict;

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

    pub fn configure(
        &self,
        configuration: CriomeDaemonConfiguration,
    ) -> Result<meta_signal_criome::Output> {
        CriomeMetaClient::new(&self.meta_socket)
            .send(meta_signal_criome::Input::Configure(configuration))
            .map_err(Into::into)
    }

    pub fn submit_verdict(
        &self,
        request_slot: AuthorizationRequestSlot,
        verdict: &ApprovalVerdict,
    ) -> Result<meta_signal_criome::Output> {
        let criome_verdict = CriomeVerdict::from_decision(request_slot, verdict.decision);
        self.submit_criome_verdict(&criome_verdict)
    }

    pub fn submit_criome_verdict(
        &self,
        criome_verdict: &CriomeVerdict,
    ) -> Result<meta_signal_criome::Output> {
        self.submit_decision(
            criome_verdict.request_slot().clone(),
            criome_verdict.decision(),
        )
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
}
