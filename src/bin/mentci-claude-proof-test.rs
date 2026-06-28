#[cfg(not(feature = "terminal-cell-runtime"))]
fn main() {
    println!(
        "MentciClaudeProofSkipped terminal-cell-runtime feature is required for the real Claude proof"
    );
}

#[cfg(feature = "terminal-cell-runtime")]
fn main() {
    if let Err(error) = proof::ProofRunner::from_environment().and_then(|runner| runner.run()) {
        eprintln!("MentciClaudeProofBlocked {error}");
        std::process::exit(2);
    }
}

#[cfg(feature = "terminal-cell-runtime")]
mod proof {
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use mentci::harness_adapters::{
        ClaudeCodeAdapter, ClaudeCodeArtifactObservation, ClaudeCodeLaunchRequest,
        ClaudeCodeModelCommand, EphemeralJjRepository, HarnessFeed, HarnessPrompt,
    };
    use mentci::harness_liveness::{CloseRequest, CloseSignal, TerminalCellDriver};
    use mentci::harness_sessions::{
        InMemoryHarnessSessionDirectory, NamedHarnessSessions, SessionAddress,
    };
    use mentci::preflight::{MentciPreflightLaunch, SessionHandle};
    use thiserror::Error;

    const RUN_GATE: &str = "MENTCI_RUN_REAL_CLAUDE_PROOF";
    const WITNESS_PATH: &str = "MENTCI_REAL_CLAUDE_WITNESS";
    const KEEP_SANDBOX: &str = "MENTCI_KEEP_REAL_CLAUDE_PROOF_SANDBOX";
    const PROOF_FILE: &str = "mentci-proof.txt";
    const PROOF_FILE_CONTENT: &str = "mentci primary-0bax terminal-cell proof\n";
    const COMMIT_MESSAGE: &str = "mentci proof task";
    const READY_PROMPT_MARKER: &str = "MENTCI_PROOF_PROMPT_MARKER_READY";
    const TURN_ONE_PROMPT_MARKER: &str = "MENTCI_PROOF_PROMPT_MARKER_TURN1";
    const TURN_TWO_PROMPT_MARKER: &str = "MENTCI_PROOF_PROMPT_MARKER_TURN2";
    const PRIMARY_WORKSPACE: &str = "/home/li/primary";
    const FORBIDDEN_ARGUMENTS: &[&str] = &[
        "--bare",
        "--print",
        "--permission-mode",
        "bypassPermissions",
    ];

    pub struct ProofRunner {
        enabled: ProofGate,
        witness_path: PathBuf,
        keep_sandbox: bool,
    }

    enum ProofGate {
        Enabled,
        Disabled,
    }

    struct ProofWorkspace {
        base: PathBuf,
        scaffold: PathBuf,
        sandbox: EphemeralJjRepository,
    }

    struct ProofPreflight {
        launch: MentciPreflightLaunch,
    }

    struct CommandResult {
        program: String,
        arguments: Vec<String>,
        status: String,
        stdout: String,
        stderr: String,
    }

    struct ProofWitness {
        witness_path: PathBuf,
        sandbox_path: PathBuf,
        scaffold_path: PathBuf,
        claude_version: CommandResult,
        argv_program: String,
        argv_arguments: Vec<String>,
        model_command: ClaudeCodeModelCommand,
        initial_prompt_summary: String,
        forbidden_arguments_seen: Vec<String>,
        preflight_launch: String,
        artifact_observation_strategy: String,
        artifact_file_event_count: u64,
        artifact_polling_fallback_count: u64,
        artifact_session_identifier: String,
        artifact_model: String,
        artifact_tool_call_count: usize,
        artifact_tool_result_count: usize,
        artifact_file_edit_count: usize,
        artifact_stop_reason_end_turn: bool,
        close_signal: String,
        close_input: String,
        proof_file_content: String,
        jj_status_after_task: CommandResult,
        jj_log_after_task: CommandResult,
        primary_status_before: CommandResult,
        primary_status_after: CommandResult,
        primary_unchanged: bool,
        sandbox_removed: bool,
    }

    impl ProofRunner {
        pub fn from_environment() -> Result<Self, ProofError> {
            let enabled = match env::var(RUN_GATE).ok().as_deref() {
                Some("1") => ProofGate::Enabled,
                _ => ProofGate::Disabled,
            };
            let witness_path = env::var_os(WITNESS_PATH)
                .map(PathBuf::from)
                .unwrap_or_else(|| env::temp_dir().join("mentci-real-claude-proof-witness.md"));
            let keep_sandbox = env::var(KEEP_SANDBOX).ok().as_deref() == Some("1");
            Ok(Self {
                enabled,
                witness_path,
                keep_sandbox,
            })
        }

