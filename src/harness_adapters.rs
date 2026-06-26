use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration as StandardDuration, SystemTime, UNIX_EPOCH};

use signal_harness as harness_contract;
use thiserror::Error;

use crate::harness_liveness::{
    CloseRequest, IdleTimeout, LaunchRequest, LivenessPolicy,
    SandboxPrivacy as DriverSandboxPrivacy, SandboxPrivacyFlag,
    StopCondition as DriverStopCondition, StopConditions, TerminalCommand, TerminalFeed,
    TerminalLaunch, TerminalSize, TerminalWorkingDirectory, TranscriptCapture, TurnCap, TurnCount,
};
use crate::harness_sessions::{HarnessKind, HarnessLaunchMetadata, NamedHarnessLaunch};
use crate::preflight::{
    AdapterIdentity, HarnessSessionModelProfile, MentciPreflightLaunch, PrivacySurface,
    SandboxPrivacy as PreflightSandboxPrivacy, StopCondition, TerminalCellDriverIdentity,
};

const PRIMARY_WORKSPACE: &str = "/home/li/primary";
const EPHEMERAL_SANDBOX_MARKER: &str = ".mentci-ephemeral-jj-sandbox";
const CLAUDE_CODE_ADAPTER: &str = "claude-code-terminal-adapter";
const TERMINAL_CELL_DRIVER: &str = "terminal-cell-v1";
const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";
const CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE_ENV: &str = "MENTCI_CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE";
const CLAUDE_LAUNCHER_INSPECTION_LIMIT: u64 = 64 * 1024;
const FORBIDDEN_CLAUDE_LAUNCHER_FRAGMENTS: &[&str] = &[
    "--bare",
    "--print",
    "--model",
    "--permission-mode",
    "bypassPermissions",
    "ANTHROPIC_API_KEY",
    "apiKeyHelper",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeAdapter {
    executable: ClaudeCodeSubscriptionTuiExecutable,
    terminal_size: TerminalSize,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self {
            executable: ClaudeCodeSubscriptionTuiExecutable::discover(),
            terminal_size: TerminalSize::new(36, 120),
        }
    }

    pub fn with_executable(mut self, executable: impl Into<PathBuf>) -> Self {
        self.executable = ClaudeCodeSubscriptionTuiExecutable::explicit(executable);
        self
    }

    pub fn resolved_executable(&self) -> Result<PathBuf, AdapterError> {
        self.executable.resolve()
    }

    pub fn launch(
        &self,
        request: ClaudeCodeLaunchRequest,
    ) -> Result<NamedHarnessLaunch, AdapterError> {
        self.validate_launch(&request)?;
        let executable = self.resolved_executable()?;
        let terminal_launch = TerminalLaunch::new(
            TerminalCommand::new(
                executable.to_string_lossy().into_owned(),
                self.arguments(&request),
            ),
            self.terminal_size,
        )
        .with_working_directory(TerminalWorkingDirectory::new(
            request.sandbox().working_directory().to_path_buf(),
        ));
        let launch_request = LaunchRequest::new(terminal_launch)
            .with_initial_input(self.initial_input(&request))
            .with_liveness(LivenessPolicy::new(
                self.stop_conditions(request.preflight_launch().stop_conditions()),
            ))
            .with_sandbox_privacy(self.driver_sandbox_privacy(request.preflight_launch()));
        Ok(NamedHarnessLaunch::new(
            request.into_preflight_launch(),
            launch_request,
            self.launch_metadata(),
        ))
    }

    pub fn feed(&self, input: HarnessFeed) -> Result<TerminalFeed, AdapterError> {
        Ok(input.into_terminal_feed())
    }

    pub fn close_request(&self) -> CloseRequest {
        CloseRequest::TerminalInput(TerminalFeed::new(b"/exit\r".to_vec()))
    }

    pub fn event_mapper(&self, harness: impl Into<String>) -> ClaudeCodeEventMapper {
        ClaudeCodeEventMapper::new(harness)
    }

    pub fn transcript_delta(
        &self,
        cursor: ClaudeCodeTranscriptCursor,
        transcript: &TranscriptCapture,
    ) -> ClaudeCodeTranscriptDelta {
        let bytes = transcript.bytes();
        let offset = cursor.offset.min(bytes.len());
        ClaudeCodeTranscriptDelta::new(
            bytes[offset..].to_vec(),
            ClaudeCodeTranscriptCursor::new(bytes.len()),
        )
    }

    fn validate_launch(&self, request: &ClaudeCodeLaunchRequest) -> Result<(), AdapterError> {
        request.sandbox().validate()?;
        Ok(())
    }

    fn launch_metadata(&self) -> HarnessLaunchMetadata {
        HarnessLaunchMetadata::new(
            HarnessKind::claude_code(),
            AdapterIdentity::new(CLAUDE_CODE_ADAPTER),
            TerminalCellDriverIdentity::new(TERMINAL_CELL_DRIVER),
            HarnessSessionModelProfile::new("subscription-tui-default"),
        )
    }

    fn arguments(&self, request: &ClaudeCodeLaunchRequest) -> Vec<String> {
        vec![
            "--add-dir".to_owned(),
            request.scaffold_path().to_string_lossy().into_owned(),
            "--name".to_owned(),
            request
                .preflight_launch()
                .session_identity()
                .lane_name()
                .as_str()
                .to_owned(),
        ]
    }

    fn initial_input(&self, request: &ClaudeCodeLaunchRequest) -> TerminalFeed {
        let mut prompt = String::new();
        prompt.push_str("Mentci sandboxed jj proof session.\n");
        prompt.push_str("Work only inside the current jj sandbox working copy.\n");
        prompt.push_str("Do not use /home/li/primary as a jj working copy.\n");
        prompt.push_str("Initial task:\n");
        prompt.push_str(request.prompt().as_str());
        prompt.push_str("\nPreflight launch:\n");
        prompt.push_str(&request.preflight_launch().to_nota());
        prompt.push_str("\n");
        InteractiveTerminalInput::new(prompt).into_terminal_feed()
    }

    fn stop_conditions(&self, stop_conditions: &[StopCondition]) -> StopConditions {
        StopConditions::new(
            stop_conditions
                .iter()
                .map(DriverStopCondition::from)
                .collect::<Vec<_>>(),
        )
    }

    fn driver_sandbox_privacy(&self, launch: &MentciPreflightLaunch) -> DriverSandboxPrivacy {
        let mut flags = vec![
            SandboxPrivacyFlag::SandboxedJjTask,
            SandboxPrivacyFlag::PrimaryJjForbidden,
            SandboxPrivacyFlag::AdapterBoundary(CLAUDE_CODE_ADAPTER.to_owned()),
        ];
        if matches!(
            launch.sandbox_privacy(),
            PreflightSandboxPrivacy::SandboxedJjTask(_, PrivacySurface::PrivateScopeClosed)
        ) {
            flags.push(SandboxPrivacyFlag::PrivateScopeClosed);
        }
        DriverSandboxPrivacy::new(flags)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeSubscriptionTuiExecutable {
    selection: ClaudeCodeExecutableSelection,
}

impl ClaudeCodeSubscriptionTuiExecutable {
    pub fn discover() -> Self {
        Self {
            selection: ClaudeCodeExecutableSelection::Discover,
        }
    }

    pub fn explicit(path: impl Into<PathBuf>) -> Self {
        Self {
            selection: ClaudeCodeExecutableSelection::Explicit(path.into()),
        }
    }

    pub fn resolve(&self) -> Result<PathBuf, AdapterError> {
        match &self.selection {
            ClaudeCodeExecutableSelection::Explicit(path) => Self::validate_selected(path),
            ClaudeCodeExecutableSelection::Discover => Self::discover_selected(),
        }
    }

    fn discover_selected() -> Result<PathBuf, AdapterError> {
        if let Some(path) = env::var_os(CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE_ENV) {
            return Self::validate_selected(Path::new(&path));
        }

        let profile_claude = Self::find_on_path("claude")?;
        let wrapper = Self::inspect_text(&profile_claude)?;
        let Some(direct_executable) = Self::direct_subscription_tui_target(&wrapper) else {
            return Err(AdapterError::ClaudeLauncherUnavailable {
                reason: format!(
                    "PATH claude resolved to {:?}, but Mentci could not discover a direct Claude subscription TUI executable; set {CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE_ENV}",
                    profile_claude
                ),
            });
        };
        Self::validate_selected(&direct_executable)
    }

    fn validate_selected(path: &Path) -> Result<PathBuf, AdapterError> {
        let executable =
            fs::canonicalize(path).map_err(|source| AdapterError::ClaudeLauncherIo {
                path: path.to_path_buf(),
                message: source.to_string(),
            })?;
        let metadata =
            fs::metadata(&executable).map_err(|source| AdapterError::ClaudeLauncherIo {
                path: executable.clone(),
                message: source.to_string(),
            })?;
        if !metadata.is_file() {
            return Err(AdapterError::ClaudeLauncherUnavailable {
                reason: format!("Claude subscription TUI executable is not a file: {executable:?}"),
            });
        }

        let text = Self::inspect_text(&executable)?;
        if let Some(fragment) = FORBIDDEN_CLAUDE_LAUNCHER_FRAGMENTS
            .iter()
            .find(|fragment| text.contains(**fragment))
        {
            return Err(AdapterError::ForbiddenClaudeLauncher {
                path: executable,
                reason: format!(
                    "selected Claude launcher contains forbidden subscription-proof fragment {fragment:?}"
                ),
            });
        }
        Ok(executable)
    }

    fn find_on_path(name: &str) -> Result<PathBuf, AdapterError> {
        let Some(paths) = env::var_os("PATH") else {
            return Err(AdapterError::ClaudeLauncherUnavailable {
                reason: "PATH is unset; set MENTCI_CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE".to_owned(),
            });
        };
        for directory in env::split_paths(&paths) {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        Err(AdapterError::ClaudeLauncherUnavailable {
            reason: "claude was not found on PATH; set MENTCI_CLAUDE_SUBSCRIPTION_TUI_EXECUTABLE"
                .to_owned(),
        })
    }

    fn inspect_text(path: &Path) -> Result<String, AdapterError> {
        let file = fs::File::open(path).map_err(|source| AdapterError::ClaudeLauncherIo {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
        let mut bytes = Vec::new();
        file.take(CLAUDE_LAUNCHER_INSPECTION_LIMIT)
            .read_to_end(&mut bytes)
            .map_err(|source| AdapterError::ClaudeLauncherIo {
                path: path.to_path_buf(),
                message: source.to_string(),
            })?;
        String::from_utf8(bytes).map_err(|source| AdapterError::ForbiddenClaudeLauncher {
            path: path.to_path_buf(),
            reason: format!("selected Claude launcher is not inspectable UTF-8 text: {source}"),
        })
    }

    fn direct_subscription_tui_target(wrapper: &str) -> Option<PathBuf> {
        wrapper
            .lines()
            .filter(|line| line.trim_start().starts_with("exec "))
            .flat_map(|line| line.split_whitespace())
            .map(|token| token.trim_matches('"').trim_matches('\''))
            .find(|token| {
                token.starts_with('/')
                    && token.ends_with("/bin/claude")
                    && token.contains("claude-code")
            })
            .map(PathBuf::from)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClaudeCodeExecutableSelection {
    Discover,
    Explicit(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeEventMapper {
    harness: harness_contract::HarnessName,
    next_sequence: u64,
}

impl ClaudeCodeEventMapper {
    pub fn new(harness: impl Into<String>) -> Self {
        Self {
            harness: harness_contract::HarnessName::new(harness),
            next_sequence: 1,
        }
    }

    pub fn input_accepted(
        &mut self,
        message_slot: harness_contract::MessageSlot,
    ) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterInputAccepted(
            harness_contract::AdapterInputAccepted {
                harness: self.harness.clone(),
                sequence: self.sequence(),
                message_slot,
            },
        )
    }

    pub fn transcript_delta(
        &mut self,
        delta: &ClaudeCodeTranscriptDelta,
        message_slot: harness_contract::MessageSlot,
    ) -> Vec<harness_contract::HarnessEvent> {
        ClaudeCodeTranscriptText::from_delta(delta).events(self, message_slot)
    }

    pub fn read_outcome(
        &mut self,
        outcome: &crate::harness_liveness::ReadOutcome,
        message_slot: harness_contract::MessageSlot,
    ) -> Vec<harness_contract::HarnessEvent> {
        self.stop_reason(outcome.reason(), outcome.transcript(), message_slot)
    }

    pub fn stop_reason(
        &mut self,
        reason: &crate::harness_liveness::StopReason,
        transcript: &TranscriptCapture,
        message_slot: harness_contract::MessageSlot,
    ) -> Vec<harness_contract::HarnessEvent> {
        match reason {
            crate::harness_liveness::StopReason::CompletionSignal => {
                vec![self.completion(message_slot)]
            }
            crate::harness_liveness::StopReason::StalledOutput => {
                vec![self.stalled(harness_contract::AdapterStallReason::CompletionTimeout)]
            }
            crate::harness_liveness::StopReason::IdleTimeout if transcript.bytes().is_empty() => {
                vec![self.stalled(harness_contract::AdapterStallReason::NoOutput)]
            }
            crate::harness_liveness::StopReason::TerminalExit(exit) => {
                vec![self.exited(ClaudeCodeExitStatus::from_report(exit).into_contract())]
            }
            crate::harness_liveness::StopReason::TurnCap(_)
            | crate::harness_liveness::StopReason::IdleTimeout
            | crate::harness_liveness::StopReason::Closed(_) => Vec::new(),
        }
    }

    fn ready(&mut self) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterReady(harness_contract::AdapterReady {
            harness: self.harness.clone(),
            sequence: self.sequence(),
        })
    }

    fn output(&mut self, text: String) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterOutput(harness_contract::AdapterOutput {
            harness: self.harness.clone(),
            sequence: self.sequence(),
            text,
        })
    }

    fn progress(&mut self, status: impl Into<String>) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterProgress(harness_contract::AdapterProgress {
            harness: self.harness.clone(),
            sequence: self.sequence(),
            status: status.into(),
        })
    }

    fn completion(
        &mut self,
        message_slot: harness_contract::MessageSlot,
    ) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterCompletion(harness_contract::AdapterCompletion {
            harness: self.harness.clone(),
            sequence: self.sequence(),
            message_slot,
        })
    }

    fn confirmation_needed(
        &mut self,
        confirmation: ClaudeCodeConfirmation,
    ) -> harness_contract::HarnessEvent {
        let sequence = self.sequence();
        harness_contract::HarnessEvent::AdapterConfirmationNeeded(
            harness_contract::AdapterConfirmationNeeded {
                harness: self.harness.clone(),
                interaction_id: format!("claude-confirmation-{}", sequence.into_u64()),
                sequence,
                prompt: confirmation.prompt,
                options: confirmation.options,
            },
        )
    }

    fn stalled(
        &mut self,
        reason: harness_contract::AdapterStallReason,
    ) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterStalled(harness_contract::AdapterStalled {
            harness: self.harness.clone(),
            sequence: self.sequence(),
            reason,
        })
    }

    fn exited(
        &mut self,
        status: harness_contract::AdapterExitStatus,
    ) -> harness_contract::HarnessEvent {
        harness_contract::HarnessEvent::AdapterExited(harness_contract::AdapterExited {
            harness: self.harness.clone(),
            sequence: self.sequence(),
            status,
        })
    }

    fn sequence(&mut self) -> harness_contract::AdapterEventSequence {
        let sequence = harness_contract::AdapterEventSequence::new(self.next_sequence);
        self.next_sequence = self.next_sequence.saturating_add(1);
        sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaudeCodeTranscriptText {
    text: String,
    lower_case: String,
}

impl ClaudeCodeTranscriptText {
    fn from_delta(delta: &ClaudeCodeTranscriptDelta) -> Self {
        let text = delta.to_string_lossy();
        let lower_case = text.to_lowercase();
        Self { text, lower_case }
    }

    fn events(
        &self,
        mapper: &mut ClaudeCodeEventMapper,
        message_slot: harness_contract::MessageSlot,
    ) -> Vec<harness_contract::HarnessEvent> {
        let mut events = Vec::new();
        if !self.text.is_empty() {
            events.push(mapper.output(self.text.clone()));
        }
        if self.is_ready() {
            events.push(mapper.ready());
        }
        if self.is_progress() {
            events.push(mapper.progress("working"));
        }
        if let Some(confirmation) = self.confirmation() {
            events.push(mapper.confirmation_needed(confirmation));
        }
        if self.is_completion() {
            events.push(mapper.completion(message_slot));
        }
        events
    }

    fn is_ready(&self) -> bool {
        self.lower_case.contains("claude code")
            && (self.lower_case.contains("welcome")
                || self.lower_case.contains("cwd:")
                || self.lower_case.contains("? for shortcuts"))
    }

    fn is_progress(&self) -> bool {
        self.lower_case.contains("thinking")
            || self.lower_case.contains("esc to interrupt")
            || self.lower_case.contains("working")
    }

    fn is_completion(&self) -> bool {
        self.lower_case.contains("mentci_proof_turn") && self.lower_case.contains("done")
    }

    fn confirmation(&self) -> Option<ClaudeCodeConfirmation> {
        if !self.looks_like_confirmation() {
            return None;
        }
        Some(ClaudeCodeConfirmation {
            prompt: self.prompt_line(),
            options: self.confirmation_options(),
        })
    }

    fn looks_like_confirmation(&self) -> bool {
        self.lower_case.contains("permission")
            || self.lower_case.contains("do you want to allow")
            || self.lower_case.contains("allow this command")
            || self.lower_case.contains("approve")
                && (self.lower_case.contains("deny") || self.lower_case.contains("reject"))
    }

    fn prompt_line(&self) -> String {
        self.text
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| ClaudeCodeTranscriptText::line_looks_like_confirmation(line))
            .or_else(|| {
                self.text
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
            })
            .unwrap_or(self.text.trim())
            .to_owned()
    }

    fn confirmation_options(&self) -> Vec<String> {
        if self.lower_case.contains("yes") && self.lower_case.contains("no") {
            return vec!["yes".to_owned(), "no".to_owned()];
        }
        if self.lower_case.contains("allow") && self.lower_case.contains("deny") {
            return vec!["allow".to_owned(), "deny".to_owned()];
        }
        vec!["approve".to_owned(), "decline".to_owned()]
    }

    fn line_looks_like_confirmation(line: &str) -> bool {
        let lower_case = line.to_lowercase();
        lower_case.contains("permission")
            || lower_case.contains("allow")
            || lower_case.contains("approve")
            || lower_case.contains("deny")
            || lower_case.contains("yes / no")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaudeCodeConfirmation {
    prompt: String,
    options: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaudeCodeExitStatus {
    Success,
    Failure,
}

impl ClaudeCodeExitStatus {
    fn from_report(exit: &crate::harness_liveness::TerminalExitReport) -> Self {
        let lower_case = exit.status().to_lowercase();
        if lower_case == "0" || lower_case.contains("success") || lower_case.contains("status: 0") {
            Self::Success
        } else {
            Self::Failure
        }
    }

    fn into_contract(self) -> harness_contract::AdapterExitStatus {
        match self {
            Self::Success => harness_contract::AdapterExitStatus::Success,
            Self::Failure => harness_contract::AdapterExitStatus::Failure,
        }
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeLaunchRequest {
    preflight_launch: MentciPreflightLaunch,
    scaffold_path: PathBuf,
    sandbox: EphemeralJjRepository,
    prompt: HarnessPrompt,
}

impl ClaudeCodeLaunchRequest {
    pub fn new(
        preflight_launch: MentciPreflightLaunch,
        scaffold_path: impl Into<PathBuf>,
        sandbox: EphemeralJjRepository,
        prompt: HarnessPrompt,
    ) -> Self {
        Self {
            preflight_launch,
            scaffold_path: scaffold_path.into(),
            sandbox,
            prompt,
        }
    }

    pub fn preflight_launch(&self) -> &MentciPreflightLaunch {
        &self.preflight_launch
    }

    pub fn scaffold_path(&self) -> &Path {
        self.scaffold_path.as_path()
    }

    pub fn sandbox(&self) -> &EphemeralJjRepository {
        &self.sandbox
    }

    pub fn prompt(&self) -> &HarnessPrompt {
        &self.prompt
    }

    fn into_preflight_launch(self) -> MentciPreflightLaunch {
        self.preflight_launch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessPrompt {
    text: String,
}

impl HarnessPrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessFeed {
    text: String,
}

impl HarnessFeed {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    fn into_terminal_feed(self) -> TerminalFeed {
        InteractiveTerminalInput::new(self.text).into_terminal_feed()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InteractiveTerminalInput {
    text: String,
}

impl InteractiveTerminalInput {
    fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    fn into_terminal_feed(self) -> TerminalFeed {
        let mut bytes = Vec::with_capacity(
            BRACKETED_PASTE_START.len() + self.text.len() + BRACKETED_PASTE_END.len() + b"\r".len(),
        );
        bytes.extend_from_slice(BRACKETED_PASTE_START);
        bytes.extend_from_slice(self.text.as_bytes());
        bytes.extend_from_slice(BRACKETED_PASTE_END);
        bytes.push(b'\r');
        TerminalFeed::new(bytes)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClaudeCodeTranscriptCursor {
    offset: usize,
}

impl ClaudeCodeTranscriptCursor {
    pub const fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeTranscriptDelta {
    bytes: Vec<u8>,
    next_cursor: ClaudeCodeTranscriptCursor,
}

impl ClaudeCodeTranscriptDelta {
    fn new(bytes: Vec<u8>, next_cursor: ClaudeCodeTranscriptCursor) -> Self {
        Self { bytes, next_cursor }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn next_cursor(&self) -> ClaudeCodeTranscriptCursor {
        self.next_cursor
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub fn contains_text(&self, text: &str) -> bool {
        self.to_string_lossy().contains(text)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EphemeralJjRepository {
    working_directory: PathBuf,
}

impl EphemeralJjRepository {
    pub fn create_in(parent_directory: impl AsRef<Path>) -> Result<Self, AdapterError> {
        let parent = parent_directory.as_ref();
        let canonical_parent = Self::canonical_path(parent)?;
        let canonical_primary = Self::canonical_primary_workspace()?;
        Self::reject_primary_workspace(&canonical_parent, &canonical_primary)?;

        let working_directory = EphemeralRepositoryName::new().path_under(&canonical_parent);
        let created_directory = CreatedEphemeralDirectory::create(working_directory)?;
        let canonical_working_directory = created_directory.canonical_path()?;
        Self::reject_primary_workspace(&canonical_working_directory, &canonical_primary)?;

        created_directory.initialize_jj()?;
        created_directory.write_marker()?;
        created_directory.into_repository()
    }

    pub fn from_existing_ephemeral(path: impl Into<PathBuf>) -> Result<Self, AdapterError> {
        let repository = Self {
            working_directory: path.into(),
        };
        repository.validate()?;
        Ok(repository)
    }

    pub fn working_directory(&self) -> &Path {
        self.working_directory.as_path()
    }

    fn validate(&self) -> Result<(), AdapterError> {
        let canonical = self.canonical_working_directory()?;
        let canonical_primary = Self::canonical_primary_workspace()?;
        Self::reject_primary_workspace(&canonical, &canonical_primary)?;
        if !canonical.join(".jj").is_dir() {
            return Err(AdapterError::SandboxViolation {
                path: canonical,
                reason: "jj sandbox must contain a .jj repository".to_owned(),
            });
        }
        if !canonical.join(EPHEMERAL_SANDBOX_MARKER).is_file() {
            return Err(AdapterError::SandboxViolation {
                path: canonical,
                reason: "jj sandbox must carry the Mentci ephemeral marker".to_owned(),
            });
        }
        Ok(())
    }

    fn canonical_working_directory(&self) -> Result<PathBuf, AdapterError> {
        Self::canonical_path(&self.working_directory)
    }

    fn canonical_primary_workspace() -> Result<PathBuf, AdapterError> {
        Self::canonical_path(Path::new(PRIMARY_WORKSPACE))
    }

    fn canonical_path(path: &Path) -> Result<PathBuf, AdapterError> {
        path.canonicalize()
            .map_err(|source| AdapterError::SandboxIo {
                path: path.to_path_buf(),
                message: source.to_string(),
            })
    }

    fn reject_primary_workspace(path: &Path, primary: &Path) -> Result<(), AdapterError> {
        if path == primary || path.starts_with(primary) {
            return Err(AdapterError::SandboxViolation {
                path: path.to_path_buf(),
                reason: "primary workspace cannot be a jj proof working copy".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
struct CreatedEphemeralDirectory {
    path: PathBuf,
    preserved: bool,
}

impl CreatedEphemeralDirectory {
    fn create(path: PathBuf) -> Result<Self, AdapterError> {
        std::fs::create_dir(&path).map_err(|source| AdapterError::SandboxIo {
            path: path.clone(),
            message: source.to_string(),
        })?;
        Ok(Self {
            path,
            preserved: false,
        })
    }

    fn canonical_path(&self) -> Result<PathBuf, AdapterError> {
        EphemeralJjRepository::canonical_path(&self.path)
    }

    fn initialize_jj(&self) -> Result<(), AdapterError> {
        let status = Command::new("jj")
            .arg("git")
            .arg("init")
            .arg("--colocate")
            .current_dir(&self.path)
            .status()
            .map_err(|source| AdapterError::SandboxIo {
                path: self.path.clone(),
                message: source.to_string(),
            })?;
        if !status.success() {
            return Err(AdapterError::SandboxInitialization {
                path: self.path.clone(),
                status: status.to_string(),
            });
        }
        Ok(())
    }

    fn write_marker(&self) -> Result<(), AdapterError> {
        let marker = self.path.join(EPHEMERAL_SANDBOX_MARKER);
        std::fs::write(&marker, "mentci ephemeral jj sandbox\n").map_err(|source| {
            AdapterError::SandboxIo {
                path: marker,
                message: source.to_string(),
            }
        })
    }

    fn into_repository(mut self) -> Result<EphemeralJjRepository, AdapterError> {
        let repository = EphemeralJjRepository::from_existing_ephemeral(self.path.clone())?;
        self.preserved = true;
        Ok(repository)
    }
}

impl Drop for CreatedEphemeralDirectory {
    fn drop(&mut self) {
        if !self.preserved {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EphemeralRepositoryName {
    name: String,
}

impl EphemeralRepositoryName {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Self {
            name: format!("mentci-jj-proof-{}-{nanos}", std::process::id()),
        }
    }

    fn path_under(&self, parent_directory: impl AsRef<Path>) -> PathBuf {
        parent_directory.as_ref().join(&self.name)
    }
}

impl From<&StopCondition> for DriverStopCondition {
    fn from(condition: &StopCondition) -> Self {
        match condition {
            StopCondition::IdleTimeout(duration) => Self::IdleTimeout(IdleTimeout::new(
                StandardDuration::from_secs(duration.value()),
            )),
            StopCondition::TurnCap(count) => {
                Self::TurnCap(TurnCap::new(TurnCount::new(count.value())))
            }
            StopCondition::CompletionSignal => Self::CompletionSignal,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum AdapterError {
    #[error("unsupported harness adapter capability: {capability}")]
    UnsupportedCapability { capability: String },

    #[error("sandbox violation at {path:?}: {reason}")]
    SandboxViolation { path: PathBuf, reason: String },

    #[error("sandbox io at {path:?}: {message}")]
    SandboxIo { path: PathBuf, message: String },

    #[error("jj sandbox initialization failed at {path:?}: {status}")]
    SandboxInitialization { path: PathBuf, status: String },

    #[error("Claude subscription TUI launcher unavailable: {reason}")]
    ClaudeLauncherUnavailable { reason: String },

    #[error("Claude subscription TUI launcher io at {path:?}: {message}")]
    ClaudeLauncherIo { path: PathBuf, message: String },

    #[error("forbidden Claude subscription TUI launcher at {path:?}: {reason}")]
    ForbiddenClaudeLauncher { path: PathBuf, reason: String },
}
