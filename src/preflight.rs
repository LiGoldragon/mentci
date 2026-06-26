use nota_next::{
    Delimiter, NotaBlock, NotaBodyEncoding, NotaDecode, NotaDecodeError, NotaEncode, NotaSource,
};

use crate::{Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightRequest {
    prompt: String,
    model_selection: ModelSelection,
    work_surface: WorkSurface,
    hard_constraints: Vec<LaunchConstraint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightPrompt {
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightModelOutput {
    nota: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedModelIdentifier {
    slot: ModelSlot,
    identifier: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelSlot {
    Preflight,
    HarnessSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelAvailability {
    Verified,
    Unavailable,
}

pub trait PreflightApi {
    fn model_availability(&self, identifier: &VerifiedModelIdentifier)
    -> Result<ModelAvailability>;

    fn complete(
        &self,
        prompt: &PreflightPrompt,
        identifier: &VerifiedModelIdentifier,
    ) -> Result<PreflightModelOutput>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightEngine<Api> {
    api: Api,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreflightLaunchEnvelope {
    MentciPreflightLaunch(MentciPreflightLaunch),
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct MentciPreflightLaunch {
    scaffold: ScaffoldPointer,
    session_identity: SessionIdentity,
    persistent_session: PersistentSession,
    sandbox_privacy: SandboxPrivacy,
    stop_conditions: Vec<StopCondition>,
    constraints: Vec<LaunchConstraint>,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct ScaffoldPointer {
    identity: ScaffoldIdentity,
    version: ScaffoldVersion,
    minimal_files: Vec<SourceLocator>,
    minimal_context: Vec<ContextLocator>,
    expansion_index: SkillIndexLocator,
    reuse_policy: ReusePolicy,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct ModelSelection {
    preflight_model: PreflightModelProfile,
    harness_session_model: HarnessSessionModelProfile,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct SessionIdentity {
    lane_name: LaneName,
    lane_metadata: Vec<LaneMetadata>,
    addressable_handle: SessionHandle,
    lookup_path: SessionLookupPath,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub enum LaneMetadata {
    Bead(MetadataValue),
    Repo(MetadataValue),
    WorkSurface(MetadataValue),
    HarnessLabel(MetadataValue),
}

#[derive(NotaDecode, NotaEncode, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentSession {
    Persistent,
    Ephemeral,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxPrivacy {
    SandboxedJjTask(PrimaryScope, PrivacySurface),
}

#[derive(NotaDecode, NotaEncode, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimaryScope {
    PrimaryForbidden,
}

#[derive(NotaDecode, NotaEncode, Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivacySurface {
    PrivateScopeClosed,
    PublicSurfaceAllowed,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub enum StopCondition {
    IdleTimeout(Duration),
    TurnCap(TurnCount),
    CompletionSignal,
}

#[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, PartialEq)]
pub enum LaunchConstraint {
    WorkSurface(WorkSurface),
    RequiredArtifact(SourceLocator),
    ForbiddenPath(Path),
    RequiredWitness(WitnessName),
    ImplementationBoundary(BoundaryName),
}

#[derive(NotaDecode, NotaEncode, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReusePolicy {
    ReuseDeferred,
}

macro_rules! text_newtype {
    ($name:ident) => {
        #[derive(NotaDecode, NotaEncode, Clone, Debug, Eq, Hash, PartialEq)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! integer_newtype {
    ($name:ident) => {
        #[derive(NotaDecode, NotaEncode, Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name(u64);

        impl $name {
            pub fn new(value: u64) -> Self {
                Self(value)
            }

            pub fn value(&self) -> u64 {
                self.0
            }
        }
    };
}

text_newtype!(ScaffoldIdentity);
integer_newtype!(ScaffoldVersion);
text_newtype!(SourceLocator);
text_newtype!(ContextLocator);
text_newtype!(SkillIndexLocator);
text_newtype!(PreflightModelProfile);
text_newtype!(HarnessSessionModelProfile);
text_newtype!(AdapterIdentity);
text_newtype!(TerminalCellDriverIdentity);
text_newtype!(LaneName);
text_newtype!(MetadataValue);
text_newtype!(SessionHandle);
text_newtype!(SessionLookupPath);
text_newtype!(WorkSurface);
text_newtype!(Path);
text_newtype!(WitnessName);
text_newtype!(BoundaryName);
integer_newtype!(Duration);
integer_newtype!(TurnCount);

impl PreflightRequest {
    pub fn new(
        prompt: impl Into<String>,
        model_selection: ModelSelection,
        work_surface: WorkSurface,
        hard_constraints: Vec<LaunchConstraint>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            model_selection,
            work_surface,
            hard_constraints,
        }
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn model_selection(&self) -> &ModelSelection {
        &self.model_selection
    }

    pub fn work_surface(&self) -> &WorkSurface {
        &self.work_surface
    }

    pub fn hard_constraints(&self) -> &[LaunchConstraint] {
        &self.hard_constraints
    }

    pub fn api_prompt(&self) -> PreflightPrompt {
        let mut text = String::new();
        text.push_str("Emit exactly one NOTA MentciPreflightLaunch record. ");
        text.push_str("Use the fixed schema in schema/preflight-launch.nota.md. ");
        text.push_str("Keep required slots separate: scaffold pointer, ");
        text.push_str("SessionIdentity, PersistentSession, SandboxPrivacy, typed ");
        text.push_str("StopCondition variants, and residual LaunchConstraint only. ");
        text.push_str("Do not include provider, adapter, terminal driver, or model literals ");
        text.push_str("in the launch packet. ");
        text.push_str("The scaffold must stay minimal and use skills/skills.nota as ");
        text.push_str("the expansion index. Prompt: ");
        text.push_str(&self.prompt);
        text.push_str(" Work surface: ");
        text.push_str(self.work_surface.as_str());
        Self::append_constraints(&mut text, &self.hard_constraints);
        PreflightPrompt { text }
    }

    fn append_constraints(text: &mut String, constraints: &[LaunchConstraint]) {
        if constraints.is_empty() {
            return;
        }
        text.push_str(" Hard constraints:");
        for constraint in constraints {
            text.push(' ');
            text.push_str(&constraint.to_nota());
        }
    }
}

impl PreflightPrompt {
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl PreflightModelOutput {
    pub fn new(nota: impl Into<String>) -> Self {
        Self { nota: nota.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.nota
    }
}

impl VerifiedModelIdentifier {
    pub fn for_preflight_profile(profile: &PreflightModelProfile) -> Result<Self> {
        match profile.as_str() {
            "cheap-contained-preflight" => Ok(Self {
                slot: ModelSlot::Preflight,
                identifier: profile.as_str().to_owned(),
            }),
            other => Err(Error::UnverifiedModel {
                slot: ModelSlot::Preflight.as_str(),
                profile: other.to_owned(),
                required_identifier: "cheap-contained-preflight".to_owned(),
            }),
        }
    }

    pub fn for_harness_profile(profile: &HarnessSessionModelProfile) -> Result<Self> {
        match profile.as_str() {
            "cheap-harness-session" => Ok(Self {
                slot: ModelSlot::HarnessSession,
                identifier: profile.as_str().to_owned(),
            }),
            other => Err(Error::UnverifiedModel {
                slot: ModelSlot::HarnessSession.as_str(),
                profile: other.to_owned(),
                required_identifier: "cheap-harness-session".to_owned(),
            }),
        }
    }

    pub fn slot(&self) -> ModelSlot {
        self.slot
    }

    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

impl ModelSlot {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preflight => "preflight model",
            Self::HarnessSession => "harness session model",
        }
    }
}

impl<Api> PreflightEngine<Api>
where
    Api: PreflightApi,
{
    pub fn new(api: Api) -> Self {
        Self { api }
    }

    pub fn launch(&self, request: &PreflightRequest) -> Result<MentciPreflightLaunch> {
        let preflight_identifier = VerifiedModelIdentifier::for_preflight_profile(
            request.model_selection.preflight_model(),
        )?;
        self.verify_model(
            &preflight_identifier,
            request.model_selection.preflight_model().as_str(),
        )?;
        let prompt = request.api_prompt();
        let output = self.api.complete(&prompt, &preflight_identifier)?;
        let launch = MentciPreflightLaunch::validated_from_nota(output.as_str())?;
        launch.validate_against_request(request)?;
        Ok(launch)
    }

    fn verify_model(&self, identifier: &VerifiedModelIdentifier, profile: &str) -> Result<()> {
        match self.api.model_availability(identifier)? {
            ModelAvailability::Verified => Ok(()),
            ModelAvailability::Unavailable => Err(Error::UnverifiedModel {
                slot: identifier.slot().as_str(),
                profile: profile.to_owned(),
                required_identifier: identifier.as_str().to_owned(),
            }),
        }
    }
}

impl MentciPreflightLaunch {
    pub fn validated_from_nota(source: &str) -> Result<Self> {
        let envelope = NotaSource::new(source)
            .parse::<PreflightLaunchEnvelope>()
            .map_err(Error::PreflightNota)?;
        let PreflightLaunchEnvelope::MentciPreflightLaunch(launch) = envelope;
        launch.validate()?;
        Ok(launch)
    }

    pub fn scaffold(&self) -> &ScaffoldPointer {
        &self.scaffold
    }

    pub fn session_identity(&self) -> &SessionIdentity {
        &self.session_identity
    }

    pub fn persistent_session(&self) -> PersistentSession {
        self.persistent_session
    }

    pub fn sandbox_privacy(&self) -> &SandboxPrivacy {
        &self.sandbox_privacy
    }

    pub fn stop_conditions(&self) -> &[StopCondition] {
        &self.stop_conditions
    }

    pub fn constraints(&self) -> &[LaunchConstraint] {
        &self.constraints
    }

    pub fn to_nota(&self) -> String {
        PreflightLaunchEnvelope::MentciPreflightLaunch(self.clone()).to_nota()
    }

    fn validate(&self) -> Result<()> {
        self.scaffold.validate()?;
        self.session_identity.validate()?;
        self.sandbox_privacy.validate();
        if self.stop_conditions.is_empty() {
            return Err(Error::PreflightLaunch(
                "at least one typed stop condition is required".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_against_request(&self, request: &PreflightRequest) -> Result<()> {
        if !self
            .constraints
            .iter()
            .any(|constraint| constraint.matches_work_surface(request.work_surface()))
        {
            return Err(Error::PreflightLaunch(
                "launch constraints must preserve requested work surface".to_owned(),
            ));
        }
        Ok(())
    }
}

impl NotaDecode for PreflightLaunchEnvelope {
    fn from_nota_block(block: &nota_next::Block) -> std::result::Result<Self, NotaDecodeError> {
        let body = NotaBlock::new(block).expect_body(Delimiter::Parenthesis, "PreflightLaunch")?;
        let children = body.expect_fields("MentciPreflightLaunch", 7)?;
        let variant = children[0]
            .demote_to_string()
            .ok_or(NotaDecodeError::ExpectedAtom {
                type_name: "MentciPreflightLaunch variant",
            })?;
        match variant {
            "MentciPreflightLaunch" => Ok(Self::MentciPreflightLaunch(
                MentciPreflightLaunch::from_root_fields(&children[1..])?,
            )),
            other => Err(NotaDecodeError::UnknownVariant {
                enum_name: "PreflightLaunch",
                variant: other.to_owned(),
            }),
        }
    }
}

impl NotaEncode for PreflightLaunchEnvelope {
    fn to_nota(&self) -> String {
        match self {
            Self::MentciPreflightLaunch(launch) => launch.to_root_nota(),
        }
    }
}

impl MentciPreflightLaunch {
    fn from_root_fields(fields: &[nota_next::Block]) -> std::result::Result<Self, NotaDecodeError> {
        Ok(Self {
            scaffold: ScaffoldPointer::from_nota_block(&fields[0])?,
            session_identity: SessionIdentity::from_nota_block(&fields[1])?,
            persistent_session: PersistentSession::from_nota_block(&fields[2])?,
            sandbox_privacy: SandboxPrivacy::from_nota_block(&fields[3])?,
            stop_conditions: Vec::<StopCondition>::from_nota_block(&fields[4])?,
            constraints: Vec::<LaunchConstraint>::from_nota_block(&fields[5])?,
        })
    }

    fn to_root_nota(&self) -> String {
        NotaBodyEncoding::new(vec![
            "MentciPreflightLaunch".to_owned(),
            self.scaffold.to_nota(),
            self.session_identity.to_nota(),
            self.persistent_session.to_nota(),
            self.sandbox_privacy.to_nota(),
            self.stop_conditions.to_nota(),
            self.constraints.to_nota(),
        ])
        .to_delimited_nota(Delimiter::Parenthesis)
    }
}

impl ScaffoldPointer {
    pub fn new(
        identity: ScaffoldIdentity,
        version: ScaffoldVersion,
        minimal_files: Vec<SourceLocator>,
        minimal_context: Vec<ContextLocator>,
        expansion_index: SkillIndexLocator,
        reuse_policy: ReusePolicy,
    ) -> Self {
        Self {
            identity,
            version,
            minimal_files,
            minimal_context,
            expansion_index,
            reuse_policy,
        }
    }

    pub fn identity(&self) -> &ScaffoldIdentity {
        &self.identity
    }

    pub fn version(&self) -> ScaffoldVersion {
        self.version
    }

    pub fn minimal_files(&self) -> &[SourceLocator] {
        &self.minimal_files
    }

    pub fn minimal_context(&self) -> &[ContextLocator] {
        &self.minimal_context
    }

    pub fn expansion_index(&self) -> &SkillIndexLocator {
        &self.expansion_index
    }

    pub fn reuse_policy(&self) -> ReusePolicy {
        self.reuse_policy
    }

    fn validate(&self) -> Result<()> {
        if self.expansion_index.as_str() != "skills/skills.nota" {
            return Err(Error::PreflightLaunch(
                "scaffold expansion index must be skills/skills.nota".to_owned(),
            ));
        }
        if self.version.value() == 0 {
            return Err(Error::PreflightLaunch(
                "scaffold version must be non-zero".to_owned(),
            ));
        }
        Ok(())
    }
}

impl ModelSelection {
    pub fn new(
        preflight_model: PreflightModelProfile,
        harness_session_model: HarnessSessionModelProfile,
    ) -> Self {
        Self {
            preflight_model,
            harness_session_model,
        }
    }

    pub fn preflight_model(&self) -> &PreflightModelProfile {
        &self.preflight_model
    }

    pub fn harness_session_model(&self) -> &HarnessSessionModelProfile {
        &self.harness_session_model
    }
}

impl SessionIdentity {
    pub fn new(
        lane_name: LaneName,
        lane_metadata: Vec<LaneMetadata>,
        addressable_handle: SessionHandle,
        lookup_path: SessionLookupPath,
    ) -> Self {
        Self {
            lane_name,
            lane_metadata,
            addressable_handle,
            lookup_path,
        }
    }

    pub fn lane_name(&self) -> &LaneName {
        &self.lane_name
    }

    pub fn lane_metadata(&self) -> &[LaneMetadata] {
        &self.lane_metadata
    }

    pub fn addressable_handle(&self) -> &SessionHandle {
        &self.addressable_handle
    }

    pub fn lookup_path(&self) -> &SessionLookupPath {
        &self.lookup_path
    }

    fn validate(&self) -> Result<()> {
        if self.lane_metadata.is_empty() {
            return Err(Error::PreflightLaunch(
                "session identity must carry lane metadata".to_owned(),
            ));
        }
        Ok(())
    }
}

impl SandboxPrivacy {
    fn validate(&self) {
        match self {
            Self::SandboxedJjTask(PrimaryScope::PrimaryForbidden, _) => {}
        }
    }
}

impl NotaDecode for SandboxPrivacy {
    fn from_nota_block(block: &nota_next::Block) -> std::result::Result<Self, NotaDecodeError> {
        let body = NotaBlock::new(block).expect_body(Delimiter::Parenthesis, "SandboxPrivacy")?;
        let children = body.expect_fields("SandboxPrivacy", 3)?;
        let variant = children[0]
            .demote_to_string()
            .ok_or(NotaDecodeError::ExpectedAtom {
                type_name: "SandboxPrivacy variant",
            })?;
        match variant {
            "SandboxedJjTask" => Ok(Self::SandboxedJjTask(
                PrimaryScope::from_nota_block(&children[1])?,
                PrivacySurface::from_nota_block(&children[2])?,
            )),
            other => Err(NotaDecodeError::UnknownVariant {
                enum_name: "SandboxPrivacy",
                variant: other.to_owned(),
            }),
        }
    }
}

impl NotaEncode for SandboxPrivacy {
    fn to_nota(&self) -> String {
        match self {
            Self::SandboxedJjTask(primary_scope, privacy_surface) => NotaBodyEncoding::new(vec![
                "SandboxedJjTask".to_owned(),
                primary_scope.to_nota(),
                privacy_surface.to_nota(),
            ])
            .to_delimited_nota(Delimiter::Parenthesis),
        }
    }
}

impl LaunchConstraint {
    fn matches_work_surface(&self, work_surface: &WorkSurface) -> bool {
        match self {
            Self::WorkSurface(value) => value == work_surface,
            _ => false,
        }
    }
}