        pub fn run(self) -> Result<(), ProofError> {
            let adapter = ClaudeCodeAdapter::new();
            let claude_version = CommandRunner::new(adapter.command())
                .argument("--version")
                .run_in(Path::new("/tmp"))?;

            if !claude_version.succeeded() {
                let report = BlockedProofReport::new(
                    self.witness_path,
                    claude_version,
                    "normal claude command is unavailable",
                );
                report.write()?;
                println!(
                    "MentciClaudeProofBlocked witness={} prerequisite=normal-claude-command",
                    report.path().display()
                );
                return Err(ProofError::Blocked(
                    "normal claude command is unavailable".to_owned(),
                ));
            }

            if matches!(self.enabled, ProofGate::Disabled) {
                println!(
                    "MentciClaudeProofSkipped set {RUN_GATE}=1 to run the real Claude Code terminal-cell proof"
                );
                return Ok(());
            }

            let primary_status_before = CommandRunner::new("jj")
                .argument("-R")
                .argument(PRIMARY_WORKSPACE)
                .argument("st")
                .run_in(Path::new("/tmp"))?;
            let workspace = ProofWorkspace::create()?;
            let preflight = ProofPreflight::new();
            let launch_request = ClaudeCodeLaunchRequest::new(
                preflight.launch.clone(),
                workspace.scaffold.clone(),
                workspace.sandbox.clone(),
                HarnessPrompt::new(format!(
                    "{READY_PROMPT_MARKER}: Turn 0 only: reply exactly MENTCI_PROOF_READY and wait for the next Mentci feed. Do not inspect any repository yet."
                )),
            )
            .with_model_command(ClaudeCodeModelCommand::haiku());
            let named_launch = adapter.launch(launch_request)?;
            let terminal_launch = named_launch.terminal_launch();
            let argv_program = terminal_launch.launch().command().program().to_owned();
            let argv_arguments = terminal_launch.launch().command().arguments().to_vec();
            let initial_prompt_summary = terminal_launch
                .initial_input()
                .map(|input| TranscriptSnippet::new(input.bytes()).summary())
                .unwrap_or_else(|| "no initial input".to_owned());
            let forbidden_arguments_seen =
                ForbiddenArguments::from_arguments(&argv_arguments).into_vec();
            if !forbidden_arguments_seen.is_empty() {
                return Err(ProofError::ForbiddenLaunchArguments(
                    forbidden_arguments_seen,
                ));
            }
            let close_request = adapter.close_request();
            let close_input = CloseInputSummary::new(&close_request).summary();
            let address =
                SessionAddress::handle(SessionHandle::new("primary-0bax-claude-proof-session"));
            let artifact_observation = ClaudeCodeArtifactObservation::from_launch(&named_launch)?;
            let driver =
                TerminalCellDriver::<mentci::harness_liveness::TerminalCellLauncher>::default();
            let mut sessions =
                NamedHarnessSessions::new(InMemoryHarnessSessionDirectory::new(), driver);

            sessions.launch(named_launch)?;
            artifact_observation.wait_for_markers(
                READY_PROMPT_MARKER,
                "MENTCI_PROOF_READY",
                Duration::from_secs(90),
            )?;
            sessions.feed(
                &address,
                adapter.feed(HarnessFeed::new(format!(
                    "{TURN_ONE_PROMPT_MARKER}: Turn 1: work only inside the current directory. Run pwd and jj status. Create mentci-proof.txt with exactly `mentci primary-0bax terminal-cell proof`. Reply with MENTCI_PROOF_TURN1_DONE."
                )))?,
            )?;
            artifact_observation.wait_for_markers(
                TURN_ONE_PROMPT_MARKER,
                "MENTCI_PROOF_TURN1_DONE",
                Duration::from_secs(90),
            )?;
            sessions.feed(
                &address,
                adapter.feed(HarnessFeed::new(format!(
                    "{TURN_TWO_PROMPT_MARKER}: Turn 2: run jj status, commit the proof file with exactly `jj commit -m 'mentci proof task'`, then run `jj log -r @- --no-graph -T 'description'`. Do not push. Reply with MENTCI_PROOF_TURN2_DONE."
                )))?,
            )?;
            let artifact_report = artifact_observation.wait_for_markers(
                TURN_TWO_PROMPT_MARKER,
                "MENTCI_PROOF_TURN2_DONE",
                Duration::from_secs(90),
            )?;
            let close_outcome = sessions.close(&address, close_request)?;
            let artifact_snapshot = artifact_report.snapshot();
            let recovered_turn = artifact_snapshot.recovered_turn();

            let proof_file_content =
                fs::read_to_string(workspace.sandbox.working_directory().join(PROOF_FILE))?;
            if proof_file_content != PROOF_FILE_CONTENT {
                return Err(ProofError::UnexpectedProofFile(proof_file_content));
            }
            let jj_status_after_task = CommandRunner::new("jj")
                .argument("st")
                .run_in(workspace.sandbox.working_directory())?;
            let jj_log_after_task = CommandRunner::new("jj")
                .argument("log")
                .argument("-r")
                .argument("@-")
                .argument("--no-graph")
                .argument("-T")
                .argument("description")
                .run_in(workspace.sandbox.working_directory())?;
            if !jj_log_after_task.stdout.contains(COMMIT_MESSAGE) {
                return Err(ProofError::MissingCommitWitness(
                    jj_log_after_task.stdout.clone(),
                ));
            }
            let primary_status_after = CommandRunner::new("jj")
                .argument("-R")
                .argument(PRIMARY_WORKSPACE)
                .argument("st")
                .run_in(Path::new("/tmp"))?;
            let primary_unchanged = primary_status_before.stdout == primary_status_after.stdout
                && primary_status_before.stderr == primary_status_after.stderr;

            let sandbox_path = workspace.sandbox.working_directory().to_path_buf();
            let scaffold_path = workspace.scaffold.clone();
            let sandbox_removed = workspace.cleanup(self.keep_sandbox)?;
            let witness = ProofWitness {
                witness_path: self.witness_path,
                sandbox_path,
                scaffold_path,
                claude_version,
                argv_program,
                argv_arguments,
                model_command: ClaudeCodeModelCommand::haiku(),
                initial_prompt_summary,
                forbidden_arguments_seen,
                preflight_launch: preflight.launch.to_nota(),
                artifact_observation_strategy: format!("{:?}", artifact_report.strategy()),
                artifact_file_event_count: artifact_report.file_event_count(),
                artifact_polling_fallback_count: artifact_report.polling_fallback_count(),
                artifact_session_identifier: artifact_snapshot
                    .session_identifier()
                    .unwrap_or("unknown")
                    .to_owned(),
                artifact_model: recovered_turn.model().unwrap_or("unknown").to_owned(),
                artifact_tool_call_count: recovered_turn.tool_calls().len(),
                artifact_tool_result_count: recovered_turn.tool_results().len(),
                artifact_file_edit_count: recovered_turn.file_edits().len(),
                artifact_stop_reason_end_turn: recovered_turn.has_stop_reason_end_turn(),
                close_signal: format!("{:?}", close_outcome.reason()),
                close_input,
                proof_file_content,
                jj_status_after_task,
                jj_log_after_task,
                primary_status_before,
                primary_status_after,
                primary_unchanged,
                sandbox_removed,
            };
            witness.write()?;
            println!(
                "MentciClaudeProofPassed witness={} sandbox={} primary_unchanged={}",
                witness.witness_path.display(),
                witness.sandbox_path.display(),
                witness.primary_unchanged
            );
            Ok(())
        }
    }

