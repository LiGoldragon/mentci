//! Encode a `mentci-daemon` rkyv configuration from socket paths.
//!
//! This is the build/deploy-time bootstrap encoder for local services and
//! process-boundary tests. The daemon still consumes exactly one pre-generated
//! binary startup message; this helper writes that message from shell-provided
//! deployment values.

use std::path::PathBuf;

use mentci::configuration::ConfigurationFile;
use meta_signal_mentci::{
    ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
    NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
};

/// The encode request for the daemon socket, local criome meta socket, and
/// output binary configuration path.
struct ConfigurationEncoding {
    socket: String,
    criome: String,
    output: PathBuf,
}

impl ConfigurationEncoding {
    fn from_arguments() -> Self {
        let mut arguments = std::env::args().skip(1);
        let usage =
            "usage: mentci-write-configuration <socket-path> <criome-meta-socket> <output-rkyv>";
        let socket = arguments.next().expect(usage);
        let criome = arguments.next().expect(usage);
        let output = arguments.next().expect(usage);
        Self {
            socket,
            criome,
            output: PathBuf::from(output),
        }
    }

    fn run(self) {
        let configuration = MentciDaemonConfiguration::new(
            vec![
                ComponentSocket::new(
                    ComponentSocketKind::Mentci,
                    StandardSocket::unix(self.socket),
                ),
                ComponentSocket::new(
                    ComponentSocketKind::MetaCriome,
                    StandardSocket::unix(self.criome),
                ),
            ],
            PersonaIdentity::new(
                PersonaName::new("psyche"),
                ComponentKind::Persona,
                PersonaKeyLabel::new("home-verdict"),
            ),
            vec![NotificationClient::StatusBar, NotificationClient::Popup],
        );
        ConfigurationFile::new(&self.output)
            .write_configuration(&configuration)
            .expect("write the mentci-daemon rkyv configuration");
        eprintln!(
            "mentci-write-configuration: wrote {}",
            self.output.display()
        );
    }
}

fn main() {
    ConfigurationEncoding::from_arguments().run();
}
