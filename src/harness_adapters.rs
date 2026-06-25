use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration as StandardDuration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::harness_liveness::{
    CloseRequest, IdleTimeout, LaunchRequest, LivenessPolicy,
    SandboxPrivacy as DriverSandboxPrivacy, SandboxPrivacyFlag,
    StopCondition as DriverStopCondition, StopConditions, TerminalCommand, TerminalFeed,
    TerminalLaunch, TerminalSize, TerminalWorkingDirectory, TurnCap, TurnCount,
};
use crate::harness_sessions::NamedHarnessLaunch;
use crate::preflight::{
    HarnessTarget, MentciPreflightLaunch, ModelSlot, PrivacySurface,
    SandboxPrivacy as PreflightSandboxPrivacy, StopCondition,
};

const PRIMARY_WORKSPACE: &str = "/home/li/primary";
const EPHEMERAL_SANDBOX_MARKER: &str = ".mentci-ephemeral-jj-sandbox";
const CLAUDE_CODE_ADAPTER: &str = "claude-code-terminal-adapter";
const TERMINAL_CELL_DRIVER: &str = "terminal-cell-v1";
const CLAUDE_HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

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
        let model = ClaudeCodeModel::from_launch(request.preflight_launch())?;
        let terminal_launch = TerminalLaunch::new(
            TerminalCommand::new(self.executable.clone(), self.arguments(&request, &model)),
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
        ))
    }

    pub fn feed(&self, input: HarnessFeed) -> Result<TerminalFeed, AdapterError> {
        Ok(TerminalFeed::new(input.into_terminal_bytes()))
    }

    pub fn close_request(&self) -> CloseRequest {
        CloseRequest::TerminalInput(TerminalFeed::new(b"/exit\r".to_vec()))
    }

    fn validate_launch(&self, request: &ClaudeCodeLaunchRequest) -> Result<(), AdapterError> {
        let HarnessTarget::ClaudeCode(driver) = request.preflight_launch().route().harness_target()
        else {
            return Err(AdapterError::UnsupportedCapability {
                capability: "ClaudeCode harness target".to_owned(),
            });
        };
        if driver.adapter().as_str() != CLAUDE_CODE_ADAPTER {
            return Err(AdapterError::UnsupportedCapability {
                capability: format!("adapter {}", driver.adapter().as_str()),
            });
        }
        if driver.terminal_cell_driver().as_str() != TERMINAL_CELL_DRIVER {
            return Err(AdapterError::UnsupportedCapability {
                capability: format!(
                    "terminal-cell driver {}",
                    driver.terminal_cell_driver().as_str()
                ),
            });
        }
        request.sandbox().validate()?;
        Ok(())
    }

    fn arguments(&self, request: &ClaudeCodeLaunchRequest, model: &ClaudeCodeModel) -> Vec<String> {
        vec![
            "--bare".to_owned(),
            "--model".to_owned(),
            model.as_str().to_owned(),
            "--permission-mode".to_owned(),
            "bypassPermissions".to_owned(),
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
        TerminalFeed::new(prompt.into_bytes())
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

    fn into_terminal_bytes(self) -> Vec<u8> {
        let mut bytes = self.text.into_bytes();
        bytes.push(b'\r');
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EphemeralJjRepository {
    working_directory: PathBuf,
}

impl EphemeralJjRepository {
    pub fn create_in(parent_directory: impl AsRef<Path>) -> Result<Self, AdapterError> {
        let working_directory = EphemeralRepositoryName::new().path_under(parent_directory);
        std::fs::create_dir_all(&working_directory).map_err(|source| AdapterError::SandboxIo {
            path: working_directory.clone(),
            message: source.to_string(),
        })?;
        let status = Command::new("jj")
            .arg("git")
            .arg("init")
            .arg("--colocate")
            .current_dir(&working_directory)
            .status()
            .map_err(|source| AdapterError::SandboxIo {
                path: working_directory.clone(),
                message: source.to_string(),
            })?;
        if !status.success() {
            return Err(AdapterError::SandboxInitialization {
                path: working_directory,
                status: status.to_string(),
            });
        }
        Self::write_marker(&working_directory)?;
        Self::from_existing_ephemeral(working_directory)
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
        let primary = Path::new(PRIMARY_WORKSPACE);
        if canonical == primary || canonical.starts_with(primary) {
            return Err(AdapterError::SandboxViolation {
                path: canonical,
                reason: "primary workspace cannot be a jj proof working copy".to_owned(),
            });
        }
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
        self.working_directory
            .canonicalize()
            .map_err(|source| AdapterError::SandboxIo {
                path: self.working_directory.clone(),
                message: source.to_string(),
            })
    }

    fn write_marker(working_directory: &Path) -> Result<(), AdapterError> {
        let marker = working_directory.join(EPHEMERAL_SANDBOX_MARKER);
        std::fs::write(&marker, "mentci ephemeral jj sandbox\n").map_err(|source| {
            AdapterError::SandboxIo {
                path: marker,
                message: source.to_string(),
            }
        })
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaudeCodeModel {
    identifier: String,
}

impl ClaudeCodeModel {
    fn from_launch(launch: &MentciPreflightLaunch) -> Result<Self, AdapterError> {
        let profile = launch
            .route()
            .model_selection()
            .harness_session_model()
            .as_str();
        match profile {
            CLAUDE_HAIKU_MODEL => Ok(Self {
                identifier: CLAUDE_HAIKU_MODEL.to_owned(),
            }),
            other => Err(AdapterError::UnverifiedModel {
                slot: ModelSlot::HarnessSession.as_str(),
                profile: other.to_owned(),
                required_identifier: CLAUDE_HAIKU_MODEL.to_owned(),
            }),
        }
    }

    fn as_str(&self) -> &str {
        self.identifier.as_str()
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

    #[error("unverified model for {slot}: profile {profile} requires {required_identifier}")]
    UnverifiedModel {
        slot: &'static str,
        profile: String,
        required_identifier: String,
    },

    #[error("sandbox violation at {path:?}: {reason}")]
    SandboxViolation { path: PathBuf, reason: String },

    #[error("sandbox io at {path:?}: {message}")]
    SandboxIo { path: PathBuf, message: String },

    #[error("jj sandbox initialization failed at {path:?}: {status}")]
    SandboxInitialization { path: PathBuf, status: String },
}