    impl ProofWorkspace {
        fn create() -> Result<Self, ProofError> {
            let base = ProofPath::new("mentci-real-claude-proof").create()?;
            let scaffold = base.join("scaffold");
            fs::create_dir_all(scaffold.join("skills"))?;
            fs::write(
                scaffold.join("skills").join("skills.nota"),
                "[(Workflow beads skills/beads.md Mechanism [claim and update the bead])]\n",
            )?;
            fs::write(
                scaffold.join("README.md"),
                "Mentci real Claude proof scaffold. Use skills/skills.nota as the expansion index.\n",
            )?;
            let sandbox_parent = base.join("sandbox-parent");
            fs::create_dir(&sandbox_parent)?;
            let sandbox = EphemeralJjRepository::create_in(&sandbox_parent)?;
            Ok(Self {
                base,
                scaffold,
                sandbox,
            })
        }

        fn cleanup(self, keep_sandbox: bool) -> Result<bool, ProofError> {
            if keep_sandbox {
                Ok(false)
            } else {
                fs::remove_dir_all(&self.base)?;
                Ok(true)
            }
        }
    }

    impl ProofPreflight {
        fn new() -> Self {
            let launch = MentciPreflightLaunch::validated_from_nota(Self::launch_nota())
                .expect("proof launch NOTA stays schema-valid");
            Self { launch }
        }

