use std::collections::HashMap;

use thiserror::Error;

use crate::harness_liveness::{
    CloseRequest, DriverError, LaunchRequest, LiveHarnessSession, ReadOutcome, TerminalCellDriver,
    TerminalFeed, TerminalSessionLauncher,
};
use crate::preflight::{
    AdapterIdentity, HarnessSessionModelProfile, LaneMetadata, LaneName, MentciPreflightLaunch,
    PersistentSession, SandboxPrivacy, ScaffoldIdentity, ScaffoldVersion, SessionHandle,
    SessionIdentity, TerminalCellDriverIdentity,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedHarnessLaunch {
    preflight_launch: MentciPreflightLaunch,
    terminal_launch: LaunchRequest,
    launch_metadata: HarnessLaunchMetadata,
}

impl NamedHarnessLaunch {
    pub fn new(
        preflight_launch: MentciPreflightLaunch,
        terminal_launch: LaunchRequest,
        launch_metadata: HarnessLaunchMetadata,
    ) -> Self {
        Self {
            preflight_launch,
            terminal_launch,
            launch_metadata,
        }
    }

    pub fn preflight_launch(&self) -> &MentciPreflightLaunch {
        &self.preflight_launch
    }

    pub fn terminal_launch(&self) -> &LaunchRequest {
        &self.terminal_launch
    }

    pub fn launch_metadata(&self) -> &HarnessLaunchMetadata {
        &self.launch_metadata
    }

    fn into_terminal_launch(self) -> LaunchRequest {
        self.terminal_launch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessLaunchMetadata {
    harness_kind: HarnessKind,
    adapter: AdapterIdentity,
    terminal_cell_driver: TerminalCellDriverIdentity,
    harness_session_model: HarnessSessionModelProfile,
}

impl HarnessLaunchMetadata {
    pub fn new(
        harness_kind: HarnessKind,
        adapter: AdapterIdentity,
        terminal_cell_driver: TerminalCellDriverIdentity,
        harness_session_model: HarnessSessionModelProfile,
    ) -> Self {
        Self {
            harness_kind,
            adapter,
            terminal_cell_driver,
            harness_session_model,
        }
    }

    pub fn harness_kind(&self) -> HarnessKind {
        self.harness_kind
    }

    pub fn adapter(&self) -> &AdapterIdentity {
        &self.adapter
    }

    pub fn terminal_cell_driver(&self) -> &TerminalCellDriverIdentity {
        &self.terminal_cell_driver
    }

    pub fn harness_session_model(&self) -> &HarnessSessionModelProfile {
        &self.harness_session_model
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionAddressRecord {
    identity: SessionIdentity,
    persistent_session: PersistentSession,
    metadata: SessionAddressMetadata,
    state: SessionRecordState,
}

impl SessionAddressRecord {
    pub fn from_launch(
        launch: &MentciPreflightLaunch,
        launch_metadata: &HarnessLaunchMetadata,
    ) -> Self {
        Self {
            identity: launch.session_identity().clone(),
            persistent_session: launch.persistent_session(),
            metadata: SessionAddressMetadata::from_launch(launch, launch_metadata),
            state: SessionRecordState::Open,
        }
    }

    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
    }

    pub fn persistent_session(&self) -> PersistentSession {
        self.persistent_session
    }

    pub fn metadata(&self) -> &SessionAddressMetadata {
        &self.metadata
    }

    pub fn state(&self) -> SessionRecordState {
        self.state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionAddressMetadata {
    lane_metadata: Vec<LaneMetadata>,
    scaffold_identity: ScaffoldIdentity,
    scaffold_version: ScaffoldVersion,
    harness_kind: HarnessKind,
    adapter: AdapterIdentity,
    terminal_cell_driver: TerminalCellDriverIdentity,
    harness_session_model: HarnessSessionModelProfile,
    sandbox_privacy: SandboxPrivacy,
}

impl SessionAddressMetadata {
    pub fn from_launch(
        launch: &MentciPreflightLaunch,
        launch_metadata: &HarnessLaunchMetadata,
    ) -> Self {
        Self {
            lane_metadata: launch.session_identity().lane_metadata().to_vec(),
            scaffold_identity: launch.scaffold().identity().clone(),
            scaffold_version: launch.scaffold().version(),
            harness_kind: launch_metadata.harness_kind(),
            adapter: launch_metadata.adapter().clone(),
            terminal_cell_driver: launch_metadata.terminal_cell_driver().clone(),
            harness_session_model: launch_metadata.harness_session_model().clone(),
            sandbox_privacy: launch.sandbox_privacy().clone(),
        }
    }

    pub fn lane_metadata(&self) -> &[LaneMetadata] {
        self.lane_metadata.as_slice()
    }

    pub fn scaffold_identity(&self) -> &ScaffoldIdentity {
        &self.scaffold_identity
    }

    pub fn scaffold_version(&self) -> ScaffoldVersion {
        self.scaffold_version
    }

    pub fn harness_kind(&self) -> HarnessKind {
        self.harness_kind
    }

    pub fn adapter(&self) -> &AdapterIdentity {
        &self.adapter
    }

    pub fn terminal_cell_driver(&self) -> &TerminalCellDriverIdentity {
        &self.terminal_cell_driver
    }

    pub fn harness_session_model(&self) -> &HarnessSessionModelProfile {
        &self.harness_session_model
    }

    pub fn sandbox_privacy(&self) -> &SandboxPrivacy {
        &self.sandbox_privacy
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HarnessKind {
    ClaudeCode,
    Codex,
    Pi,
    OpenEndedHarness,
}

impl HarnessKind {
    pub fn claude_code() -> Self {
        Self::ClaudeCode
    }

    pub fn codex() -> Self {
        Self::Codex
    }

    pub fn pi() -> Self {
        Self::Pi
    }

    pub fn open_ended_harness() -> Self {
        Self::OpenEndedHarness
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRecordState {
    Open,
    Closed,
    Retired,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SessionAddress {
    LaneName(LaneName),
    Handle(SessionHandle),
}

impl SessionAddress {
    pub fn lane_name(value: LaneName) -> Self {
        Self::LaneName(value)
    }

    pub fn handle(value: SessionHandle) -> Self {
        Self::Handle(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRegistrationStatus {
    Registered,
    Existing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRegistration {
    record: SessionAddressRecord,
    status: SessionRegistrationStatus,
}

impl SessionRegistration {
    pub fn new(record: SessionAddressRecord, status: SessionRegistrationStatus) -> Self {
        Self { record, status }
    }

    pub fn record(&self) -> &SessionAddressRecord {
        &self.record
    }

    pub fn into_record(self) -> SessionAddressRecord {
        self.record
    }

    pub fn status(&self) -> SessionRegistrationStatus {
        self.status
    }
}

pub trait HarnessSessionDirectory {
    fn register_or_reuse(
        &mut self,
        record: SessionAddressRecord,
    ) -> Result<SessionRegistration, SessionLookupError>;

    fn resolve(&self, address: &SessionAddress)
    -> Result<SessionAddressRecord, SessionLookupError>;

    fn mark_closed(
        &mut self,
        handle: &SessionHandle,
    ) -> Result<SessionAddressRecord, SessionLookupError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryHarnessSessionDirectory {
    by_lane_name: HashMap<LaneName, SessionHandle>,
    by_handle: HashMap<SessionHandle, SessionAddressRecord>,
}

impl InMemoryHarnessSessionDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_record(
        &mut self,
        record: SessionAddressRecord,
    ) -> Result<SessionRegistration, SessionLookupError> {
        self.register_or_reuse(record)
    }

    pub fn mark_retired(
        &mut self,
        handle: &SessionHandle,
    ) -> Result<SessionAddressRecord, SessionLookupError> {
        self.mark_state(handle, SessionRecordState::Retired)
    }

    fn handle_for_address(
        &self,
        address: &SessionAddress,
    ) -> Result<SessionHandle, SessionLookupError> {
        match address {
            SessionAddress::LaneName(lane_name) => self
                .by_lane_name
                .get(lane_name)
                .ok_or_else(|| SessionLookupError::UnknownSession {
                    address: address.clone(),
                })
                .cloned(),
            SessionAddress::Handle(handle) => {
                if self.by_handle.contains_key(handle) {
                    Ok(handle.clone())
                } else {
                    Err(SessionLookupError::UnknownSession {
                        address: address.clone(),
                    })
                }
            }
        }
    }

    fn mark_state(
        &mut self,
        handle: &SessionHandle,
        state: SessionRecordState,
    ) -> Result<SessionAddressRecord, SessionLookupError> {
        let Some(record) = self.by_handle.get_mut(handle) else {
            return Err(SessionLookupError::UnknownSession {
                address: SessionAddress::handle(handle.clone()),
            });
        };
        record.state = state;
        Ok(record.clone())
    }
}

impl HarnessSessionDirectory for InMemoryHarnessSessionDirectory {
    fn register_or_reuse(
        &mut self,
        record: SessionAddressRecord,
    ) -> Result<SessionRegistration, SessionLookupError> {
        let lane_name = record.identity().lane_name().clone();
        let handle = record.identity().addressable_handle().clone();

        if let Some(existing_handle) = self.by_lane_name.get(&lane_name) {
            let existing = self
                .by_handle
                .get(existing_handle)
                .expect("indexed session");
            if existing.state != SessionRecordState::Open {
                return Err(SessionLookupError::ClosedSession {
                    address: SessionAddress::lane_name(lane_name),
                    handle: existing_handle.clone(),
                    state: existing.state,
                });
            }
            if existing.identity() == record.identity()
                && existing.persistent_session() == record.persistent_session()
                && existing.metadata() == record.metadata()
            {
                return Ok(SessionRegistration::new(
                    existing.clone(),
                    SessionRegistrationStatus::Existing,
                ));
            }
            return Err(SessionLookupError::AddressConflict {
                lane_name,
                existing: existing.metadata().clone(),
                requested: record.metadata().clone(),
            });
        }

        if self.by_handle.contains_key(&handle) {
            return Err(SessionLookupError::DuplicateSessionHandle { lane_name, handle });
        }

        self.by_lane_name.insert(lane_name, handle.clone());
        self.by_handle.insert(handle, record.clone());
        Ok(SessionRegistration::new(
            record,
            SessionRegistrationStatus::Registered,
        ))
    }

    fn resolve(
        &self,
        address: &SessionAddress,
    ) -> Result<SessionAddressRecord, SessionLookupError> {
        let handle = self.handle_for_address(address)?;
        let record = self.by_handle.get(&handle).expect("indexed session");
        if record.state != SessionRecordState::Open {
            return Err(SessionLookupError::ClosedSession {
                address: address.clone(),
                handle,
                state: record.state,
            });
        }
        Ok(record.clone())
    }

    fn mark_closed(
        &mut self,
        handle: &SessionHandle,
    ) -> Result<SessionAddressRecord, SessionLookupError> {
        self.mark_state(handle, SessionRecordState::Closed)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionLookupError {
    #[error("unknown harness session address: {address:?}")]
    UnknownSession { address: SessionAddress },

    #[error("duplicate harness session handle {handle:?} for lane {lane_name:?}")]
    DuplicateSessionHandle {
        lane_name: LaneName,
        handle: SessionHandle,
    },

    #[error("harness session address conflict for lane {lane_name:?}")]
    AddressConflict {
        lane_name: LaneName,
        existing: SessionAddressMetadata,
        requested: SessionAddressMetadata,
    },

    #[error("harness session address is not open: {address:?}")]
    ClosedSession {
        address: SessionAddress,
        handle: SessionHandle,
        state: SessionRecordState,
    },
}

#[derive(Debug, Error)]
pub enum SessionRoutingError {
    #[error("{0}")]
    Lookup(#[from] SessionLookupError),

    #[error("{0}")]
    Driver(#[from] DriverError),

    #[error("orchestrate resolved session address without a local terminal session: {handle:?}")]
    StaleSession { handle: SessionHandle },
}

pub struct NamedHarnessSessions<Directory, Launcher>
where
    Directory: HarnessSessionDirectory,
    Launcher: TerminalSessionLauncher,
{
    directory: Directory,
    driver: TerminalCellDriver<Launcher>,
    sessions: HashMap<SessionHandle, LiveHarnessSession<Launcher::Session>>,
}

impl<Directory, Launcher> NamedHarnessSessions<Directory, Launcher>
where
    Directory: HarnessSessionDirectory,
    Launcher: TerminalSessionLauncher,
{
    pub fn new(directory: Directory, driver: TerminalCellDriver<Launcher>) -> Self {
        Self {
            directory,
            driver,
            sessions: HashMap::new(),
        }
    }

    pub fn launch(
        &mut self,
        request: NamedHarnessLaunch,
    ) -> Result<SessionAddressRecord, SessionRoutingError> {
        let record = SessionAddressRecord::from_launch(
            request.preflight_launch(),
            request.launch_metadata(),
        );
        let registration = self.directory.register_or_reuse(record)?;
        let handle = registration
            .record()
            .identity()
            .addressable_handle()
            .clone();
        if registration.status() == SessionRegistrationStatus::Existing {
            if self.sessions.contains_key(&handle) {
                return Ok(registration.into_record());
            }
            return Err(SessionRoutingError::StaleSession { handle });
        }

        let session = self.driver.launch(request.into_terminal_launch())?;
        self.sessions.insert(handle, session);
        Ok(registration.into_record())
    }

    pub fn feed(
        &mut self,
        address: &SessionAddress,
        feed: TerminalFeed,
    ) -> Result<Option<ReadOutcome>, SessionRoutingError> {
        let handle = self.resolved_handle(address)?;
        let session = self
            .sessions
            .get_mut(&handle)
            .ok_or(SessionRoutingError::StaleSession { handle })?;
        session.send(feed).map_err(SessionRoutingError::Driver)
    }

    pub fn read(&mut self, address: &SessionAddress) -> Result<ReadOutcome, SessionRoutingError> {
        let handle = self.resolved_handle(address)?;
        let session = self
            .sessions
            .get_mut(&handle)
            .ok_or(SessionRoutingError::StaleSession { handle })?;
        session
            .read_until_stop()
            .map_err(SessionRoutingError::Driver)
    }

    pub fn close(
        &mut self,
        address: &SessionAddress,
        request: CloseRequest,
    ) -> Result<ReadOutcome, SessionRoutingError> {
        let handle = self.resolved_handle(address)?;
        let session =
            self.sessions
                .get_mut(&handle)
                .ok_or_else(|| SessionRoutingError::StaleSession {
                    handle: handle.clone(),
                })?;
        let outcome = session.close(request)?;
        self.sessions.remove(&handle);
        self.directory.mark_closed(&handle)?;
        Ok(outcome)
    }

    pub fn directory(&self) -> &Directory {
        &self.directory
    }

    pub fn directory_mut(&mut self) -> &mut Directory {
        &mut self.directory
    }

    fn resolved_handle(
        &self,
        address: &SessionAddress,
    ) -> Result<SessionHandle, SessionLookupError> {
        Ok(self
            .directory
            .resolve(address)?
            .identity()
            .addressable_handle()
            .clone())
    }
}
