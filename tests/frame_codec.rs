use std::io::Cursor;

use mentci::frame_codec::FrameCodec;
use signal_frame::{ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch};
use signal_mentci::{
    InterfaceMutation, InterfaceUpdate, MentciFrame, MentciFrameBody, MentciRequest, StatusText,
    UpdateIdentifier,
};

fn exchange() -> ExchangeIdentifier {
    ExchangeIdentifier::new(
        SessionEpoch::new(1),
        ExchangeLane::Connector,
        LaneSequence::first(),
    )
}

#[test]
fn codec_round_trips_length_prefixed_mentci_frame() {
    let codec = FrameCodec::new();
    let frame = MentciFrame::new(MentciFrameBody::Request {
        exchange: exchange(),
        request: MentciRequest::PushUpdate(InterfaceUpdate {
            identifier: UpdateIdentifier::new("update-1"),
            mutation: InterfaceMutation::SetStatus(StatusText::new("waiting")),
        })
        .into_request(),
    });
    let mut bytes = Vec::new();

    codec
        .write_mentci_frame(&mut bytes, &frame)
        .expect("write frame");

    let recovered = codec
        .read_mentci_frame(&mut Cursor::new(bytes))
        .expect("read frame");
    assert_eq!(recovered, frame);
}
