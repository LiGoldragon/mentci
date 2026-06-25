use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use mentci::harness_adapters::{
    AdapterError, ClaudeCodeAdapter, ClaudeCodeLaunchRequest, ClaudeCodeTranscriptCursor,
    EphemeralJjRepository, HarnessFeed, HarnessPrompt,
};
use mentci::harness_liveness::{
    CloseReport, CloseRequest, CloseSignal, DriverError, StopReason, TerminalCellDriver,
    TerminalFeed, TerminalObservation, TerminalSessionLauncher, TerminalSessionSurface,
    TranscriptCapture,
};
use mentci::harness_sessions::{
    InMemoryHarnessSessionDirectory, NamedHarnessSessions, SessionAddress,
};
use mentci::preflight::{MentciPreflightLaunch, SessionHandle};

const EPHEMERAL_SANDBOX_MARKER: &str = ".mentci-ephemeral-jj-sandbox";
const PRIMARY_WORKSPACE: &str = "/home/li/primary";
const EPHEMERAL_SANDBOX_PREFIX: &str = "mentci-jj-proof-";
const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";
const FORBIDDEN_CLAUDE_SUBSCRIPTION_TUI_ARGUMENTS: &[&str] =
    &["--bare", "--print", "--model", "--permission-mode"];

#[derive(Clone)]
struct FakeLauncher {
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    feed_responses: Rc<RefCell<VecDeque<Vec<u8>>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl FakeLauncher {
    fn new(responses: impl Into<VecDeque<Vec<u8>>>) -> Self {
        Self {
            events: Rc::new(RefCell::new(VecDeque::new())),
            feed_responses: Rc::new(RefCell::new(responses.into())),
            sent: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.borrow().clone()
    }
}

impl TerminalSessionLauncher for FakeLauncher {
    type Session = FakeTerminal;

    fn launch(
        &self,
        _request: mentci::harness_liveness::LaunchRequest,
    ) -> Result<Self::Session, DriverError> {
        Ok(FakeTerminal {
            events: self.events.clone(),
            feed_responses: self.feed_responses.clone(),
            sent: self.sent.clone(),
            transcript: Vec::new(),
        })
    }
}

struct FakeTerminal {
    events: Rc<RefCell<VecDeque<TerminalObservation>>>,
    feed_responses: Rc<RefCell<VecDeque<Vec<u8>>>>,
    sent: Rc<RefCell<Vec<Vec<u8>>>>,
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
        Ok(CloseReport::new(CloseSignal::TerminalInput))
    }
}

fn valid_claude_launch_nota() -> &'static str {
    r#"(MentciPreflightLaunch
  (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred)
  ([(beads skills/beads.md [claim and update the bead])
    (rust-discipline skills/rust-discipline.md [implement the adapter])]
   (cheap-contained-preflight subscription-tui-default)
   (ClaudeCode claude-code-terminal-adapter terminal-cell-v1)
   [Prompt requires a sandboxed jj task and a persistent named harness session])
  (mentci-primary-edm1 [(Bead primary-edm1) (WorkSurface sandboxed-jj-task) (HarnessLabel mentci-harness)] primary-edm1-session orchestrate/lanes/primary-edm1)
  Persistent
  (SandboxedJjTask PrimaryForbidden PrivateScopeClosed)
  [(IdleTimeout 1) (TurnCap 8) CompletionSignal]
  [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary)])"#
}

fn launch_packet() -> MentciPreflightLaunch {
    MentciPreflightLaunch::validated_from_nota(valid_claude_launch_nota())
        .expect("valid claude launch")
}

fn sandbox_directory(parent: &Path) -> EphemeralJjRepository {
    let path = parent.join("sandbox");
    fs::create_dir_all(path.join(".jj")).expect("jj directory");
    fs::write(
        path.join(EPHEMERAL_SANDBOX_MARKER),
        "mentci ephemeral jj sandbox\n",
    )
    .expect("sandbox marker");
    EphemeralJjRepository::from_existing_ephemeral(path).expect("ephemeral sandbox")
}

