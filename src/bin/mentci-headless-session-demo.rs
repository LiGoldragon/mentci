//! mentci-headless-session-demo — a minimal, real proof that a prompt drives a
//! HEADLESS `claude` run (no terminal-cell PTY, no attached TUI), that the
//! observed turn is surfaced through Mentci's own `RenderNota` view path, and
//! that the same session resumes on a later invocation after this process has
//! fully torn down.
//!
//! Each `submit <lane> <prompt>` invocation is a separate short-lived process:
//! the "harness" tears down between turns, so the only durable state carrying a
//! session across turns is the session-id in the tracker file (the minimal
//! stand-in for the orchestrate session store) plus `claude`'s own on-disk
//! session store. A second `submit` for the same lane resumes the tracked
//! session; a tracked session `claude` no longer knows about self-heals into a
//! fresh session instead of failing.
//!
//! Deferred by design and NOT built here: the preflight/prompt-treatment stage,
//! the full orchestrate session store, multi-window Mentci, and terminal-cell
//! hosting. This binary proves the headless spine only.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use harness::ClaudeArtifactObserver;
use mentci_lib::{RenderNota, RenderOrigin};
use nota::NotaEncode;
use serde_json::Value;
use thiserror::Error;

/// The one workspace a headless run must never be pointed at.
const PRIMARY_WORKSPACE: &str = "/home/li/primary";
/// The stable substring `claude` emits when a resume target is unknown.
const SESSION_NOT_FOUND_MARKER: &str = "No conversation found with session ID";

fn main() {
    match Invocation::from_process_arguments().and_then(Invocation::run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("MentciHeadlessSessionDemoBlocked {error}");
            std::process::exit(2);
        }
    }
}

/// The typed boundary error for this demo binary.
#[derive(Debug, Error)]
enum DemoError {
    #[error("usage: mentci-headless-session-demo submit <lane> <prompt> [--model <alias>]")]
    Usage,
    #[error("refused: sandbox {0:?} is inside the primary workspace {PRIMARY_WORKSPACE}")]
    PrimaryWorkspaceRefused(PathBuf),
    #[error("could not mint a session identifier: {0}")]
    SessionIdentifier(std::io::Error),
    #[error("filesystem error at {path:?}: {source}")]
    Filesystem {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to spawn `claude`: {0}")]
    Spawn(std::io::Error),
    #[error("`claude` produced no result event; combined output was:\n{0}")]
    MissingResultEvent(String),
    #[error("observing the claude transcript failed: {0}")]
    Observation(harness::Error),
}

/// The parsed command line. The only supported gesture is `submit`.
struct Invocation {
    lane: LaneName,
    prompt: String,
    model: ModelAlias,
}

impl Invocation {
    fn from_process_arguments() -> Result<Self, DemoError> {
        let mut arguments = std::env::args().skip(1);
        match arguments.next().as_deref() {
            Some("submit") => {}
            _ => return Err(DemoError::Usage),
        }
        let lane = LaneName(arguments.next().ok_or(DemoError::Usage)?);
        let prompt = arguments.next().ok_or(DemoError::Usage)?;
        let mut model = ModelAlias::default();
        while let Some(flag) = arguments.next() {
            match flag.as_str() {
                "--model" => model = ModelAlias(arguments.next().ok_or(DemoError::Usage)?),
                _ => return Err(DemoError::Usage),
            }
        }
        Ok(Self { lane, prompt, model })
    }

    fn run(self) -> Result<(), DemoError> {
        let sandbox = LaunchSandbox::for_lane(&self.lane)?;
        let tracker = SessionTracker::under_sandbox_base();
        let plan = tracker.plan_for(&self.lane)?;

        println!("== mentci-headless-session-demo ==");
        println!("lane      : {}", self.lane);
        println!("sandbox   : {}", sandbox.path().display());
        println!("tracker   : {}", tracker.path().display());
        println!("plan      : {} (session {})", plan.describe(), plan.session());
        println!("prompt    : {}", self.prompt);
        println!();

        let launcher = HeadlessRun::new(sandbox, self.model);
        let (turn, plan_used) = launcher.execute_with_self_heal(plan, &self.prompt)?;

        tracker.record(&self.lane, &turn.session)?;

        let observation = launcher.observe_turn(&self.lane, &turn, plan_used)?;
        launcher.surface_through_mentci_view(&observation);
        println!();
        println!(
            "tracked session-id {} recorded for lane {} (this process now exits; a later \
             `submit {}` resumes it)",
            turn.session, self.lane, self.lane
        );
        Ok(())
    }
}

/// The topic/lane a session belongs to. Domain newtype, never a bare string.
#[derive(Clone, Debug, Eq, PartialEq, NotaEncode)]
struct LaneName(String);

impl fmt::Display for LaneName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A `claude` session identifier (a UUID). Domain newtype.
#[derive(Clone, Debug, Eq, PartialEq, NotaEncode)]
struct ClaudeSessionIdentifier(String);

impl ClaudeSessionIdentifier {
    /// Mint a fresh v4 UUID from the kernel entropy source.
    fn generate() -> Result<Self, DemoError> {
        let raw = fs::read_to_string("/proc/sys/kernel/random/uuid")
            .map_err(DemoError::SessionIdentifier)?;
        Ok(Self(raw.trim().to_string()))
    }

