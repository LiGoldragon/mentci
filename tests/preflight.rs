use mentci::Error;
use mentci::preflight::{
    LaneMetadata, LaneName, LaunchConstraint, MentciPreflightLaunch, MetadataValue,
    ModelAvailability, ModelSelection, PersistentSession, PreflightApi, PreflightEngine,
    PreflightModelOutput, PreflightModelProfile, PreflightPrompt, PreflightRequest, PrimaryScope,
    PrivacySurface, ReusePolicy, SandboxPrivacy, ScaffoldIdentity, ScaffoldPointer,
    ScaffoldVersion, SessionHandle, SessionIdentity, SessionLookupPath, SkillIndexLocator,
    SourceLocator, StopCondition, TurnCount, VerifiedModelIdentifier, WorkSurface,
};

#[derive(Clone, Debug)]
struct FakePreflightApi {
    output: String,
    fail_completion: bool,
}

impl FakePreflightApi {
    fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            fail_completion: false,
        }
    }

    fn with_completion_failure(mut self) -> Self {
        self.fail_completion = true;
        self
    }
}

impl PreflightApi for FakePreflightApi {
    fn model_availability(
        &self,
        _identifier: &VerifiedModelIdentifier,
    ) -> mentci::Result<ModelAvailability> {
        Ok(ModelAvailability::Verified)
    }

    fn complete(
        &self,
        prompt: &PreflightPrompt,
        identifier: &VerifiedModelIdentifier,
    ) -> mentci::Result<PreflightModelOutput> {
        assert_eq!(identifier.as_str(), "cheap-contained-preflight");
        assert!(prompt.as_str().contains("MentciPreflightLaunch"));
        assert!(prompt.as_str().contains("skills/skills.dotos"));
        if self.fail_completion {
            return Err(Error::PreflightApi(
                "contained model call failed".to_owned(),
            ));
        }
        Ok(PreflightModelOutput::new(self.output.clone()))
    }
}

fn model_selection() -> ModelSelection {
    ModelSelection::new(
        PreflightModelProfile::new("cheap-contained-preflight"),
        mentci::preflight::HarnessSessionModelProfile::new("cheap-harness-session"),
    )
}

fn request() -> PreflightRequest {
    PreflightRequest::new(
        "Build the Mentci API preflight path",
        model_selection(),
        WorkSurface::new("sandboxed-jj-task"),
        vec![LaunchConstraint::ForbiddenPath(
            mentci::preflight::Path::new("/home/li/primary"),
        )],
    )
}

/// The launch packet the datom fixture below must compose to, built as a value.
fn expected_launch() -> MentciPreflightLaunch {
    MentciPreflightLaunch::new(
        ScaffoldPointer::new(
            ScaffoldIdentity::new("mentci-prompt-scaffold"),
            ScaffoldVersion::new(1),
            vec![SourceLocator::new("skills/skills.dotos")],
            vec![mentci::preflight::ContextLocator::new("ARCHITECTURE.md")],
            SkillIndexLocator::new("skills/skills.dotos"),
            ReusePolicy::ReuseDeferred,
        ),
        SessionIdentity::new(
            LaneName::new("mentci-primary-k6va"),
            vec![
                LaneMetadata::Bead(MetadataValue::new("primary-k6va")),
                LaneMetadata::WorkSurface(MetadataValue::new("sandboxed-jj-task")),
            ],
            SessionHandle::new("primary-k6va-session"),
            SessionLookupPath::new("orchestrate/lanes/primary-k6va"),
        ),
        PersistentSession::Persistent,
        SandboxPrivacy::SandboxedJjTask(
            PrimaryScope::PrimaryForbidden,
            PrivacySurface::PrivateScopeClosed,
        ),
        vec![
            StopCondition::IdleTimeout(mentci::preflight::Duration::new(600)),
            StopCondition::TurnCap(TurnCount::new(8)),
            StopCondition::CompletionSignal,
        ],
        vec![
            LaunchConstraint::WorkSurface(WorkSurface::new("sandboxed-jj-task")),
            LaunchConstraint::ForbiddenPath(mentci::preflight::Path::new("/home/li/primary")),
        ],
    )
}

/// One canonical datom `MentciPreflightLaunch`, written out by hand exactly as
/// the type projects it. Every line of this text is actualized by
/// `canonical_launch_text_actualizes_to_the_expected_value` below.
fn valid_launch_datom() -> String {
    concat!(
        "MentciPreflightLaunch.{\n",
        "  { { mentci-prompt-scaffold } { 1 } [ { skills/skills.dotos } ] ",
        "[ { ARCHITECTURE.md } ] { skills/skills.dotos } ReuseDeferred }\n",
        "  { { mentci-primary-k6va } ",
        "[ Bead.{ primary-k6va } WorkSurface.{ sandboxed-jj-task } ] ",
        "{ primary-k6va-session } { orchestrate/lanes/primary-k6va } }\n",
        "  Persistent\n",
        "  SandboxedJjTask.{ PrimaryForbidden PrivateScopeClosed }\n",
        "  [ IdleTimeout.{ 600 } TurnCap.{ 8 } CompletionSignal ]\n",
        "  [ WorkSurface.{ sandboxed-jj-task } ForbiddenPath.{ /home/li/primary } ]\n",
        "}"
    )
    .to_owned()
}