fn proof_directory_names(parent: &Path) -> Vec<String> {
    let mut names = fs::read_dir(parent)
        .expect("read primary guard directory")
        .filter_map(|entry| {
            let name = entry.expect("directory entry").file_name();
            let name = name.to_string_lossy();
            name.starts_with(EPHEMERAL_SANDBOX_PREFIX)
                .then(|| name.into_owned())
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn assert_create_in_rejects_primary_scope_without_creating_sandbox(parent: &Path) {
    let before = proof_directory_names(parent);

    let error = EphemeralJjRepository::create_in(parent).expect_err("primary scope rejected");

    assert!(matches!(
        error,
        AdapterError::SandboxViolation { reason, .. }
            if reason.contains("primary workspace")
    ));
    assert_eq!(
        proof_directory_names(parent),
        before,
        "primary rejection created a mentci jj proof sandbox under {parent:?}"
    );
    assert!(
        !parent.join(EPHEMERAL_SANDBOX_MARKER).exists(),
        "primary rejection wrote an ephemeral marker at {parent:?}"
    );
}

fn launch_request(parent: &Path) -> ClaudeCodeLaunchRequest {
    ClaudeCodeLaunchRequest::new(
        launch_packet(),
        parent.join("scaffold"),
        sandbox_directory(parent),
        HarnessPrompt::new("show jj status and wait for the next turn"),
    )
}

fn address() -> SessionAddress {
    SessionAddress::handle(SessionHandle::new("primary-edm1-session"))
}

fn framed_tui_input(text: impl AsRef<str>) -> Vec<u8> {
    let text = text.as_ref();
    let mut bytes = Vec::with_capacity(
        BRACKETED_PASTE_START.len() + text.len() + BRACKETED_PASTE_END.len() + 1,
    );
    bytes.extend_from_slice(BRACKETED_PASTE_START);
    bytes.extend_from_slice(text.as_bytes());
    bytes.extend_from_slice(BRACKETED_PASTE_END);
    bytes.push(b'\r');
    bytes
}

fn assert_subscription_tui_arguments(arguments: &[String]) {
    for forbidden in FORBIDDEN_CLAUDE_SUBSCRIPTION_TUI_ARGUMENTS {
        assert!(
            !arguments.iter().any(|argument| argument == forbidden),
            "subscription TUI launch must not include forbidden argument {forbidden}: {arguments:?}"
        );
    }
    assert!(
        !arguments
            .windows(2)
            .any(|window| window == ["--permission-mode", "bypassPermissions"]),
        "subscription TUI launch must not bypass permissions: {arguments:?}"
    );
    assert!(
        !arguments.iter().any(|argument| {
            argument.contains("ANTHROPIC_API_KEY") || argument.contains("apiKeyHelper")
        }),
        "subscription TUI launch must not carry API auth assumptions: {arguments:?}"
    );
}

#[test]
fn claude_code_adapter_builds_subscription_tui_terminal_launch_plan() {
    let directory = tempfile::tempdir().expect("tempdir");
    let adapter = ClaudeCodeAdapter::new();

    let launch = adapter
        .launch(launch_request(directory.path()))
        .expect("adapter launch");
    let terminal_launch = launch.terminal_launch();
    let command = terminal_launch.launch().command();

    assert_eq!(command.program(), "claude");
    assert_eq!(
        command.arguments(),
        &[
            "--add-dir".to_owned(),
            directory
                .path()
                .join("scaffold")
                .to_string_lossy()
                .into_owned(),
            "--name".to_owned(),
            "mentci-primary-edm1".to_owned(),
        ]
    );
    assert_subscription_tui_arguments(command.arguments());
    assert_eq!(
        terminal_launch
            .launch()
            .working_directory()
            .expect("working directory")
            .as_path(),
        directory.path().join("sandbox").as_path()
    );
    assert!(
        terminal_launch
            .initial_input()
            .expect("initial input")
            .bytes()
            .windows(b"/home/li/primary".len())
            .any(|window| window == b"/home/li/primary")
    );
}

#[test]
fn claude_code_adapter_does_not_require_harness_model_identifier() {
    let directory = tempfile::tempdir().expect("tempdir");
    let adapter = ClaudeCodeAdapter::new();
    let launch = MentciPreflightLaunch::validated_from_nota(
        &valid_claude_launch_nota().replace("subscription-tui-default", "cheap-harness-session"),
    )
    .expect("syntactically valid launch");

    let named_launch = adapter
        .launch(ClaudeCodeLaunchRequest::new(
            launch,
            directory.path().join("scaffold"),
            sandbox_directory(directory.path()),
            HarnessPrompt::new("task"),
        ))
        .expect("subscription TUI launch does not validate a provider model");

    assert_subscription_tui_arguments(
        named_launch
            .terminal_launch()
            .launch()
            .command()
            .arguments(),
    );
}

#[test]
fn claude_code_adapter_close_request_renders_exit_terminal_input() {
    let adapter = ClaudeCodeAdapter::new();

    let close_request = adapter.close_request();

    let CloseRequest::TerminalInput(feed) = close_request else {
        panic!("Claude adapter should close by terminal input");
    };
    assert_eq!(feed.bytes(), b"/exit\r");
}

#[test]
fn claude_code_adapter_frames_initial_prompt_and_feed_for_interactive_tui() {
    let directory = tempfile::tempdir().expect("tempdir");
    let adapter = ClaudeCodeAdapter::new();

    let launch = adapter
        .launch(launch_request(directory.path()))
        .expect("adapter launch");
    let initial_input = launch
        .terminal_launch()
        .initial_input()
        .expect("initial input");
    let feed = adapter
        .feed(HarnessFeed::new("continue the sandboxed task"))
        .expect("feed");

    assert!(initial_input.bytes().starts_with(BRACKETED_PASTE_START));
    assert!(initial_input.bytes().ends_with(b"\x1b[201~\r"));
    assert!(
        String::from_utf8_lossy(initial_input.bytes())
            .contains("Initial task:\nshow jj status and wait for the next turn")
    );
    assert_eq!(
        feed.bytes(),
        framed_tui_input("continue the sandboxed task").as_slice()
    );
}

#[test]
fn sandbox_validation_rejects_primary_and_requires_ephemeral_jj_repository() {
    let directory = tempfile::tempdir().expect("tempdir");
    let missing_repository = EphemeralJjRepository::from_existing_ephemeral(directory.path());
    let primary = EphemeralJjRepository::from_existing_ephemeral("/home/li/primary");
    let unmarked = {
        let path = directory.path().join("unmarked");
        fs::create_dir_all(path.join(".jj")).expect("jj directory");
        EphemeralJjRepository::from_existing_ephemeral(path)
    };

    assert!(matches!(
        missing_repository,
        Err(AdapterError::SandboxViolation { reason, .. })
            if reason.contains(".jj repository")
    ));
    assert!(matches!(
        primary,
        Err(AdapterError::SandboxViolation { reason, .. })
            if reason.contains("primary workspace")
    ));
    assert!(matches!(
        unmarked,
        Err(AdapterError::SandboxViolation { reason, .. })
            if reason.contains("ephemeral marker")
    ));
}

#[test]
fn create_in_rejects_primary_and_descendants_before_creating_a_sandbox() {
    assert_create_in_rejects_primary_scope_without_creating_sandbox(Path::new(PRIMARY_WORKSPACE));
    assert_create_in_rejects_primary_scope_without_creating_sandbox(Path::new(
        "/home/li/primary/skills",
    ));
}

#[test]
fn create_in_fails_closed_when_parent_is_missing() {
    let directory = tempfile::tempdir().expect("tempdir");
    let missing_parent = directory.path().join("missing-parent");

    let error = EphemeralJjRepository::create_in(&missing_parent).expect_err("missing parent");

    assert!(matches!(
        error,
        AdapterError::SandboxIo { path, .. } if path == missing_parent
    ));
    assert!(!missing_parent.exists());
}

#[test]
fn adapter_feed_drives_persistent_session_over_multiple_turns() {
    let directory = tempfile::tempdir().expect("tempdir");
    let adapter = ClaudeCodeAdapter::new();
    let launcher = FakeLauncher::new(VecDeque::from([
        b"initial adapter prompt\n".to_vec(),
        b"first adapter turn\n".to_vec(),
        b"second adapter turn\n".to_vec(),
    ]));
    let driver = TerminalCellDriver::new(launcher.clone());
    let mut sessions = NamedHarnessSessions::new(InMemoryHarnessSessionDirectory::new(), driver);
    let launch = adapter
        .launch(launch_request(directory.path()))
        .expect("adapter launch");

    sessions.launch(launch).expect("session launched");
    sessions
        .feed(
            &address(),
            adapter.feed(HarnessFeed::new("first")).expect("feed"),
        )
        .expect("first feed");
    let first = sessions.read(&address()).expect("first read");
    sessions
        .feed(
            &address(),
            adapter.feed(HarnessFeed::new("second")).expect("feed"),
        )
        .expect("second feed");
    let second = sessions.read(&address()).expect("second read");

    assert_eq!(
        launcher.sent(),
        vec![
            framed_tui_input(
                "Mentci sandboxed jj proof session.\nWork only inside the current jj sandbox working copy.\nDo not use /home/li/primary as a jj working copy.\nInitial task:\nshow jj status and wait for the next turn\nPreflight launch:\n(MentciPreflightLaunch (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred) ([(beads skills/beads.md [claim and update the bead]) (rust-discipline skills/rust-discipline.md [implement the adapter])] (cheap-contained-preflight subscription-tui-default) (ClaudeCode claude-code-terminal-adapter terminal-cell-v1) [Prompt requires a sandboxed jj task and a persistent named harness session]) (mentci-primary-edm1 [(Bead primary-edm1) (WorkSurface sandboxed-jj-task) (HarnessLabel mentci-harness)] primary-edm1-session orchestrate/lanes/primary-edm1) Persistent (SandboxedJjTask PrimaryForbidden PrivateScopeClosed) [(IdleTimeout 1) (TurnCap 8) CompletionSignal] [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary)])\n"
            ),
            framed_tui_input("first"),
            framed_tui_input("second"),
        ]
    );
    assert_eq!(first.reason(), &StopReason::IdleTimeout);
    assert_eq!(
        first.transcript().bytes(),
        b"initial adapter prompt\nfirst adapter turn\n"
    );
    assert_eq!(second.reason(), &StopReason::IdleTimeout);
    assert_eq!(
        second.transcript().bytes(),
        b"initial adapter prompt\nfirst adapter turn\nsecond adapter turn\n"
    );
}

#[test]
fn claude_code_adapter_scrapes_transcript_deltas_without_print_json() {
    let adapter = ClaudeCodeAdapter::new();
    let first_transcript =
        TranscriptCapture::from_bytes(b"Claude TUI opened\nMENTCI_PROOF_READY\n".to_vec());

    let first_delta =
        adapter.transcript_delta(ClaudeCodeTranscriptCursor::default(), &first_transcript);
    let second_transcript = TranscriptCapture::from_bytes(
        b"Claude TUI opened\nMENTCI_PROOF_READY\nMENTCI_PROOF_TURN1_DONE\n".to_vec(),
    );
    let second_delta = adapter.transcript_delta(first_delta.next_cursor(), &second_transcript);

    assert!(first_delta.contains_text("MENTCI_PROOF_READY"));
    assert_eq!(
        first_delta.next_cursor().offset(),
        first_transcript.bytes().len()
    );
    assert_eq!(second_delta.bytes(), b"MENTCI_PROOF_TURN1_DONE\n");
    assert!(second_delta.contains_text("MENTCI_PROOF_TURN1_DONE"));
}

#[test]
fn claude_specific_behavior_is_not_in_generic_liveness_or_session_layers() {
    let liveness_source = include_str!("../src/harness_liveness.rs");
    let session_source = include_str!("../src/harness_sessions.rs");

    for source in [liveness_source, session_source] {
        assert!(!source.contains("--permission-mode"));
        assert!(!source.contains("bypassPermissions"));
        assert!(!source.contains("claude-haiku-4-5-20251001"));
        assert!(!source.contains("--bare"));
        assert!(!source.contains("--print"));
        assert!(!source.contains("ANTHROPIC_API_KEY"));
        assert!(!source.contains("apiKeyHelper"));
        assert!(!source.contains("/exit"));
    }
}

#[cfg(feature = "terminal-cell-runtime")]
#[test]
fn terminal_cell_runtime_accepts_adapter_feed_without_invoking_external_claude_code() {
    use mentci::harness_liveness::{
        IdleTimeout, LaunchRequest, LivenessPolicy, StopCondition, StopConditions, TerminalCommand,
        TerminalLaunch, TerminalSize,
    };

    let adapter = ClaudeCodeAdapter::new();
    let driver = TerminalCellDriver::<mentci::harness_liveness::TerminalCellLauncher>::default();
    let request = LaunchRequest::new(TerminalLaunch::new(
        TerminalCommand::new(
            "/bin/sh",
            vec![
                "-c".to_owned(),
                "while IFS= read -r line; do printf 'adapter:%s\\n' \"$line\"; done".to_owned(),
            ],
        ),
        TerminalSize::new(24, 80),
    ))
    .with_liveness(LivenessPolicy::new(StopConditions::new([
        StopCondition::IdleTimeout(IdleTimeout::new(Duration::from_millis(200))),
    ])));
    let mut session = driver.launch(request).expect("terminal-cell launched");

    session
        .send(adapter.feed(HarnessFeed::new("one")).expect("feed"))
        .expect("first send");
    let first = session.read_until_stop().expect("first read");
    session
        .send(adapter.feed(HarnessFeed::new("two")).expect("feed"))
        .expect("second send");
    let second = session.read_until_stop().expect("second read");
    let _closed = session
        .close(CloseRequest::Kill)
        .expect("close local shell process");

    assert_eq!(first.reason(), &StopReason::IdleTimeout);
    assert_eq!(second.reason(), &StopReason::IdleTimeout);
    assert!(
        second
            .transcript()
            .to_string_lossy()
            .contains("adapter:\u{1b}[200~two\u{1b}[201~"),
        "real terminal-cell transcript did not include second adapter feed: {:?}",
        second.transcript().to_string_lossy()
    );
}
