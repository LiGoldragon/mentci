use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use mentci::harness_liveness::{
    CloseReport, CloseRequest, CloseSignal, DriverError, IdleTimeout, LaunchRequest,
    LivenessPolicy, StopCondition, StopConditions, StopReason, TerminalCellDriver, TerminalCommand,
    TerminalFeed, TerminalLaunch, TerminalObservation, TerminalSessionLauncher,
    TerminalSessionSurface, TerminalSize, TranscriptCapture,
};
use mentci::harness_sessions::{
    HarnessKind, HarnessLaunchMetadata, InMemoryHarnessSessionDirectory, NamedHarnessLaunch,
    NamedHarnessSessions, NamedSessionAddress, OpenOrReuseHarnessSession, OpenOrReuseOutcome,
    SessionAddress, SessionAddressRecord, SessionLookupError, SessionRecordState,
    SessionRoutingError,
};
use mentci::preflight::{
    AdapterIdentity, HarnessSessionModelProfile, LaneName, MentciPreflightLaunch,
    PersistentSession, SessionHandle, TerminalCellDriverIdentity,
};

#[derive(Clone)]
struct FakeLauncher {
    launched: Rc<RefCell<Vec<LaunchRequest>>>,
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    feed_responses: Rc<RefCell<VecDeque<Vec<u8>>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
    close_report: CloseReport,
}

impl FakeLauncher {
    fn new(events: impl Into<VecDeque<TerminalObservation>>) -> Self {
        Self {
            launched: Rc::new(RefCell::new(Vec::new())),
            events: Rc::new(RefCell::new(events.into())),
            feed_responses: Rc::new(RefCell::new(VecDeque::new())),
            sent: Rc::new(RefCell::new(Vec::new())),
            close_report: CloseReport::new(CloseSignal::DriverDefault),
        }
    }

    fn with_feed_responses(responses: impl Into<VecDeque<Vec<u8>>>) -> Self {
        Self {
            launched: Rc::new(RefCell::new(Vec::new())),
            events: Rc::new(RefCell::new(VecDeque::new())),
            feed_responses: Rc::new(RefCell::new(responses.into())),
            sent: Rc::new(RefCell::new(Vec::new())),
            close_report: CloseReport::new(CloseSignal::DriverDefault),
        }
    }

    fn launched(&self) -> Vec<LaunchRequest> {
        self.launched.borrow().clone()
    }

    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.borrow().clone()
    }
}

impl TerminalSessionLauncher for FakeLauncher {
    type Session = FakeTerminal;

    fn launch(&self, request: LaunchRequest) -> Result<Self::Session, DriverError> {
        self.launched.borrow_mut().push(request);
        Ok(FakeTerminal {
            events: self.events.clone(),
            feed_responses: self.feed_responses.clone(),
            sent: self.sent.clone(),
            close_report: self.close_report.clone(),
            transcript: Vec::new(),
        })
    }
}

struct FakeTerminal {
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    feed_responses: Rc<RefCell<VecDeque<Vec<u8>>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
    close_report: CloseReport,
    transcript: Vec<u8>,
}

impl TerminalSessionSurface for FakeTerminal {
    fn send(&mut self, feed: TerminalFeed) -> Result<(), DriverError> {
        self.sent.borrow_mut().push(feed.bytes().to_vec());
        if let Some(response) = self.feed_responses.borrow_mut().pop_front() {
            self.events
                .borrow_mut()
                .push_back(TerminalObservation::Transcript(response));
        }
        Ok(())
    }

