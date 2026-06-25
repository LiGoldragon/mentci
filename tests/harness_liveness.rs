use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use mentci::harness_liveness::{
    CloseReport, CloseRequest, CloseSignal, DriverError, IdleTimeout, LaunchRequest,
    LivenessPolicy, ReadOutcome, SandboxPrivacy, SandboxPrivacyFlag, StalledOutputTimeout,
    StopCondition, StopConditions, StopReason, TerminalCellDriver, TerminalCommand,
    TerminalExitReport, TerminalFeed, TerminalLaunch, TerminalObservation, TerminalSessionLauncher,
    TerminalSessionSurface, TerminalSize, TranscriptCapture, TurnCap, TurnCount,
};

#[derive(Clone)]
struct FakeLauncher {
    launched: Rc<RefCell<Vec<LaunchRequest>>>,
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
    close_report: CloseReport,
}

impl FakeLauncher {
    fn new(events: impl Into<VecDeque<TerminalObservation>>) -> Self {
        Self {
            launched: Rc::new(RefCell::new(Vec::new())),
            events: Rc::new(RefCell::new(events.into())),
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
            sent: self.sent.clone(),
            close_report: self.close_report.clone(),
            transcript: Vec::new(),
        })
    }
}

struct FakeTerminal {
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
    close_report: CloseReport,
    transcript: Vec<u8>,
}

impl TerminalSessionSurface for FakeTerminal {
    fn send(&mut self, feed: TerminalFeed) -> Result<(), DriverError> {
        self.sent.borrow_mut().push(feed.bytes().to_vec());
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

fn launch_request(policy: LivenessPolicy) -> LaunchRequest {
    LaunchRequest::new(TerminalLaunch::new(
        TerminalCommand::new("fake-harness", Vec::<String>::new()),
        TerminalSize::new(24, 80),
    ))
    .with_liveness(policy)
}

#[test]
fn send_and_read_loop_capture_transcript_until_completion_signal() {
    let launcher = FakeLauncher::new(VecDeque::from([
        TerminalObservation::Transcript(b"ready\n".to_vec()),
        TerminalObservation::Transcript(b"answer\n".to_vec()),
        TerminalObservation::CompletionSignaled,
    ]));
    let policy = LivenessPolicy::new(StopConditions::new([
        StopCondition::IdleTimeout(IdleTimeout::new(Duration::from_secs(1))),
        StopCondition::CompletionSignal,
    ]));
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    session
        .send(TerminalFeed::new(b"prompt\r".to_vec()))
        .expect("feed accepted");
    let outcome = session.read_until_stop().expect("read outcome");

    assert_eq!(launcher.sent(), vec![b"prompt\r".to_vec()]);
    assert_eq!(outcome.reason(), &StopReason::CompletionSignal);
    assert_eq!(outcome.transcript().bytes(), b"ready\nanswer\n");
}

#[test]
fn idle_timeout_fires_when_no_output_arrives() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let policy = LivenessPolicy::new(StopConditions::new([StopCondition::IdleTimeout(
        IdleTimeout::new(Duration::from_millis(1)),
    )]));
    let driver = TerminalCellDriver::new(launcher);
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    let outcome = session.read_until_stop().expect("idle outcome");

    assert_eq!(outcome.reason(), &StopReason::IdleTimeout);
    assert!(outcome.transcript().bytes().is_empty());
}

#[test]
fn stalled_output_reports_partial_transcript_after_progress_stops() {
    let launcher = FakeLauncher::new(VecDeque::from([TerminalObservation::Transcript(
        b"partial\n".to_vec(),
    )]));
    let policy = LivenessPolicy::new(StopConditions::new([StopCondition::IdleTimeout(
        IdleTimeout::new(Duration::from_secs(1)),
    )]))
    .with_stalled_output_timeout(StalledOutputTimeout::new(Duration::from_millis(1)));
    let driver = TerminalCellDriver::new(launcher);
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    let outcome = session.read_until_stop().expect("stalled outcome");

    assert_eq!(outcome.reason(), &StopReason::StalledOutput);
    assert_eq!(outcome.transcript().bytes(), b"partial\n");
}

#[test]
fn close_signal_returns_closed_outcome_and_preserves_terminal_input_close() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut session = driver
        .launch(launch_request(LivenessPolicy::default()))
        .expect("session launched");

    let outcome = session
        .close(CloseRequest::TerminalInput(TerminalFeed::new(
            b"exit\r".to_vec(),
        )))
        .expect("closed");

