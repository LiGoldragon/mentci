# Mentci Preflight Launch NOTA Schema

This is the fixed NOTA contract for API preflight output that launches a
Mentci harness session. It is a schema artifact for the prompt-to-work slice,
not an implementation of the preflight engine, terminal-cell driver, adapter,
or scaffold cache.

The preflight output is one positional NOTA record:

```nota
(MentciPreflightLaunch <scaffold> <session-identity> <persistent-session> <sandbox-privacy> <stop-conditions> <constraints>)
;;   scaffold           : (ScaffoldPointer <identity> <version> <minimal-files> <minimal-context> <expansion-index> <reuse-policy>)
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
persistent-session request, sandbox/privacy posture, scaffold identity/version,
and stop conditions must stay in their named records.

## Required Slot Rules

- `SessionIdentity` is always present and is distinct from
  `PersistentSession`.
- `SandboxPrivacy` is always present and may not be represented as a
  `LaunchConstraint`.
- `StopCondition` is a closed typed enum. A bare timeout number or text field is
  not valid.
- `ScaffoldPointer` always carries `ScaffoldIdentity` and `ScaffoldVersion`.
  `ReuseDeferred` records that scaffold reuse and caching mechanics are not part
  of this slice.
- `ScaffoldPointer.expansion-index` is `skills/skills.nota`. The scaffold
  remains minimal; agents load further skills and repo context from that index.

## Adapter And Model Boundary

This guidance does not add schema fields. The Mentci preflight front door may
use a semantic preflight model profile to produce this packet, but the packet
itself carries no model profile, provider name, adapter identity, terminal-cell
driver identity, command-line argument, readiness phrase, or permission policy.
Those are adapter/session launch-plan details below this schema.

If an adapter cannot map its own semantic profile to a locally available
provider model or terminal mode, it fails launch-plan construction with a typed
adapter diagnostic instead of teaching Mentci the provider's model roster.

## Canonical Example

```nota
(MentciPreflightLaunch
  (mentci-prompt-scaffold 1 [skills/skills.nota] [ARCHITECTURE.md] skills/skills.nota ReuseDeferred)
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
(MentciPreflightLaunch <scaffold> [] [(Constraint [session primary-swvx]) (Constraint [private])])

;; Invalid: timeout is not a typed stop condition.
(MentciPreflightLaunch <scaffold> <session-identity> <persistent-session> <sandbox-privacy> 600 [])

;; Invalid: provider and adapter launch-plan details are below this packet.
(MentciPreflightLaunch <scaffold> (ProviderAdapter provider-terminal-adapter terminal-cell-v1) <session-identity> <persistent-session> <sandbox-privacy> <stop-conditions> [])
```