#[test]
fn canonical_launch_text_actualizes_to_the_expected_value() {
    let launch = MentciPreflightLaunch::validated_from_datom(&valid_launch_datom())
        .expect("hand-written canonical datom actualizes");

    assert_eq!(launch, expected_launch());
}

#[test]
fn launch_value_projects_back_to_text_that_actualizes() {
    let projected = expected_launch().to_datom_text();

    let recovered = MentciPreflightLaunch::validated_from_datom(&projected)
        .expect("projected datom actualizes");

    assert_eq!(recovered, expected_launch());
}

#[test]
fn preflight_path_calls_api_and_validates_launch_packet() {
    let engine = PreflightEngine::new(FakePreflightApi::new(valid_launch_datom()));

    let launch = engine.launch(&request()).expect("valid launch");

    assert_eq!(
        launch.scaffold().identity().as_str(),
        "mentci-prompt-scaffold"
    );
    assert_eq!(launch.scaffold().version().value(), 1);
    assert_eq!(
        launch.scaffold().expansion_index().as_str(),
        "skills/skills.dotos"
    );
    assert!(matches!(
        launch.sandbox_privacy(),
        SandboxPrivacy::SandboxedJjTask(_, PrivacySurface::PrivateScopeClosed)
    ));
    assert_eq!(launch.stop_conditions().len(), 3);
}

#[test]
fn preflight_model_slots_reject_provider_specific_identifiers() {
    let provider_model_request = PreflightRequest::new(
        "Build the Mentci API preflight path",
        ModelSelection::new(
            PreflightModelProfile::new("claude-haiku-4-5-20251001"),
            mentci::preflight::HarnessSessionModelProfile::new("cheap-harness-session"),
        ),
        WorkSurface::new("sandboxed-jj-task"),
        Vec::new(),
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(valid_launch_datom()));

    let error = engine
        .launch(&provider_model_request)
        .expect_err("provider-specific preflight model rejected");

    assert!(matches!(
        error,
        Error::UnverifiedModel { slot, profile, required_identifier }
            if slot == "preflight model"
                && profile == "claude-haiku-4-5-20251001"
                && required_identifier == "cheap-contained-preflight"
    ));
}

#[test]
fn preflight_rejects_generic_compression_or_missing_named_slots() {
    let compressed = "MentciPreflightLaunch.{ { { mentci-prompt-scaffold } { 1 } [] [] \
                      { skills/skills.dotos } ReuseDeferred } [] [ Constraint.{ session } ] }";
    let engine = PreflightEngine::new(FakePreflightApi::new(compressed));

    let error = engine
        .launch(&request())
        .expect_err("compressed output rejected");

    assert!(matches!(error, Error::Datom(_) | Error::PreflightLaunch(_)));
}

#[test]
fn preflight_rejects_route_or_harness_target_in_launch_packet() {
    let route_bearing_packet = valid_launch_datom().replace(
        "  Persistent\n",
        "  { { cheap-contained-preflight } { cheap-harness-session } }\n  Persistent\n",
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(route_bearing_packet));

    let error = engine
        .launch(&request())
        .expect_err("provider route rejected");

    assert!(matches!(error, Error::Datom(_)));
}

#[test]
fn preflight_reports_unverified_model_before_guessing() {
    let request = PreflightRequest::new(
        "Build the Mentci API preflight path",
        ModelSelection::new(
            PreflightModelProfile::new("some-new-model"),
            mentci::preflight::HarnessSessionModelProfile::new("cheap-harness-session"),
        ),
        WorkSurface::new("sandboxed-jj-task"),
        Vec::new(),
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(valid_launch_datom()));

    let error = engine
        .launch(&request)
        .expect_err("unverified model rejected");

    assert!(matches!(
        error,
        Error::UnverifiedModel { slot, profile, .. }
            if slot == "preflight model" && profile == "some-new-model"
    ));
}

#[test]
fn preflight_reports_model_call_failure() {
    let engine =
        PreflightEngine::new(FakePreflightApi::new(valid_launch_datom()).with_completion_failure());

    let error = engine
        .launch(&request())
        .expect_err("model call failure reported");

    assert!(matches!(error, Error::PreflightApi(message) if message.contains("model call")));
}

#[test]
fn preflight_rejects_scaffold_without_skills_index() {
    let wrong_index = valid_launch_datom().replace(
        "{ skills/skills.dotos } ReuseDeferred",
        "{ reports/operator } ReuseDeferred",
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(wrong_index));

    let error = engine
        .launch(&request())
        .expect_err("bad scaffold rejected");

    assert!(
        matches!(error, Error::PreflightLaunch(message) if message.contains("skills/skills.dotos"))
    );
}