    fn read_event(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<TerminalObservation>, DriverError> {
        let event = self.events.borrow_mut().pop_front();
        if let Some(TerminalObservation::Transcript(bytes)) = &event {
            self.transcript.extend_from_slice(bytes);
        }
        Ok(event)
    }

    fn transcript(&mut self) -> Result<TranscriptCapture, DriverError> {
        Ok(TranscriptCapture::from_bytes(self.transcript.clone()))
    }

    fn close(&mut self, request: CloseRequest) -> Result<CloseReport, DriverError> {
        if let CloseRequest::TerminalInput(feed) = request {
            self.send(feed)?;
        }
        Ok(self.close_report.clone())
    }
}

fn terminal_launch() -> LaunchRequest {
    terminal_launch_with_liveness(LivenessPolicy::new(StopConditions::new([
        StopCondition::CompletionSignal,
    ])))
}

fn terminal_launch_with_liveness(liveness: LivenessPolicy) -> LaunchRequest {
    LaunchRequest::new(TerminalLaunch::new(
        TerminalCommand::new("fake-harness", Vec::<String>::new()),
        TerminalSize::new(24, 80),
    ))
    .with_liveness(liveness)
}

fn valid_launch_nota() -> &'static str {
    r#"(MentciPreflightLaunch
  (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred)
  (mentci-primary-vxu6 [(Bead primary-vxu6) (WorkSurface sandboxed-jj-task) (HarnessLabel mentci-harness)] primary-vxu6-session orchestrate/lanes/primary-vxu6)
  Persistent
  (SandboxedJjTask PrimaryForbidden PrivateScopeClosed)
  [CompletionSignal]
  [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary)])"#
}

fn launch_packet() -> MentciPreflightLaunch {
    MentciPreflightLaunch::validated_from_nota(valid_launch_nota()).expect("valid launch packet")
}

fn launch_metadata() -> HarnessLaunchMetadata {
    HarnessLaunchMetadata::new(
        HarnessKind::codex(),
        AdapterIdentity::new("codex-terminal-adapter"),
        TerminalCellDriverIdentity::new("terminal-cell-v1"),
        HarnessSessionModelProfile::new("cheap-harness-session"),
    )
}

fn conflicting_launch_metadata() -> HarnessLaunchMetadata {
    HarnessLaunchMetadata::new(
        HarnessKind::codex(),
        AdapterIdentity::new("other-terminal-adapter"),
        TerminalCellDriverIdentity::new("terminal-cell-v1"),
        HarnessSessionModelProfile::new("cheap-harness-session"),
    )
}

fn different_harness_launch_metadata() -> HarnessLaunchMetadata {
    HarnessLaunchMetadata::new(
        HarnessKind::open_ended_harness(),
        AdapterIdentity::new("open-ended-terminal-adapter"),
        TerminalCellDriverIdentity::new("terminal-cell-v2"),
        HarnessSessionModelProfile::new("other-harness-session"),
    )
}

fn ephemeral_launch_packet() -> MentciPreflightLaunch {
    MentciPreflightLaunch::validated_from_nota(
        &valid_launch_nota().replace("Persistent", "Ephemeral"),
    )
    .expect("ephemeral launch packet")
}

fn conflicting_identity_launch_packet() -> MentciPreflightLaunch {
    MentciPreflightLaunch::validated_from_nota(&valid_launch_nota().replace(
        "(HarnessLabel mentci-harness)",
        "(HarnessLabel different-address-target)",
    ))
    .expect("conflicting identity packet remains valid")
}

fn duplicate_handle_launch_packet() -> MentciPreflightLaunch {
    MentciPreflightLaunch::validated_from_nota(
        &valid_launch_nota().replace("mentci-primary-vxu6", "mentci-primary-other"),
    )
    .expect("duplicate handle packet remains valid")
}

fn named_launch() -> NamedHarnessLaunch {
    NamedHarnessLaunch::new(launch_packet(), terminal_launch(), launch_metadata())
}

fn named_launch_with_liveness(liveness: LivenessPolicy) -> NamedHarnessLaunch {
    NamedHarnessLaunch::new(
        launch_packet(),
        terminal_launch_with_liveness(liveness),
        launch_metadata(),
    )
}

fn address() -> SessionAddress {
    SessionAddress::handle(SessionHandle::new("primary-vxu6-session"))
}

fn named_address() -> NamedSessionAddress {
    NamedSessionAddress::from_identity(launch_packet().session_identity())
}

fn open_or_reuse_request() -> OpenOrReuseHarnessSession {
    OpenOrReuseHarnessSession::new(
        named_address(),
        PersistentSession::Persistent,
        named_launch(),
    )
}

