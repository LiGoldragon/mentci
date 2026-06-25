use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCommand {
    program: String,
    arguments: Vec<String>,
}

impl TerminalCommand {
    pub fn new(program: impl Into<String>, arguments: impl Into<Vec<String>>) -> Self {
        Self {
            program: program.into(),
            arguments: arguments.into(),
        }
    }

    pub fn program(&self) -> &str {
        self.program.as_str()
    }

    pub fn arguments(&self) -> &[String] {
        self.arguments.as_slice()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    rows: u16,
    columns: u16,
}

impl TerminalSize {
    pub const fn new(rows: u16, columns: u16) -> Self {
        Self { rows, columns }
    }

    pub const fn rows(self) -> u16 {
        self.rows
    }

    pub const fn columns(self) -> u16 {
        self.columns
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLaunch {
    command: TerminalCommand,
    size: TerminalSize,
    working_directory: Option<TerminalWorkingDirectory>,
}

impl TerminalLaunch {
    pub fn new(command: TerminalCommand, size: TerminalSize) -> Self {
        Self {
            command,
            size,
            working_directory: None,
        }
    }

    pub fn with_working_directory(mut self, working_directory: TerminalWorkingDirectory) -> Self {
        self.working_directory = Some(working_directory);
        self
    }

    pub fn command(&self) -> &TerminalCommand {
        &self.command
    }

    pub const fn size(&self) -> TerminalSize {
        self.size
    }

    pub fn working_directory(&self) -> Option<&TerminalWorkingDirectory> {
        self.working_directory.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalWorkingDirectory {
    path: PathBuf,
}

impl TerminalWorkingDirectory {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn as_path(&self) -> &Path {
        self.path.as_path()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    launch: TerminalLaunch,
    initial_input: Option<TerminalFeed>,
    liveness: LivenessPolicy,
    sandbox_privacy: SandboxPrivacy,
}

impl LaunchRequest {
    pub fn new(launch: TerminalLaunch) -> Self {
        Self {
            launch,
            initial_input: None,
            liveness: LivenessPolicy::default(),
            sandbox_privacy: SandboxPrivacy::default(),
        }
    }

    pub fn with_initial_input(mut self, input: TerminalFeed) -> Self {
        self.initial_input = Some(input);
        self
    }

    pub fn with_liveness(mut self, liveness: LivenessPolicy) -> Self {
        self.liveness = liveness;
        self
    }

    pub fn with_sandbox_privacy(mut self, sandbox_privacy: SandboxPrivacy) -> Self {
        self.sandbox_privacy = sandbox_privacy;
        self
    }

    pub fn launch(&self) -> &TerminalLaunch {
        &self.launch
    }

    pub fn initial_input(&self) -> Option<&TerminalFeed> {
        self.initial_input.as_ref()
    }

    pub fn liveness(&self) -> &LivenessPolicy {
        &self.liveness
    }

    pub fn sandbox_privacy(&self) -> &SandboxPrivacy {
        &self.sandbox_privacy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SandboxPrivacy {
    flags: Vec<SandboxPrivacyFlag>,
}

impl SandboxPrivacy {
    pub fn new(flags: impl Into<Vec<SandboxPrivacyFlag>>) -> Self {
        Self {
            flags: flags.into(),
        }
    }

    pub fn flags(&self) -> &[SandboxPrivacyFlag] {
        self.flags.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxPrivacyFlag {
    SandboxedJjTask,
    PrimaryJjForbidden,
    PrivateScopeClosed,
    AdapterBoundary(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFeed {
    bytes: Vec<u8>,
}

impl TerminalFeed {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LivenessPolicy {
    stop_conditions: StopConditions,
    stalled_output_timeout: Option<StalledOutputTimeout>,
}

impl LivenessPolicy {
    pub fn new(stop_conditions: StopConditions) -> Self {
        Self {
            stop_conditions,
            stalled_output_timeout: None,
        }
    }

    pub fn with_stalled_output_timeout(mut self, timeout: StalledOutputTimeout) -> Self {
        self.stalled_output_timeout = Some(timeout);
        self
    }

    pub fn stop_conditions(&self) -> &StopConditions {
        &self.stop_conditions
    }

    pub fn stalled_output_timeout(&self) -> Option<StalledOutputTimeout> {
        self.stalled_output_timeout
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopConditions {
    conditions: Vec<StopCondition>,
}

impl StopConditions {
    pub fn new(conditions: impl Into<Vec<StopCondition>>) -> Self {
        Self {
            conditions: conditions.into(),
        }
    }

    pub fn conditions(&self) -> &[StopCondition] {
        self.conditions.as_slice()
    }

    fn idle_timeout(&self) -> Option<IdleTimeout> {
        self.conditions
            .iter()
            .find_map(|condition| match condition {
                StopCondition::IdleTimeout(timeout) => Some(*timeout),
                StopCondition::TurnCap(_) | StopCondition::CompletionSignal => None,
            })
    }

    fn turn_cap(&self) -> Option<TurnCap> {
        self.conditions
            .iter()
            .find_map(|condition| match condition {
                StopCondition::TurnCap(cap) => Some(*cap),
                StopCondition::IdleTimeout(_) | StopCondition::CompletionSignal => None,
            })
    }

    fn completion_signal_is_enabled(&self) -> bool {
        self.conditions
            .iter()
            .any(|condition| matches!(condition, StopCondition::CompletionSignal))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopCondition {
    IdleTimeout(IdleTimeout),
    TurnCap(TurnCap),
    CompletionSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleTimeout(Duration);

impl IdleTimeout {
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    pub const fn duration(self) -> Duration {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StalledOutputTimeout(Duration);

impl StalledOutputTimeout {
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    pub const fn duration(self) -> Duration {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TurnCount(u64);

impl TurnCount {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn into_u64(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnCap(TurnCount);

impl TurnCap {
    pub const fn new(count: TurnCount) -> Self {
        Self(count)
    }

    pub const fn count(self) -> TurnCount {
        self.0
    }

    fn is_reached_by(self, count: TurnCount) -> bool {
        count >= self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TranscriptCapture {
    bytes: Vec<u8>,
}

impl TranscriptCapture {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    fn append(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn has_output(&self) -> bool {
        !self.bytes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOutcome {
    reason: StopReason,
    transcript: TranscriptCapture,
}

impl ReadOutcome {
    fn new(reason: StopReason, transcript: TranscriptCapture) -> Self {
        Self { reason, transcript }
    }

    pub fn reason(&self) -> &StopReason {
        &self.reason
    }

    pub fn transcript(&self) -> &TranscriptCapture {
        &self.transcript
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    IdleTimeout,
    StalledOutput,
    TurnCap(TurnCount),
    CompletionSignal,
    TerminalExit(TerminalExitReport),
    Closed(CloseReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalObservation {
    Transcript(Vec<u8>),
    CompletionSignaled,
    TerminalExit(TerminalExitReport),
    WorkerLifecycle(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExitReport {
    status: String,
}

impl TerminalExitReport {
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
        }
    }

    pub fn status(&self) -> &str {
        self.status.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseRequest {
    DriverDefault,
    TerminalInput(TerminalFeed),
    Interrupt,
    Terminate,
    Kill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseReport {
    signal: CloseSignal,
}

impl CloseReport {
    pub fn new(signal: CloseSignal) -> Self {
        Self { signal }
    }

    pub fn signal(&self) -> &CloseSignal {
        &self.signal
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseSignal {
    DriverDefault,
    TerminalInput,
    Interrupt,
    Terminate,
    Kill,
}

pub trait TerminalSessionLauncher {
    type Session: TerminalSessionSurface;

    fn launch(&self, request: LaunchRequest) -> Result<Self::Session, DriverError>;
}

pub trait TerminalSessionSurface {
    fn send(&mut self, feed: TerminalFeed) -> Result<(), DriverError>;

    fn read_event(&mut self, timeout: Duration)
    -> Result<Option<TerminalObservation>, DriverError>;

    fn transcript(&mut self) -> Result<TranscriptCapture, DriverError>;

    fn close(&mut self, request: CloseRequest) -> Result<CloseReport, DriverError>;
}

#[cfg(feature = "terminal-cell-runtime")]
pub use terminal_cell_runtime::{TerminalCellLauncher, TerminalCellSurface};

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("terminal launch failed: {0}")]
    TerminalLaunch(String),

    #[error("terminal write failed: {0}")]
    TerminalWrite(String),

    #[error("terminal read failed: {0}")]
    TerminalRead(String),

    #[error("terminal close failed: {0}")]
    TerminalClose(String),
}

pub struct TerminalCellDriver<Launcher> {
    launcher: Launcher,
}

impl<Launcher> TerminalCellDriver<Launcher>
where
    Launcher: TerminalSessionLauncher,
{
    pub fn new(launcher: Launcher) -> Self {
        Self { launcher }
    }

    pub fn launch(
        &self,
        request: LaunchRequest,
    ) -> Result<LiveHarnessSession<Launcher::Session>, DriverError> {
        let liveness = request.liveness().clone();
        let initial_input = request.initial_input().cloned();
        let terminal = self.launcher.launch(request)?;
        let mut session = LiveHarnessSession::new(terminal, liveness);
        if let Some(input) = initial_input {
            session.send(input)?;
        }
        Ok(session)
    }
}

#[cfg(feature = "terminal-cell-runtime")]
impl Default for TerminalCellDriver<TerminalCellLauncher> {
    fn default() -> Self {
        Self {
            launcher: TerminalCellLauncher,
        }
    }
}

pub struct LiveHarnessSession<Terminal> {
    terminal: Terminal,
    liveness: LivenessPolicy,
    transcript: TranscriptCapture,
    turn_count: TurnCount,
    completion_signaled: CompletionSignalState,
    closed: Option<CloseReport>,
}

impl<Terminal> LiveHarnessSession<Terminal>
where
    Terminal: TerminalSessionSurface,
{
    pub fn new(terminal: Terminal, liveness: LivenessPolicy) -> Self {
        Self {
            terminal,
            liveness,
            transcript: TranscriptCapture::new(),
            turn_count: TurnCount::new(0),
            completion_signaled: CompletionSignalState::Open,
            closed: None,
        }
    }

    pub fn send(&mut self, feed: TerminalFeed) -> Result<Option<ReadOutcome>, DriverError> {
        self.terminal.send(feed)?;
        self.turn_count = self.turn_count.next();
        Ok(self.turn_cap_outcome())
    }

    pub fn read_until_stop(&mut self) -> Result<ReadOutcome, DriverError> {
        let started = Instant::now();
        let mut last_output = None;
        loop {
            if let Some(outcome) = self.completion_signal_outcome() {
                return Ok(outcome);
            }
            if let Some(outcome) = self.turn_cap_outcome() {
                return Ok(outcome);
            }
            if let Some(report) = &self.closed {
                return Ok(ReadOutcome::new(
                    StopReason::Closed(report.clone()),
                    self.transcript.clone(),
                ));
            }

            let deadline = self.next_read_deadline(started, last_output);
            if deadline.duration().is_zero() {
                return Ok(self.timeout_outcome(deadline.kind()));
            }

            match self.terminal.read_event(deadline.duration())? {
                Some(TerminalObservation::Transcript(bytes)) => {
                    self.transcript.append(&bytes);
                    last_output = Some(Instant::now());
                }
                Some(TerminalObservation::CompletionSignaled) => {
                    self.completion_signaled = CompletionSignalState::Signaled;
                }
                Some(TerminalObservation::TerminalExit(exit)) => {
                    return Ok(ReadOutcome::new(
                        StopReason::TerminalExit(exit),
                        self.transcript.clone(),
                    ));
                }
                Some(TerminalObservation::WorkerLifecycle(_)) => {}
                None => return Ok(self.timeout_outcome(deadline.kind())),
            }
        }
    }

    pub fn close(&mut self, request: CloseRequest) -> Result<ReadOutcome, DriverError> {
        let report = self.terminal.close(request)?;
        self.closed = Some(report.clone());
        Ok(ReadOutcome::new(
            StopReason::Closed(report),
            self.transcript.clone(),
        ))
    }

    pub fn transcript(&mut self) -> Result<TranscriptCapture, DriverError> {
        let terminal_transcript = self.terminal.transcript()?;
        if terminal_transcript.bytes().len() > self.transcript.bytes().len() {
            self.transcript = terminal_transcript;
        }
        Ok(self.transcript.clone())
    }

    fn completion_signal_outcome(&self) -> Option<ReadOutcome> {
        if self
            .liveness
            .stop_conditions()
            .completion_signal_is_enabled()
            && self.completion_signaled == CompletionSignalState::Signaled
        {
            Some(ReadOutcome::new(
                StopReason::CompletionSignal,
                self.transcript.clone(),
            ))
        } else {
            None
        }
    }

    fn turn_cap_outcome(&self) -> Option<ReadOutcome> {
        let cap = self.liveness.stop_conditions().turn_cap()?;
        if cap.is_reached_by(self.turn_count) {
            Some(ReadOutcome::new(
                StopReason::TurnCap(self.turn_count),
                self.transcript.clone(),
            ))
        } else {
            None
        }
    }

    fn next_read_deadline(&self, started: Instant, last_output: Option<Instant>) -> ReadDeadline {
        let idle = self
            .liveness
            .stop_conditions()
            .idle_timeout()
            .map(|timeout| {
                ReadDeadline::new(
                    timeout.duration().saturating_sub(started.elapsed()),
                    TimeoutKind::Idle,
                )
            });
        let stalled =
            self.liveness
                .stalled_output_timeout()
                .zip(last_output)
                .map(|(timeout, output)| {
                    ReadDeadline::new(
                        timeout.duration().saturating_sub(output.elapsed()),
                        TimeoutKind::StalledOutput,
                    )
                });

        match (idle, stalled) {
            (Some(idle), Some(stalled)) => idle.min(stalled),
            (Some(idle), None) => idle,
            (None, Some(stalled)) => stalled,
            (None, None) => ReadDeadline::new(Duration::from_millis(100), TimeoutKind::Idle),
        }
    }

    fn timeout_outcome(&self, kind: TimeoutKind) -> ReadOutcome {
        match kind {
            TimeoutKind::Idle => ReadOutcome::new(StopReason::IdleTimeout, self.transcript.clone()),
            TimeoutKind::StalledOutput if self.transcript.has_output() => {
                ReadOutcome::new(StopReason::StalledOutput, self.transcript.clone())
            }
            TimeoutKind::StalledOutput => {
                ReadOutcome::new(StopReason::IdleTimeout, self.transcript.clone())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadDeadline {
    duration: Duration,
    kind: TimeoutKind,
}

impl ReadDeadline {
    const fn new(duration: Duration, kind: TimeoutKind) -> Self {
        Self { duration, kind }
    }

    const fn duration(self) -> Duration {
        self.duration
    }

    const fn kind(self) -> TimeoutKind {
        self.kind
    }

    fn min(self, other: Self) -> Self {
        if self.duration <= other.duration {
            self
        } else {
            other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeoutKind {
    Idle,
    StalledOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionSignalState {
    Open,
    Signaled,
}

#[cfg(feature = "terminal-cell-runtime")]
mod terminal_cell_runtime {
    use std::collections::VecDeque;
    use std::time::Duration;

    use kameo::actor::ActorRef;
    use terminal_cell::{
        InputSource, TerminalCell, TerminalCellSession, TerminalExitRequest, TerminalInput,
        TranscriptDelta, TranscriptSnapshotRequest, TranscriptSubscription,
        TranscriptSubscriptionRequest,
    };

    use crate::harness_liveness::{
        CloseReport, CloseRequest, CloseSignal, DriverError, LaunchRequest, TerminalCommand,
        TerminalExitReport, TerminalFeed, TerminalLaunch, TerminalObservation,
        TerminalSessionLauncher, TerminalSessionSurface, TerminalSize, TerminalWorkingDirectory,
        TranscriptCapture,
    };

    #[derive(Debug, Clone, Copy, Default)]
    pub struct TerminalCellLauncher;

    impl TerminalSessionLauncher for TerminalCellLauncher {
        type Session = TerminalCellSurface;

        fn launch(&self, request: LaunchRequest) -> Result<Self::Session, DriverError> {
            TerminalCellSurface::from_launch(request.launch().clone())
        }
    }

    pub struct TerminalCellSurface {
        session: TerminalCellSession,
        actor: ActorRef<TerminalCell>,
        runtime: tokio::runtime::Runtime,
        transcript_replay: VecDeque<TranscriptDelta>,
        transcript_subscription: TranscriptSubscription,
    }

    impl TerminalCellSurface {
        pub fn from_launch(launch: TerminalLaunch) -> Result<Self, DriverError> {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|error| DriverError::TerminalRead(error.to_string()))?;
            let session = runtime.block_on(async {
                TerminalCell::spawn_session(launch.into_terminal_cell_launch())
            });
            let actor = session.actor();
            let subscription = runtime
                .block_on(async {
                    actor
                        .ask(TranscriptSubscriptionRequest::from_beginning())
                        .send()
                        .await
                })
                .map_err(|error| DriverError::TerminalRead(error.to_string()))?;
            let transcript_replay = subscription.replay().iter().cloned().collect();
            Ok(Self {
                session,
                actor,
                runtime,
                transcript_replay,
                transcript_subscription: subscription,
            })
        }

        fn terminal_exit(&self) -> Result<Option<TerminalExitReport>, DriverError> {
            let exit = self
                .runtime
                .block_on(async { self.actor.ask(TerminalExitRequest).send().await })
                .map_err(|error| DriverError::TerminalRead(error.to_string()))?;
            Ok(exit.map(|exit| TerminalExitReport::new(exit.status())))
        }

        fn next_transcript_delta(
            &mut self,
            timeout: Duration,
        ) -> Result<Option<TranscriptDelta>, DriverError> {
            if let Some(delta) = self.transcript_replay.pop_front() {
                return Ok(Some(delta));
            }

            match self.runtime.block_on(async {
                tokio::time::timeout(timeout, self.transcript_subscription.next_live_delta()).await
            }) {
                Ok(delta) => Ok(delta),
                Err(_) => Ok(None),
            }
        }
    }

    impl TerminalSessionSurface for TerminalCellSurface {
        fn send(&mut self, feed: TerminalFeed) -> Result<(), DriverError> {
            self.session
                .input_port()
                .accept(TerminalInput::new(
                    feed.bytes().to_vec(),
                    InputSource::Programmatic,
                ))
                .map(|_| ())
                .map_err(|error| DriverError::TerminalWrite(error.to_string()))
        }

        fn read_event(
            &mut self,
            timeout: Duration,
        ) -> Result<Option<TerminalObservation>, DriverError> {
            if let Some(exit) = self.terminal_exit()? {
                return Ok(Some(TerminalObservation::TerminalExit(exit)));
            }

            match self.next_transcript_delta(timeout)? {
                Some(delta) => Ok(Some(TerminalObservation::Transcript(
                    delta.bytes().to_vec(),
                ))),
                None => self
                    .terminal_exit()
                    .map(|exit| exit.map(TerminalObservation::TerminalExit)),
            }
        }

        fn transcript(&mut self) -> Result<TranscriptCapture, DriverError> {
            let snapshot = self
                .runtime
                .block_on(async { self.actor.ask(TranscriptSnapshotRequest).send().await })
                .map_err(|error| DriverError::TerminalRead(error.to_string()))?;
            Ok(TranscriptCapture::from_bytes(snapshot.bytes().to_vec()))
        }

        fn close(&mut self, request: CloseRequest) -> Result<CloseReport, DriverError> {
            let signal = match request {
                CloseRequest::DriverDefault => CloseSignal::DriverDefault,
                CloseRequest::TerminalInput(feed) => {
                    self.send(feed)?;
                    CloseSignal::TerminalInput
                }
                CloseRequest::Interrupt => CloseSignal::Interrupt,
                CloseRequest::Terminate => CloseSignal::Terminate,
                CloseRequest::Kill => CloseSignal::Kill,
            };
            self.actor.kill();
            Ok(CloseReport::new(signal))
        }
    }

    impl TerminalLaunch {
        fn into_terminal_cell_launch(self) -> terminal_cell::TerminalLaunch {
            let command = match self.working_directory {
                Some(working_directory) => self.command.with_working_directory(working_directory),
                None => self.command,
            };
            terminal_cell::TerminalLaunch::new(
                command.into_terminal_cell_command(),
                self.size.into_terminal_cell_size(),
            )
        }
    }

    impl TerminalCommand {
        fn with_working_directory(self, working_directory: TerminalWorkingDirectory) -> Self {
            let mut arguments = Vec::with_capacity(self.arguments.len() + 3);
            arguments.push("-C".to_owned());
            arguments.push(working_directory.as_path().to_string_lossy().into_owned());
            arguments.push(self.program);
            arguments.extend(self.arguments);
            Self::new("env", arguments)
        }

        fn into_terminal_cell_command(self) -> terminal_cell::TerminalCommand {
            terminal_cell::TerminalCommand::new(self.program, self.arguments)
        }
    }

    impl TerminalSize {
        fn into_terminal_cell_size(self) -> terminal_cell::TerminalSize {
            terminal_cell::TerminalSize::new(self.rows, self.columns)
        }
    }
}
