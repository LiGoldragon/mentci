use std::io::Cursor;

use mentci::datom_text::actualize;
use mentci::frame_codec::FrameCodec;
use mentci::introspection_bridge::{IntrospectionObservation, IntrospectionPane};
use signal_frame::{ExchangeIdentifier, ExchangeLane, LaneSequence, RequestPayload, SessionEpoch};
use signal_introspect::{
    ComponentTrace, IntrospectionTarget, PrototypeWitnessObservationQuery, Query, Response,
};
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

#[test]
fn codec_round_trips_a_length_prefixed_introspect_signal() {
    let codec = FrameCodec::new();
    let query = Query::PrototypeWitnessObservation(PrototypeWitnessObservationQuery {
        engine_identifier: "prototype".to_owned(),
    });
    let mut bytes = Vec::new();

    codec
        .write_introspection_signal(&mut bytes, &query)
        .expect("write signal");

    let recovered: Query = codec
        .read_introspection_signal(&mut Cursor::new(bytes))
        .expect("read signal");
    assert_eq!(recovered, query);
}

#[test]
fn introspect_pane_body_is_canonical_datom_that_actualizes_back() {
    let observation = IntrospectionObservation::IntrospectOverview(vec![Response::ComponentTrace(
        ComponentTrace {
            engine_identifier: "prototype".to_owned(),
            introspection_target: IntrospectionTarget::Signal,
            component_trace_events: Vec::new(),
        },
    )]);

    let body = IntrospectionPane::from(observation.clone())
        .into_content()
        .body;

    assert_eq!(
        body.as_str(),
        "IntrospectOverview.[ ComponentTrace.{ prototype Signal [] } ]"
    );
    let recovered: IntrospectionObservation = actualize(body.as_str()).expect("actualize pane");
    assert_eq!(recovered, observation);
}