#[test]
fn existing_session_lookup_reuses_address_without_second_terminal_launch() {
    let launcher = FakeLauncher::with_feed_responses(VecDeque::from([
        b"first\n".to_vec(),
        b"second\n".to_vec(),
    ]));
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);
    let liveness = LivenessPolicy::new(StopConditions::new([StopCondition::IdleTimeout(
        IdleTimeout::new(Duration::from_millis(1)),
    )]));

    let first = sessions
        .launch(named_launch_with_liveness(liveness))
        .expect("first launch");
    let second = sessions.launch(named_launch()).expect("lookup reused");
    sessions
        .feed(&address(), TerminalFeed::new(b"first prompt\r".to_vec()))
        .expect("first feed");
    let first_read = sessions.read(&address()).expect("first read");
    sessions
        .feed(&address(), TerminalFeed::new(b"second prompt\r".to_vec()))
        .expect("second feed");
    let second_read = sessions.read(&address()).expect("second read");

    assert_eq!(first, second);
    assert_eq!(launcher.launched().len(), 1);
    assert_eq!(
        launcher.sent(),
        vec![b"first prompt\r".to_vec(), b"second prompt\r".to_vec()]
    );
    assert_eq!(first_read.reason(), &StopReason::IdleTimeout);
    assert_eq!(first_read.transcript().bytes(), b"first\n");
    assert_eq!(second_read.reason(), &StopReason::IdleTimeout);
    assert_eq!(second_read.transcript().bytes(), b"first\nsecond\n");
}

#[test]
fn open_or_reuse_reports_new_persistent_session_opened() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let outcome = sessions
        .open_or_reuse(open_or_reuse_request())
        .expect("new persistent session opened");

    let OpenOrReuseOutcome::Opened(record) = outcome else {
        panic!("new session should be opened");
    };
    assert_eq!(record.named_address(), named_address());
    assert_eq!(record.persistent_session(), PersistentSession::Persistent);
    assert_eq!(launcher.launched().len(), 1);
}

#[test]
fn open_or_reuse_reports_existing_persistent_session_reused() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let opened = sessions
        .open_or_reuse(open_or_reuse_request())
        .expect("new persistent session opened");
    let reused = sessions
        .open_or_reuse(open_or_reuse_request())
        .expect("existing persistent session reused");

    assert!(matches!(opened, OpenOrReuseOutcome::Opened(_)));
    assert!(matches!(reused, OpenOrReuseOutcome::Reused(_)));
    assert_eq!(
        opened.record().named_address(),
        reused.record().named_address()
    );
    assert_eq!(launcher.launched().len(), 1);
}

#[test]
fn open_or_reuse_rejects_address_identity_mismatch_before_terminal_launch() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);
    let mismatched_address =
        NamedSessionAddress::from_identity(duplicate_handle_launch_packet().session_identity());

    let error = sessions
        .open_or_reuse(OpenOrReuseHarnessSession::new(
            mismatched_address,
            PersistentSession::Persistent,
            named_launch(),
        ))
        .expect_err("mismatched address rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::AddressIdentityConflict { .. })
    ));
    assert!(launcher.launched().is_empty());
}

#[test]
fn open_or_reuse_rejects_session_request_mismatch_before_terminal_launch() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let error = sessions
        .open_or_reuse(OpenOrReuseHarnessSession::new(
            named_address(),
            PersistentSession::Persistent,
            NamedHarnessLaunch::new(
                ephemeral_launch_packet(),
                terminal_launch(),
                launch_metadata(),
            ),
        ))
        .expect_err("session request mismatch rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::SessionRequestConflict { .. })
    ));
    assert!(launcher.launched().is_empty());
}

#[test]
fn open_or_reuse_rejects_non_persistent_request_before_terminal_launch() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let error = sessions
        .open_or_reuse(OpenOrReuseHarnessSession::new(
            named_address(),
            PersistentSession::Ephemeral,
            NamedHarnessLaunch::new(
                ephemeral_launch_packet(),
                terminal_launch(),
                launch_metadata(),
            ),
        ))
        .expect_err("non-persistent request rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::NonPersistentSessionRequest { .. })
    ));
    assert!(launcher.launched().is_empty());
}

