use std::path::Path;

use mentci::Error;
use mentci::command::DaemonCommand;
use mentci::configuration::ConfigurationFile;
use meta_signal_mentci::{
    ComponentKind, MentciDaemonConfiguration, NotificationClient, PersonaIdentity, PersonaKeyLabel,
    PersonaName, StandardSocket,
};

fn configuration(socket_path: &str, criome_path: &str) -> MentciDaemonConfiguration {
    MentciDaemonConfiguration::new(
        StandardSocket::unix(socket_path),
        StandardSocket::unix(criome_path),
        PersonaIdentity::new(
            PersonaName::new("psyche"),
            ComponentKind::Persona,
            PersonaKeyLabel::new("home-verdict"),
        ),
        vec![NotificationClient::StatusBar, NotificationClient::Popup],
    )
}

#[test]
fn startup_configuration_round_trips_as_meta_signal_frame() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("mentci-startup.rkyv");
    let socket = directory.path().join("mentci.socket");
    let criome = directory.path().join("criome.socket");
    let expected = configuration(
        socket.to_str().expect("socket utf8"),
        criome.to_str().expect("criome utf8"),
    );
    let file = ConfigurationFile::new(&path);

    file.write_configuration(&expected).expect("write startup");

    let recovered = file.configuration().expect("read startup");
    assert_eq!(recovered, expected);

    let command = DaemonCommand::from_arguments([path.to_string_lossy().to_string()]);
    let daemon_configuration = command.configuration().expect("command configuration");
    assert_eq!(
        daemon_configuration.socket_path().expect("socket path"),
        Path::new(socket.to_str().expect("socket utf8")),
    );
    assert_eq!(
        daemon_configuration
            .home_criome_socket_path()
            .expect("criome socket path"),
        Path::new(criome.to_str().expect("criome utf8")),
    );
}

#[test]
fn daemon_rejects_nota_startup_path() {
    let command = DaemonCommand::from_arguments(["startup.nota"]);
    assert!(matches!(
        command.configuration(),
        Err(Error::StartupNotaRejected(_))
    ));
}
