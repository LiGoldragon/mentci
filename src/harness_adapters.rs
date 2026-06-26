use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration as StandardDuration, SystemTime, UNIX_EPOCH};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeCodeAdapter {
    executable: String,
    terminal_size: TerminalSize,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self {
            executable: "claude".to_owned(),
            terminal_size: TerminalSize::new(36, 120),
        }
    }

    pub fn with_executable(mut self, executable: impl Into<String>) -> Self {
        self.executable = executable.into();
        self
    }

    pub fn launch(
        &self,
        request: ClaudeCodeLaunchRequest,
    ) -> Result<NamedHarnessLaunch, AdapterError> {
        self.validate_launch(&request)?;
        let terminal_launch = TerminalLaunch::new(
            TerminalCommand::new(self.executable.clone(), self.arguments(&request)),
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
}