#[test]
fn unknown_session_diagnostic_resolves_before_terminal_liveness() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let error = sessions
        .feed(&address(), TerminalFeed::new(b"prompt\r".to_vec()))
        .expect_err("unknown session rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::UnknownSession { .. })
    ));
    assert!(launcher.launched().is_empty());
    assert!(launcher.sent().is_empty());
}

#[test]
fn closed_session_diagnostic_prevents_liveness_read() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    sessions.launch(named_launch()).expect("session launched");
    sessions
        .close(&address(), CloseRequest::DriverDefault)
        .expect("session closed");
    let error = sessions
        .read(&address())
        .expect_err("closed session rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::ClosedSession {
            state: SessionRecordState::Closed,
            ..
        })
    ));
    assert_eq!(launcher.launched().len(), 1);
}

#[test]
fn address_metadata_preserves_launch_packet_identity_without_liveness_state() {
    let launch = launch_packet();
    let metadata_source = launch_metadata();
    let record = SessionAddressRecord::from_launch(&launch, &metadata_source);
    let named_address = record.named_address();
    let metadata = record.metadata();

    assert_eq!(named_address.lane_name().as_str(), "mentci-primary-vxu6");
    assert_eq!(named_address.handle().as_str(), "primary-vxu6-session");
    assert_eq!(
        named_address.lookup_path().as_str(),
        "orchestrate/lanes/primary-vxu6"
    );
    assert_eq!(
        metadata.scaffold_identity().as_str(),
        "mentci-prompt-scaffold"
    );
    assert_eq!(metadata.scaffold_version().value(), 1);
    assert_eq!(metadata.lane_metadata().len(), 3);
    assert_eq!(record.launch_metadata(), &metadata_source);
    assert_eq!(
        record.persistent_session(),
        mentci::preflight::PersistentSession::Persistent
    );
    assert_eq!(record.state(), SessionRecordState::Open);
}

#[test]
fn target_identity_resolves_to_stable_address_independent_of_session_request() {
    let persistent = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    let ephemeral =
        SessionAddressRecord::from_launch(&ephemeral_launch_packet(), &launch_metadata());

    assert_eq!(persistent.named_address(), ephemeral.named_address());
    assert_ne!(
        persistent.persistent_session(),
        ephemeral.persistent_session()
    );
    assert_eq!(persistent.metadata(), ephemeral.metadata());
}

#[test]
fn provider_launch_metadata_is_not_address_metadata() {
    let codex = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    let open_ended =
        SessionAddressRecord::from_launch(&launch_packet(), &different_harness_launch_metadata());

    assert_eq!(codex.named_address(), open_ended.named_address());
    assert_eq!(codex.metadata(), open_ended.metadata());
    assert_ne!(codex.launch_metadata(), open_ended.launch_metadata());
}

#[test]
fn address_conflict_diagnostic_prevents_terminal_launch() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let original_launch = launch_packet();
    let original_metadata = launch_metadata();
    let original = SessionAddressRecord::from_launch(&original_launch, &original_metadata);
    directory
        .insert_record(original)
        .expect("original address registered");
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let error = sessions
        .launch(NamedHarnessLaunch::new(
            conflicting_identity_launch_packet(),
            terminal_launch(),
            launch_metadata(),
        ))
        .expect_err("conflicting address rejected");

    assert!(matches!(
        error,
        SessionRoutingError::Lookup(SessionLookupError::AddressConflict { .. })
    ));
    assert!(launcher.launched().is_empty());
}

#[test]
fn session_instance_conflict_is_separate_from_address_conflict() {
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let original = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    directory
        .insert_record(original)
        .expect("original address registered");

    let error = directory
        .insert_record(SessionAddressRecord::from_launch(
            &ephemeral_launch_packet(),
            &launch_metadata(),
        ))
        .expect_err("persistent-session mismatch rejected separately");

    assert!(matches!(
        error,
        SessionLookupError::SessionInstanceConflict { .. }
    ));
}

