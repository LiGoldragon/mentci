//! Seed a live criome daemon with one parked ClientApproval request for Mentci.
//!
//! This process-boundary witness configures criome through its meta socket, then
//! submits one authorization evaluation through the working socket. It exits
//! after criome parks the request, leaving Mentci to pick it up through
//! `ObserveInterfaceState`.

use std::path::PathBuf;

use criome::transport::{CriomeClient, CriomeMetaClient};
use signal_criome::{
    AttestedMoment, AttestedMomentProposition, AuthorizationEvaluation, AuthorizationMode,
    AuthorizedObjectKind, AuthorizedObjectReference, ComponentKind, ContractDigest,
    CriomeDaemonConfiguration, CriomeReply, CriomeRequest, Evidence, ObjectDigest, OperationDigest,
    RequiredSignatureThreshold, TimeWindow, TimestampNanos,
};

struct PickupWitness {
    working: CriomeClient,
    meta: CriomeMetaClient,
    socket_path: String,
    meta_socket_path: String,
    store_path: String,
}

impl PickupWitness {
    fn from_environment() -> Self {
        let socket = std::env::var_os("CRIOME_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/criome.sock"));
        let meta = std::env::var_os("CRIOME_META_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("{}.meta", socket.display())));
        let store = std::env::var_os("CRIOME_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("{}.sema", socket.display())));
        Self {
            working: CriomeClient::new(&socket),
            meta: CriomeMetaClient::new(&meta),
            socket_path: socket.display().to_string(),
            meta_socket_path: meta.display().to_string(),
            store_path: store.display().to_string(),
        }
    }

    fn run(&self) {
        self.configure_client_approval();
        self.park_request();
    }

    fn configure_client_approval(&self) {
        let configuration = CriomeDaemonConfiguration::new(&self.socket_path, &self.store_path)
            .with_meta_socket_path(&self.meta_socket_path)
            .with_authorization_mode(AuthorizationMode::ClientApproval);
        let reply = self
            .meta
            .send(meta_signal_criome::Input::Configure(configuration))
            .expect("configure criome ClientApproval over meta socket");
        assert!(
            matches!(reply, meta_signal_criome::Output::Configured(_)),
            "expected Configured, got {reply:?}"
        );
        eprintln!("mentci-criome-pickup-witness-test: configured criome ClientApproval");
    }

    fn park_request(&self) {
        let reply = self
            .working
            .send(CriomeRequest::EvaluateAuthorization(Self::evaluation()))
            .expect("submit authorization evaluation to criome");
        let CriomeReply::AuthorizationPending(pending) = reply else {
            panic!("expected AuthorizationPending, got {reply:?}");
        };
        eprintln!(
            "mentci-criome-pickup-witness-test: parked {}",
            pending.request_slot.payload()
        );
    }

    fn evaluation() -> AuthorizationEvaluation {
        let bytes = Self::head_bytes();
        let object = AuthorizedObjectReference {
            component: ComponentKind::Spirit,
            digest: ObjectDigest::from_bytes(&bytes),
            kind: AuthorizedObjectKind::Head,
        };
        let stamp = AttestedMoment::new(
            AttestedMomentProposition::new(
                TimeWindow {
                    opens_at: TimestampNanos::new(10),
                    closes_at: TimestampNanos::new(20),
                },
                RequiredSignatureThreshold::new(1),
                Vec::new(),
            ),
            Vec::new(),
        );
        let evidence = Evidence::new(
            ComponentKind::Spirit,
            OperationDigest::from_bytes(&bytes),
            stamp,
            Vec::new(),
            Vec::new(),
        );
        AuthorizationEvaluation {
            contract: ContractDigest::from_bytes(&bytes),
            object,
            evidence,
        }
    }

    fn head_bytes() -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let mut index = 0u8;
        while (index as usize) < bytes.len() {
            bytes[index as usize] = index.wrapping_mul(13).wrapping_add(7);
            index += 1;
        }
        bytes
    }
}

fn main() {
    PickupWitness::from_environment().run();
}
