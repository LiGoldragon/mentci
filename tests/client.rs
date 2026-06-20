use mentci::client::ClientCommand;
use signal_mentci::{
    InterfaceMutation, InterfaceUpdate, MentciFrameBody, MentciRequest, NotaEncode, StatusText,
    UpdateIdentifier,
};

fn update_request() -> MentciRequest {
    MentciRequest::PushUpdate(InterfaceUpdate {
        identifier: UpdateIdentifier::new("update-1"),
        mutation: InterfaceMutation::SetStatus(StatusText::new("waiting")),
    })
}

#[test]
fn client_recognizes_criome_parked_command_atom() {
    let command = ClientCommand::from_arguments(["criome:parked"], "/tmp/unused-mentci.socket");

    assert!(command.criome_command().expect("criome command").is_some());
}

#[test]
fn client_recognizes_criome_approval_command_atom() {
    let command = ClientCommand::from_arguments(
        ["criome:approve:authorization-request-1"],
        "/tmp/unused-mentci.socket",
    );

    assert!(command.criome_command().expect("criome command").is_some());
}

#[test]
fn client_builds_request_frame_from_inline_nota() {
    let request = update_request();
    let command = ClientCommand::from_arguments([request.to_nota()], "/tmp/unused-mentci.socket");

    let frame = command.request_frame().expect("request frame");

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(request.payloads().head(), &update_request());
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}

#[test]
fn client_builds_request_frame_from_nota_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("request.nota");
    std::fs::write(&path, update_request().to_nota()).expect("write request");
    let command = ClientCommand::from_arguments(
        [path.to_string_lossy().to_string()],
        "/tmp/unused-mentci.socket",
    );

    let frame = command.request_frame().expect("request frame");

    match frame.into_body() {
        MentciFrameBody::Request { request, .. } => {
            assert_eq!(request.payloads().head(), &update_request());
        }
        other => panic!("expected request frame, got {other:?}"),
    }
}
