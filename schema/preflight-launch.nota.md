# Mentci Preflight Launch NOTA Schema

This is the fixed NOTA contract for API preflight output that launches a
Mentci harness session. It is a schema artifact for the prompt-to-work slice,
not an implementation of the preflight engine, terminal-cell driver, adapter,
or scaffold cache.

The preflight output is one positional NOTA record:

```nota
(MentciPreflightLaunch <scaffold> <route> <session-identity> <persistent-session> <sandbox-privacy> <stop-conditions> <constraints>)
;;   scaffold           : (ScaffoldPointer <identity> <version> <minimal-files> <minimal-context> <expansion-index> <reuse-policy>)
;;   route              : (RouteMetadata <chosen-skills> <model-selection> <harness-target> <routing-rationale>)
;;   session-identity   : (SessionIdentity <lane-name> <lane-metadata> <addressable-handle> <lookup-path>)
;;   persistent-session : Persistent | Ephemeral
;;   sandbox-privacy    : (SandboxedJjTask <primary-scope> <privacy-surface>)
;;   stop-conditions    : [(IdleTimeout <duration>) | (TurnCap <turn-count>) | CompletionSignal]
;;   constraints        : [LaunchConstraint]
```

## Records

```nota
(ScaffoldPointer <identity> <version> <minimal-files> <minimal-context> <expansion-index> <reuse-policy>)
;;   identity        : ScaffoldIdentity
;;   version         : ScaffoldVersion
;;   minimal-files   : [SourceLocator]
;;   minimal-context : [ContextLocator]
;;   expansion-index : SkillIndexLocator
;;   reuse-policy    : ReuseDeferred

(RouteMetadata <chosen-skills> <model-selection> <harness-target> <routing-rationale>)
;;   chosen-skills     : [(ChosenSkill <skill-name> <skill-source> <load-reason>)]
;;   model-selection   : (ModelSelection <preflight-model> <harness-session-model>)
;;   harness-target    : (HarnessTarget <harness-kind> <adapter> <terminal-cell-driver>)
;;   routing-rationale : RoutingRationale

(ChosenSkill <skill-name> <skill-source> <load-reason>)
;;   skill-name   : SkillName
;;   skill-source : SourceLocator
;;   load-reason  : LoadReason

(ModelSelection <preflight-model> <harness-session-model>)
;;   preflight-model        : PreflightModelProfile
;;   harness-session-model  : HarnessSessionModelProfile
;;   These are unpinned semantic knobs. Concrete provider model identifiers
;;   are selected later, outside this schema.

HarnessTarget [
  (ClaudeCode AdapterDriver)
  (Codex AdapterDriver)
  (Pi AdapterDriver)
  (OpenEndedHarness AdapterDriver)
]

(AdapterDriver <adapter> <terminal-cell-driver>)
;;   adapter              : AdapterIdentity
;;   terminal-cell-driver : TerminalCellDriverIdentity

(SessionIdentity <lane-name> <lane-metadata> <addressable-handle> <lookup-path>)
;;   lane-name          : LaneName
;;   lane-metadata      : [LaneMetadata]
;;   addressable-handle : SessionHandle
;;   lookup-path        : SessionLookupPath

(LaneMetadata <metadata-kind> <metadata-value>)
;;   metadata-kind  : Bead | Repo | WorkSurface | HarnessLabel
;;   metadata-value : MetadataValue

PersistentSession [Persistent Ephemeral]
;;   This slot says whether the harness session should survive the launch
;;   request. It does not name the session; session naming is owned by
;;   SessionIdentity.

SandboxPrivacy [(SandboxedJjTask PrimaryScope PrivacySurface)]
;;   primary-scope    : PrimaryForbidden
;;   privacy-surface  : PrivateScopeClosed | PublicSurfaceAllowed
;;   This dedicated slot carries sandbox and privacy posture. It is not a
;;   generic constraint.
```

## Closed Variant Sets

```nota
StopCondition [(IdleTimeout Duration) (TurnCap TurnCount) CompletionSignal]

LaunchConstraint [
  (WorkSurface WorkSurface)
  (RequiredArtifact SourceLocator)
  (ForbiddenPath Path)
  (RequiredWitness WitnessName)
  (ImplementationBoundary BoundaryName)
]
```

`LaunchConstraint` is intentionally residual and closed. It carries only
constraints that do not already have a first-class slot. Session identity,
persistent-session request, sandbox/privacy posture, model selection, harness
target, scaffold identity/version, chosen skills, and stop conditions must stay
in their named records.

## Required Slot Rules

- `SessionIdentity` is always present and is distinct from
  `PersistentSession`.
- `SandboxPrivacy` is always present and may not be represented as a
  `LaunchConstraint`.
- `StopCondition` is a closed typed enum. A bare timeout number or text field is
  not valid.
- `ModelSelection` always carries two separate unpinned knobs:
  `PreflightModelProfile` and `HarnessSessionModelProfile`.
- `ScaffoldPointer` always carries `ScaffoldIdentity` and `ScaffoldVersion`.
  `ReuseDeferred` records that scaffold reuse and caching mechanics are not part
  of this slice.
- `ScaffoldPointer.expansion-index` is `skills/skills.nota`. The scaffold
  remains minimal; agents load further skills and repo context from that index.

## Canonical Example

```nota
(MentciPreflightLaunch
  (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred)
  ([(beads skills/beads.md [claim and update the bead])
    (nota-design skills/nota-design.md [preserve positional NOTA shape])]
   (cheap-contained-preflight cheap-harness-session)
   (Codex codex-terminal-adapter terminal-cell-v1)
   [Prompt requires a sandboxed jj task and a persistent named harness session])
  (mentci-primary-swvx [(Bead primary-swvx) (WorkSurface sandboxed-jj-task)] primary-swvx-session orchestrate/lanes/primary-swvx)
  Persistent
  (SandboxedJjTask PrimaryForbidden PrivateScopeClosed)
  [(IdleTimeout 600) (TurnCap 8) CompletionSignal]
  [(WorkSurface sandboxed-jj-task) (ForbiddenPath /home/li/primary)])
```

## Invalid Compression Forms

These forms are invalid because they hide first-class slots inside generic
constraints or collapse typed variants into free text:

```nota
;; Invalid: session identity and privacy are swallowed by constraints.
(MentciPreflightLaunch <scaffold> <route> [] [(Constraint [session primary-swvx]) (Constraint [private])])

;; Invalid: timeout is not a typed stop condition.
(MentciPreflightLaunch <scaffold> <route> <session-identity> <persistent-session> <sandbox-privacy> 600 [])

;; Invalid: one generic model string collapses the two unpinned model knobs.
(ModelSelection cheap-model)
```