    fn from_existing(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClaudeSessionIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The `claude --model` alias for a turn. Domain newtype with a default.
#[derive(Clone, Debug, NotaEncode)]
struct ModelAlias(String);

impl Default for ModelAlias {
    fn default() -> Self {
        Self("haiku".to_string())
    }
}

impl fmt::Display for ModelAlias {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Whether a turn opens a fresh session or resumes an existing one. A closed
/// typed record, not a `bool is_resume` flag, so the session it carries and the
/// argv it produces stay bound to the variant.
enum HeadlessLaunchPlan {
    Fresh(ClaudeSessionIdentifier),
    Resume(ClaudeSessionIdentifier),
}

impl HeadlessLaunchPlan {
    fn session(&self) -> &ClaudeSessionIdentifier {
        match self {
            Self::Fresh(session) | Self::Resume(session) => session,
        }
    }

    fn describe(&self) -> &'static str {
        match self {
            Self::Fresh(_) => "fresh",
            Self::Resume(_) => "resume",
        }
    }

    /// The confirmed-working headless invocation for `claude` v2.1.198:
    /// `-p` print mode, streamed JSON events, our own session-id on a fresh
    /// turn or `--resume` on a continued turn.
    fn argv(&self, prompt: &str, model: &ModelAlias) -> Vec<String> {
        let mut argv = vec!["-p".to_string(), prompt.to_string()];
        match self {
            Self::Fresh(session) => {
                argv.push("--session-id".to_string());
                argv.push(session.as_str().to_string());
            }
            Self::Resume(session) => {
                argv.push("--resume".to_string());
                argv.push(session.as_str().to_string());
            }
        }
        argv.extend([
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--model".to_string(),
            model.0.clone(),
            "--allowedTools".to_string(),
            String::new(),
        ]);
        argv
    }
}

/// The deterministic per-lane working directory the headless run uses. It is
/// stable across `submit` invocations because `claude --resume` is scoped to
/// the working directory's project: a stable cwd is what lets a later,
/// separately-launched process resume the tracked session.
struct LaunchSandbox {
    directory: PathBuf,
}

impl LaunchSandbox {
    fn for_lane(lane: &LaneName) -> Result<Self, DemoError> {
        let intended = Self::base().join(format!("lane-{}", lane.0));
        // Refuse before any mkdir, so a base pointed at the primary workspace
        // never even creates a directory there.
        Self::refuse_primary(Self::lexically_absolute(&intended)?)?;
        fs::create_dir_all(&intended).map_err(|source| DemoError::Filesystem {
            path: intended.clone(),
            source,
        })?;
        let directory = fs::canonicalize(&intended).map_err(|source| DemoError::Filesystem {
            path: intended.clone(),
            source,
        })?;
        // Re-check after canonicalization to catch a symlink escape into primary.
        let sandbox = Self { directory };
        sandbox.guard_not_primary()?;
        Ok(sandbox)
    }

    fn lexically_absolute(path: &Path) -> Result<PathBuf, DemoError> {
        std::path::absolute(path).map_err(|source| DemoError::Filesystem {
            path: path.to_path_buf(),
            source,
        })
    }

    fn refuse_primary(path: PathBuf) -> Result<(), DemoError> {
        if path.starts_with(PRIMARY_WORKSPACE) {
            return Err(DemoError::PrimaryWorkspaceRefused(path));
        }
        Ok(())
    }

    fn base() -> PathBuf {
        std::env::var_os("MENTCI_HEADLESS_DEMO_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("mentci-headless-session-demo"))
    }

    fn guard_not_primary(&self) -> Result<(), DemoError> {
        Self::refuse_primary(self.directory.clone())
    }

    fn path(&self) -> &Path {
        &self.directory
    }
}

/// The minimal session-id store — the stand-in for orchestrate's session store.
/// One line per lane: `<lane>\t<session-id>`.
struct SessionTracker {
    path: PathBuf,
}

impl SessionTracker {
    fn under_sandbox_base() -> Self {
        Self {
            path: LaunchSandbox::base().join("session-tracker.tsv"),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self, lane: &LaneName) -> Option<ClaudeSessionIdentifier> {
        let contents = fs::read_to_string(&self.path).ok()?;
        contents.lines().find_map(|line| {
            let (recorded_lane, session) = line.split_once('\t')?;
            (recorded_lane == lane.0).then(|| ClaudeSessionIdentifier::from_existing(session))
        })
    }

    fn plan_for(&self, lane: &LaneName) -> Result<HeadlessLaunchPlan, DemoError> {
        match self.load(lane) {
            Some(session) => Ok(HeadlessLaunchPlan::Resume(session)),
            None => Ok(HeadlessLaunchPlan::Fresh(ClaudeSessionIdentifier::generate()?)),
        }
    }

    fn record(&self, lane: &LaneName, session: &ClaudeSessionIdentifier) -> Result<(), DemoError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| DemoError::Filesystem {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut lines: Vec<String> = fs::read_to_string(&self.path)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.starts_with(&format!("{}\t", lane.0)))
            .map(ToString::to_string)
            .collect();
        lines.push(format!("{}\t{}", lane.0, session.as_str()));
        fs::write(&self.path, format!("{}\n", lines.join("\n"))).map_err(|source| {
            DemoError::Filesystem {
                path: self.path.clone(),
                source,
            }
        })
    }
}

/// One headless `claude` turn's captured events.
struct HeadlessTurn {
    session: ClaudeSessionIdentifier,
    response_text: String,
    event_count: u64,
}

/// What a headless launch produced. Typed record over a flag: a `NotFound`
/// resume carries the id that was rejected so the self-heal path can re-plan.
enum HeadlessOutcome {
    Completed(HeadlessTurn),
    SessionNotFound { requested: ClaudeSessionIdentifier },
}

/// Runs headless `claude` turns in a stable sandbox and surfaces them through
/// Mentci's view.
struct HeadlessRun {
    sandbox: LaunchSandbox,
    model: ModelAlias,
}

impl HeadlessRun {
    fn new(sandbox: LaunchSandbox, model: ModelAlias) -> Self {
        Self { sandbox, model }
    }

    /// Execute a plan, self-healing a `NotFound` resume into a fresh session.
    /// Returns the completed turn and the plan variant that produced it.
    fn execute_with_self_heal(
        &self,
        plan: HeadlessLaunchPlan,
        prompt: &str,
    ) -> Result<(HeadlessTurn, &'static str), DemoError> {
        match self.execute(&plan, prompt)? {
            HeadlessOutcome::Completed(turn) => Ok((turn, plan.describe())),
            HeadlessOutcome::SessionNotFound { requested } => {
                println!(
                    "self-heal: tracked session {requested} is gone (claude reported \
                     \"{SESSION_NOT_FOUND_MARKER}\"); re-resuming as a fresh session"
                );
                let fresh = HeadlessLaunchPlan::Fresh(ClaudeSessionIdentifier::generate()?);
                match self.execute(&fresh, prompt)? {
                    HeadlessOutcome::Completed(turn) => Ok((turn, "self-healed-fresh")),
                    HeadlessOutcome::SessionNotFound { requested } => {
                        Err(DemoError::MissingResultEvent(format!(
                            "fresh session {requested} also reported not-found"
                        )))
                    }
                }
            }
        }
    }

    fn execute(
        &self,
        plan: &HeadlessLaunchPlan,
        prompt: &str,
    ) -> Result<HeadlessOutcome, DemoError> {
        let argv = plan.argv(prompt, &self.model);
        let output = Command::new("claude")
            .args(&argv)
            .current_dir(self.sandbox.path())
            .output()
            .map_err(DemoError::Spawn)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        StreamJsonEvents::parse(&stdout).into_outcome(plan, &stdout, &stderr)
    }

    /// Observe the headless turn through the harness JSONL observer (the same
    /// transcript-watch plumbing the design routes into Mentci) and fold it
    /// plus the captured response into the object Mentci renders.
    fn observe_turn(
        &self,
        lane: &LaneName,
        turn: &HeadlessTurn,
        plan_used: &'static str,
    ) -> Result<ClaudeTurnObservation, DemoError> {
        let observer = ClaudeArtifactObserver::new(self.sandbox.path())
            .with_session_identifier(turn.session.as_str());
        let snapshot = self.await_snapshot(&observer, &turn.session)?;
        let recovered = snapshot.recovered_turn();
        Ok(ClaudeTurnObservation {
            lane: lane.clone(),
            session: turn.session.clone(),
            plan: plan_used.to_string(),
            model: recovered.model().unwrap_or("unknown").to_string(),
            stop_reason_end_turn: recovered.has_stop_reason_end_turn(),
            streamed_event_count: turn.event_count,
            tool_call_count: recovered.tool_calls().len() as u64,
            status_transition_count: recovered.status_transitions().len() as u64,
            transcript: snapshot
                .project_jsonl_paths()
                .first()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            response: turn.response_text.clone(),
        })
    }

    /// The transcript is flushed as `claude` exits; poll briefly until the
    /// observer sees this session before snapshotting.
    fn await_snapshot(
        &self,
        observer: &ClaudeArtifactObserver,
        session: &ClaudeSessionIdentifier,
    ) -> Result<harness::ClaudeArtifactSnapshot, DemoError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = observer.snapshot().map_err(DemoError::Observation)?;
            let seen = snapshot.session_identifier() == Some(session.as_str());
            if seen || Instant::now() >= deadline {
                return Ok(snapshot);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Render the observation through Mentci's own `RenderNota` view path — the
    /// exact projection the egui shell paints into its transcript — and print
    /// the rendered block. This is what the psyche sees "through Mentci",
    /// not raw stdout.
    fn surface_through_mentci_view(&self, observation: &ClaudeTurnObservation) {
        let rendered = observation.render_nota(RenderOrigin::Event);
        println!("---- MENTCI VIEW ({}) ----", rendered.origin().label());
        println!("{}", rendered.body());
        println!("--------------------------");
    }
}

/// The parsed stream-json event stream of a single headless turn.
struct StreamJsonEvents {
    events: Vec<Value>,
}

impl StreamJsonEvents {
    fn parse(stdout: &str) -> Self {
        let events = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect();
        Self { events }
    }

    fn result_event(&self) -> Option<&Value> {
        self.events
            .iter()
            .rev()
            .find(|event| event.get("type").and_then(Value::as_str) == Some("result"))
    }

    fn into_outcome(
        self,
        plan: &HeadlessLaunchPlan,
        stdout: &str,
        stderr: &str,
    ) -> Result<HeadlessOutcome, DemoError> {
        if stderr.contains(SESSION_NOT_FOUND_MARKER) {
            return Ok(HeadlessOutcome::SessionNotFound {
                requested: plan.session().clone(),
            });
        }
        let result = self
            .result_event()
            .ok_or_else(|| DemoError::MissingResultEvent(format!("{stdout}\n{stderr}")))?;
        let is_error = result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if is_error {
            let mentions_missing = result
                .get("errors")
                .map(|errors| errors.to_string().contains(SESSION_NOT_FOUND_MARKER))
                .unwrap_or(false);
            if mentions_missing {
                return Ok(HeadlessOutcome::SessionNotFound {
                    requested: plan.session().clone(),
                });
            }
            return Err(DemoError::MissingResultEvent(result.to_string()));
        }
        let session = result
            .get("session_id")
            .and_then(Value::as_str)
            .map(ClaudeSessionIdentifier::from_existing)
            .unwrap_or_else(|| plan.session().clone());
        let response_text = result
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(HeadlessOutcome::Completed(HeadlessTurn {
            session,
            response_text,
            event_count: self.events.len() as u64,
        }))
    }
}

/// The typed observation of one headless turn. This is the object Mentci
/// renders: it is `NotaEncode`, so `mentci_lib`'s blanket `RenderNota` projects
/// it to the NOTA block a Mentci shell displays.
#[derive(Debug, NotaEncode)]
struct ClaudeTurnObservation {
    lane: LaneName,
    session: ClaudeSessionIdentifier,
    plan: String,
    model: String,
    stop_reason_end_turn: bool,
    streamed_event_count: u64,
    tool_call_count: u64,
    status_transition_count: u64,
    transcript: String,
    response: String,
}