#[test]
fn launch_metadata_conflict_is_separate_from_address_conflict() {
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let original = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    directory
        .insert_record(original)
        .expect("original address registered");

    let error = directory
        .insert_record(SessionAddressRecord::from_launch(
            &launch_packet(),
            &conflicting_launch_metadata(),
        ))
        .expect_err("launch metadata mismatch rejected separately");

    assert!(matches!(
        error,
        SessionLookupError::LaunchMetadataConflict { .. }
    ));
}

#[test]
fn duplicate_handle_is_a_typed_address_diagnostic() {
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let original_launch = launch_packet();
    let original_metadata = launch_metadata();
    let original = SessionAddressRecord::from_launch(&original_launch, &original_metadata);
    directory
        .insert_record(original)
        .expect("original address registered");
    let duplicate = duplicate_handle_launch_packet();

    let error = directory
        .insert_record(SessionAddressRecord::from_launch(
            &duplicate,
            &launch_metadata(),
        ))
        .expect_err("duplicate handle rejected");

    assert!(matches!(
        error,
        SessionLookupError::DuplicateSessionHandle { .. }
    ));
}

#[test]
fn lane_name_lookup_routes_to_the_same_live_terminal_session() {
    let launcher = FakeLauncher::new(VecDeque::from([
        TerminalObservation::Transcript(b"by lane\n".to_vec()),
        TerminalObservation::CompletionSignaled,
    ]));
    let driver = TerminalCellDriver::new(launcher.clone());
    let directory = InMemoryHarnessSessionDirectory::new();
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let registration = sessions.launch(named_launch()).expect("session launched");
    let by_lane = SessionAddress::lane_name(LaneName::new("mentci-primary-vxu6"));
    let by_named_handle = named_address().as_handle_address();
    sessions
        .feed(&by_lane, TerminalFeed::new(b"lane prompt\r".to_vec()))
        .expect("feed by lane");
    let read = sessions.read(&by_named_handle).expect("read by handle");

    assert_eq!(
        registration.identity().lookup_path().as_str(),
        "orchestrate/lanes/primary-vxu6"
    );
    assert_eq!(launcher.sent(), vec![b"lane prompt\r".to_vec()]);
    assert_eq!(read.transcript().bytes(), b"by lane\n");
}

#[test]
fn stale_address_is_reported_without_claiming_process_health() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let launch = launch_packet();
    let metadata = launch_metadata();
    let record = SessionAddressRecord::from_launch(&launch, &metadata);
    directory
        .insert_record(record)
        .expect("external directory address registered");
    let mut sessions = NamedHarnessSessions::new(directory, driver);

    let error = sessions
        .read(&address())
        .expect_err("address without local terminal handle is stale");

    assert!(matches!(error, SessionRoutingError::StaleSession { .. }));
    assert!(launcher.launched().is_empty());
}

#[test]
fn session_owner_inspection_resolves_record_without_claiming_process_health() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let record = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    directory
        .insert_record(record)
        .expect("external directory address registered");
    let sessions = NamedHarnessSessions::new(directory, driver);

    let inspection = sessions
        .inspect(&named_address())
        .expect("owner record inspected");

    assert_eq!(inspection.record().named_address(), named_address());
    assert_eq!(
        inspection.record().persistent_session(),
        PersistentSession::Persistent
    );
    assert!(launcher.launched().is_empty());
}

#[test]
fn session_owner_inspection_rejects_resolved_address_identity_mismatch() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut directory = InMemoryHarnessSessionDirectory::new();
    let record = SessionAddressRecord::from_launch(&launch_packet(), &launch_metadata());
    directory
        .insert_record(record)
        .expect("external directory address registered");
    let sessions = NamedHarnessSessions::new(directory, driver);
    let mismatched_address =
        NamedSessionAddress::from_identity(duplicate_handle_launch_packet().session_identity());

    let error = sessions
        .inspect(&mismatched_address)
        .expect_err("mismatched resolved address rejected");

    assert!(matches!(
        error,
        SessionLookupError::AddressIdentityConflict { .. }
    ));
    assert!(launcher.launched().is_empty());
}