    assert_eq!(launcher.sent(), vec![b"exit\r".to_vec()]);
    assert_eq!(
        outcome.reason(),
        &StopReason::Closed(CloseReport::new(CloseSignal::DriverDefault))
    );
}

#[test]
fn terminal_exit_is_a_final_liveness_outcome() {
    let exit = TerminalExitReport::new("exit status: 7");
    let launcher = FakeLauncher::new(VecDeque::from([TerminalObservation::TerminalExit(
        exit.clone(),
    )]));
    let policy = LivenessPolicy::new(StopConditions::new([StopCondition::IdleTimeout(
        IdleTimeout::new(Duration::from_secs(1)),
    )]));
    let driver = TerminalCellDriver::new(launcher);
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    let outcome = session.read_until_stop().expect("exit outcome");

    assert_eq!(outcome.reason(), &StopReason::TerminalExit(exit));
}

#[test]
fn turn_cap_is_typed_and_not_schema_free_text() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let policy = LivenessPolicy::new(StopConditions::new([StopCondition::TurnCap(TurnCap::new(
        TurnCount::new(1),
    ))]));
    let driver = TerminalCellDriver::new(launcher);
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    let outcome = session
        .send(TerminalFeed::new(b"first turn\r".to_vec()))
        .expect("feed accepted")
        .expect("turn cap reached");

    assert_eq!(outcome.reason(), &StopReason::TurnCap(TurnCount::new(1)));
}

#[test]
fn launch_preserves_sandbox_privacy_flags_at_driver_boundary() {
    let launcher = FakeLauncher::new(VecDeque::new());
    let driver = TerminalCellDriver::new(launcher.clone());
    let request =
        launch_request(LivenessPolicy::default()).with_sandbox_privacy(SandboxPrivacy::new([
            SandboxPrivacyFlag::SandboxedJjTask,
            SandboxPrivacyFlag::PrimaryJjForbidden,
            SandboxPrivacyFlag::PrivateScopeClosed,
        ]));

    let _session = driver.launch(request).expect("session launched");

    let launched = launcher.launched();
    assert_eq!(
        launched[0].sandbox_privacy().flags(),
        &[
            SandboxPrivacyFlag::SandboxedJjTask,
            SandboxPrivacyFlag::PrimaryJjForbidden,
            SandboxPrivacyFlag::PrivateScopeClosed,
        ]
    );
}

#[test]
fn transcript_events_are_provider_agnostic_bytes() {
    let launcher = FakeLauncher::new(VecDeque::from([
        TerminalObservation::Transcript(b"provider-specific words stay raw\n".to_vec()),
        TerminalObservation::CompletionSignaled,
    ]));
    let policy = LivenessPolicy::new(StopConditions::new([StopCondition::CompletionSignal]));
    let driver = TerminalCellDriver::new(launcher);
    let mut session = driver
        .launch(launch_request(policy))
        .expect("session launched");

    let outcome: ReadOutcome = session.read_until_stop().expect("read outcome");

    assert_eq!(outcome.reason(), &StopReason::CompletionSignal);
    assert_eq!(
        outcome.transcript().to_string_lossy(),
        "provider-specific words stay raw\n"
    );
}

#[cfg(feature = "terminal-cell-runtime")]
#[test]
fn terminal_cell_runtime_launches_local_command_and_captures_transcript() {
    let marker = "mentci-terminal-cell-smoke";
    let driver = TerminalCellDriver::<mentci::harness_liveness::TerminalCellLauncher>::default();
    let request = LaunchRequest::new(TerminalLaunch::new(
        TerminalCommand::new(
            "/bin/sh",
            vec!["-c".to_string(), format!("printf {marker}")],
        ),
        TerminalSize::new(24, 80),
    ))
    .with_liveness(LivenessPolicy::new(StopConditions::new([
        StopCondition::IdleTimeout(IdleTimeout::new(Duration::from_secs(2))),
    ])));
    let mut session = driver
        .launch(request)
        .expect("terminal-cell session launched");

    let outcome = session
        .read_until_stop()
        .expect("terminal-cell read outcome");
    let transcript = session
        .transcript()
        .expect("terminal-cell transcript snapshot")
        .to_string_lossy();

    assert!(
        matches!(outcome.reason(), StopReason::TerminalExit(_)),
        "expected terminal exit from trivial command, got {:?}",
        outcome.reason()
    );
    assert!(
        transcript.contains(marker),
        "expected transcript to contain {marker:?}, got {transcript:?}"
    );
}
