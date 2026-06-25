use mentci::Error;
use mentci::preflight::{
    LaunchConstraint, ModelAvailability, ModelSelection, PreflightApi, PreflightEngine,
    PreflightModelOutput, PreflightModelProfile, PreflightPrompt, PreflightRequest,
    VerifiedModelIdentifier, WorkSurface,
};

#[derive(Clone, Debug)]
struct FakePreflightApi {
    output: String,
    unavailable_model: Option<String>,
    fail_completion: bool,
}

impl FakePreflightApi {
    fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            unavailable_model: None,
            fail_completion: false,
        }
    }

    fn with_unavailable_model(mut self, model: impl Into<String>) -> Self {
        self.unavailable_model = Some(model.into());
        self
    }

    fn with_completion_failure(mut self) -> Self {
        self.fail_completion = true;
        self
    }
}

impl PreflightApi for FakePreflightApi {
    fn model_availability(
        &self,
        identifier: &VerifiedModelIdentifier,
    ) -> mentci::Result<ModelAvailability> {
        if self
            .unavailable_model
            .as_ref()
            .is_some_and(|model| model == identifier.as_str())
        {
            return Ok(ModelAvailability::Unavailable);
        }
        Ok(ModelAvailability::Verified)
    }

    fn complete(
        &self,
        prompt: &PreflightPrompt,
        identifier: &VerifiedModelIdentifier,
    ) -> mentci::Result<PreflightModelOutput> {
        assert_eq!(identifier.as_str(), "claude-haiku-4-5-20251001");
        assert!(prompt.as_str().contains("MentciPreflightLaunch"));
        assert!(prompt.as_str().contains("skills/skills.nota"));
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

fn valid_launch_nota() -> String {
    r#"(MentciPreflightLaunch
  (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred)
  ([(beads skills/beads.md [claim and update the bead])
    (nota-design skills/nota-design.md [preserve positional NOTA shape])]
   (cheap-contained-preflight cheap-harness-session)
   (Codex codex-terminal-adapter terminal-cell-v1)
   [Prompt requires a sandboxed jj task and a persistent named harness session])
  (mentci-primary-k6va [(Bead primary-k6va) (WorkSurface sandboxed-jj-task)] primary-k6va-session orchestrate/lanes/primary-k6va)
  Persistent
  (SandboxedJjTask PrimaryForbidden PrivateScopeClosed)
  [(IdleTimeout 600) (TurnCap 8) CompletionSignal]
  [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary)])"#
        .to_owned()
}

#[test]
fn preflight_path_calls_api_and_validates_launch_packet() {
    let engine = PreflightEngine::new(FakePreflightApi::new(valid_launch_nota()));

    let launch = engine.launch(&request()).expect("valid launch");

    assert_eq!(
        launch.scaffold().identity().as_str(),
        "mentci-prompt-scaffold"
    );
    assert_eq!(launch.scaffold().version().value(), 1);
    assert_eq!(
        launch.scaffold().expansion_index().as_str(),
        "skills/skills.nota"
    );
    assert_eq!(launch.route().chosen_skills().len(), 2);
    assert_eq!(launch.stop_conditions().len(), 3);
}

#[test]
fn preflight_rejects_generic_compression_or_missing_named_slots() {
    let compressed = "(MentciPreflightLaunch (mentci-prompt-scaffold 1 [] [] skills/skills.nota ReuseDeferred) [] [(Constraint [session primary-k6va])])";
    let engine = PreflightEngine::new(FakePreflightApi::new(compressed));

    let error = engine
        .launch(&request())
        .expect_err("compressed output rejected");

    assert!(matches!(
        error,
        Error::PreflightNota(_) | Error::PreflightLaunch(_)
    ));
}

#[test]
fn preflight_rejects_missing_skill_selection() {
    let missing_skills = valid_launch_nota().replace(
        "[(beads skills/beads.md [claim and update the bead])\n    (nota-design skills/nota-design.md [preserve positional NOTA shape])]",
        "[]",
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(missing_skills));

    let error = engine
        .launch(&request())
        .expect_err("missing skills rejected");

    assert!(matches!(error, Error::PreflightLaunch(message) if message.contains("chosen skill")));
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
    let engine = PreflightEngine::new(FakePreflightApi::new(valid_launch_nota()));

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
fn preflight_reports_runtime_unavailable_verified_model() {
    let engine = PreflightEngine::new(
        FakePreflightApi::new(valid_launch_nota()).with_unavailable_model("gpt-5.4-mini"),
    );

    let error = engine
        .launch(&request())
        .expect_err("unavailable harness model rejected");

    assert!(matches!(
        error,
        Error::UnverifiedModel { slot, required_identifier, .. }
            if slot == "harness session model" && required_identifier == "gpt-5.4-mini"
    ));
}

#[test]
fn preflight_reports_model_call_failure() {
    let engine =
        PreflightEngine::new(FakePreflightApi::new(valid_launch_nota()).with_completion_failure());

    let error = engine
        .launch(&request())
        .expect_err("model call failure reported");

    assert!(matches!(error, Error::PreflightApi(message) if message.contains("model call")));
}

#[test]
fn preflight_rejects_scaffold_without_skills_index() {
    let wrong_index = valid_launch_nota().replace(
        "skills/skills.nota ReuseDeferred",
        "reports/operator ReuseDeferred",
    );
    let engine = PreflightEngine::new(FakePreflightApi::new(wrong_index));

    let error = engine
        .launch(&request())
        .expect_err("bad scaffold rejected");

    assert!(
        matches!(error, Error::PreflightLaunch(message) if message.contains("skills/skills.nota"))
    );
}