        fn launch_nota() -> &'static str {
            r#"(MentciPreflightLaunch
  (mentci-primary-0bax-proof 1 [skills/skills.nota] [README.md] skills/skills.nota ReuseDeferred)
  (mentci-primary-0bax [(Bead primary-0bax) (WorkSurface sandboxed-jj-task) (HarnessLabel real-claude-terminal-cell)] primary-0bax-claude-proof-session orchestrate/lanes/primary-0bax)
  Persistent
  (SandboxedJjTask PrimaryForbidden PrivateScopeClosed)
  [(IdleTimeout 45) (TurnCap 8) CompletionSignal]
  [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary) (RequiredWitness real-claude-terminal-cell) (RequiredWitness claude-artifact-observation) (ImplementationBoundary claude-adapter-only)])"#
        }
    }

    impl ProofWitness {
        fn write(&self) -> Result<(), ProofError> {
            if let Some(parent) = self.witness_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&self.witness_path, self.render())?;
            Ok(())
        }

        fn render(&self) -> String {
            format!(
                "# Mentci real Claude proof witness\n\nstatus: passed\nsandbox_path: {}\nscaffold_path: {}\nsandbox_removed: {}\nprimary_unchanged: {}\n\n## Claude\nversion_status: {}\nversion_stdout: {}\nsubscription_tui: normal interactive claude\nmodel_argument: {}\n\n## Launch\nargv_program: {}\nargv_arguments: {:?}\nforbidden_arguments_seen: {:?}\nclose_input: {}\ninitial_prompt_summary: {}\n\n## Preflight\n{}\n\n## Artifact Observation\nstrategy: {}\nfile_event_count: {}\npolling_fallback_count: {}\nsession_identifier: {}\nmodel: {}\ntool_call_count: {}\ntool_result_count: {}\nfile_edit_count: {}\nstop_reason_end_turn: {}\n\nclose_signal: {}\n\n## Jj Task\nproof_file_content: {:?}\njj_status_after_task_status: {}\njj_status_after_task_stdout: {}\njj_log_after_task_status: {}\njj_log_after_task_stdout: {}\n\n## Primary Guard\nprimary_before_status: {}\nprimary_before_stdout: {}\nprimary_after_status: {}\nprimary_after_stdout: {}\n",
                self.sandbox_path.display(),
                self.scaffold_path.display(),
                self.sandbox_removed,
                self.primary_unchanged,
                self.claude_version.status,
                self.claude_version.stdout.trim(),
                self.model_command.as_str(),
                self.argv_program,
                self.argv_arguments,
                self.forbidden_arguments_seen,
                self.close_input,
                self.initial_prompt_summary,
                self.preflight_launch,
                self.artifact_observation_strategy,
                self.artifact_file_event_count,
                self.artifact_polling_fallback_count,
                self.artifact_session_identifier,
                self.artifact_model,
                self.artifact_tool_call_count,
                self.artifact_tool_result_count,
                self.artifact_file_edit_count,
                self.artifact_stop_reason_end_turn,
                self.close_signal,
                self.proof_file_content,
                self.jj_status_after_task.status,
                self.jj_status_after_task.stdout.trim(),
                self.jj_log_after_task.status,
                self.jj_log_after_task.stdout.trim(),
                self.primary_status_before.status,
                self.primary_status_before.stdout.trim(),
                self.primary_status_after.status,
                self.primary_status_after.stdout.trim(),
            )
        }
    }

    struct BlockedProofReport {
        path: PathBuf,
        claude_version: CommandResult,
        prerequisite: String,
    }

    impl BlockedProofReport {
        fn new(
            path: PathBuf,
            claude_version: CommandResult,
            prerequisite: impl Into<String>,
        ) -> Self {
            Self {
                path,
                claude_version,
                prerequisite: prerequisite.into(),
            }
        }

        fn path(&self) -> &Path {
            self.path.as_path()
        }

        fn write(&self) -> Result<(), ProofError> {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&self.path, self.render())?;
            Ok(())
        }

        fn render(&self) -> String {
            format!(
                "# Mentci real Claude proof witness\n\nstatus: blocked\nmissing_prerequisite: {}\n\n## Detection\nclaude_version_program: {}\nclaude_version_arguments: {:?}\nclaude_version_status: {}\nclaude_version_stdout: {}\nclaude_version_stderr: {}\n\n## Required Proof Not Run\nThe real terminal-cell proof did not run because the normal interactive `claude` command was unavailable. This proof must use the user's configured Claude subscription TUI in a terminal cell. Mentci must not add `--bare`, `--print`, Anthropic API-key plumbing, or `apiKeyHelper` assumptions.\n",
                self.prerequisite,
                self.claude_version.program,
                self.claude_version.arguments,
                self.claude_version.status,
                self.claude_version.stdout.trim(),
                self.claude_version.stderr.trim(),
            )
        }
    }

    struct ForbiddenArguments {
        arguments: Vec<String>,
    }

    impl ForbiddenArguments {
        fn from_arguments(arguments: &[String]) -> Self {
            let mut seen = arguments
                .iter()
                .filter(|argument| FORBIDDEN_ARGUMENTS.contains(&argument.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if arguments
                .windows(2)
                .any(|window| window == ["--permission-mode", "bypassPermissions"])
            {
                seen.push("--permission-mode bypassPermissions".to_owned());
            }
            Self { arguments: seen }
        }

        fn into_vec(self) -> Vec<String> {
            self.arguments
        }
    }

    struct CommandRunner {
        program: String,
        arguments: Vec<OsString>,
    }

    impl CommandRunner {
        fn new(program: impl Into<String>) -> Self {
            Self {
                program: program.into(),
                arguments: Vec::new(),
            }
        }

        fn argument(mut self, argument: impl Into<OsString>) -> Self {
            self.arguments.push(argument.into());
            self
        }

        fn run_in(self, directory: &Path) -> Result<CommandResult, ProofError> {
            let output = Command::new(&self.program)
                .args(&self.arguments)
                .current_dir(directory)
                .output()?;
            Ok(CommandResult::from_output(
                self.program,
                self.arguments,
                output,
            ))
        }
    }

    impl CommandResult {
        fn from_output(program: String, arguments: Vec<OsString>, output: Output) -> Self {
            Self {
                program,
                arguments: arguments
                    .into_iter()
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect(),
                status: output.status.to_string(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }
        }

        fn succeeded(&self) -> bool {
            self.status == "exit status: 0"
        }
    }

    struct ProofPath {
        prefix: String,
    }

    impl ProofPath {
        fn new(prefix: impl Into<String>) -> Self {
            Self {
                prefix: prefix.into(),
            }
        }

        fn create(&self) -> Result<PathBuf, ProofError> {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let path =
                env::temp_dir().join(format!("{}-{}-{nanos}", self.prefix, std::process::id()));
            fs::create_dir(&path)?;
            Ok(path)
        }
    }

    struct TranscriptSnippet {
        text: String,
    }

    impl TranscriptSnippet {
        fn new(bytes: &[u8]) -> Self {
            Self {
                text: String::from_utf8_lossy(bytes)
                    .replace('\x1b', "<ESC>")
                    .replace('\r', "\n"),
            }
        }

        fn summary(&self) -> String {
            self.text.chars().take(600).collect()
        }
    }

    struct CloseInputSummary<'a> {
        request: &'a CloseRequest,
    }

    impl<'a> CloseInputSummary<'a> {
        fn new(request: &'a CloseRequest) -> Self {
            Self { request }
        }

        fn summary(&self) -> String {
            match self.request {
                CloseRequest::TerminalInput(feed) => {
                    format!("TerminalInput {:?}", String::from_utf8_lossy(feed.bytes()))
                }
                CloseRequest::DriverDefault => format!("{:?}", CloseSignal::DriverDefault),
                CloseRequest::Interrupt => format!("{:?}", CloseSignal::Interrupt),
                CloseRequest::Terminate => format!("{:?}", CloseSignal::Terminate),
                CloseRequest::Kill => format!("{:?}", CloseSignal::Kill),
            }
        }
    }

    impl From<mentci::harness_adapters::AdapterError> for ProofError {
        fn from(error: mentci::harness_adapters::AdapterError) -> Self {
            Self::Adapter(error.to_string())
        }
    }

    impl From<mentci::harness_sessions::SessionRoutingError> for ProofError {
        fn from(error: mentci::harness_sessions::SessionRoutingError) -> Self {
            Self::Session(error.to_string())
        }
    }

    #[derive(Debug, Error)]
    pub enum ProofError {
        #[error("io: {0}")]
        Io(#[from] std::io::Error),

        #[error("adapter: {0}")]
        Adapter(String),

        #[error("session: {0}")]
        Session(String),

        #[error("blocked: {0}")]
        Blocked(String),

        #[error("unexpected proof file content: {0:?}")]
        UnexpectedProofFile(String),

        #[error("jj commit witness missing expected message; log output was {0:?}")]
        MissingCommitWitness(String),

        #[error("forbidden Claude subscription TUI launch arguments: {0:?}")]
        ForbiddenLaunchArguments(Vec<String>),
    }
}
