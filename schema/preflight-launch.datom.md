# Mentci Preflight Launch — Datom Shape

This is the fixed datom shape for API preflight output that launches a Mentci
harness session. It is a schema artifact for the prompt-to-work slice, not an
implementation of the preflight engine, terminal-cell driver, adapter, or
scaffold cache.

Datom is positional and schema-driven: all naming lives in the type, and the
text carries only the data. What each position means is said by the type
below, never by the text. The authority for this shape is the Rust type
`mentci::preflight::MentciPreflightLaunch`; this document describes it.

## Root

The preflight output is one datom value of `PreflightLaunchEnvelope`, whose
only variant is `MentciPreflightLaunch`:

    MentciPreflightLaunch.{ <scaffold> <session-identity> <persistent-session>
                            <sandbox-privacy> <stop-conditions> <constraints> }

    scaffold           : ScaffoldPointer
    session-identity   : SessionIdentity
    persistent-session : PersistentSession
    sandbox-privacy    : SandboxPrivacy
    stop-conditions    : Vector<StopCondition>
    constraints        : Vector<LaunchConstraint>

## Structures

A struct is written in braces, positionally. A single-field wrapper is still a
struct, so its text form is `{ value }`, never a bare value.

    ScaffoldPointer   { <identity> <version> <minimal-files> <minimal-context>
                        <expansion-index> <reuse-policy> }
      identity        : ScaffoldIdentity   { Text }
      version         : ScaffoldVersion    { Integer }
      minimal-files   : Vector<SourceLocator>
      minimal-context : Vector<ContextLocator>
      expansion-index : SkillIndexLocator  { Text }
      reuse-policy    : ReuseDeferred

    SessionIdentity      { <lane-name> <lane-metadata> <addressable-handle>
                           <lookup-path> }
      lane-name          : LaneName           { Text }
      lane-metadata      : Vector<LaneMetadata>
      addressable-handle : SessionHandle      { Text }
      lookup-path        : SessionLookupPath  { Text }

## Variants

A variant is a head alone when it carries nothing, and a head, a dot and a
body when it carries data.

    LaneMetadata       [ Bead.{ Text } Repo.{ Text }
                         WorkSurface.{ Text } HarnessLabel.{ Text } ]

    PersistentSession  [ Persistent Ephemeral ]
      Whether the harness session should survive the launch request. It does
      not name the session; session naming is owned by SessionIdentity.

    SandboxPrivacy     [ SandboxedJjTask.{ PrimaryScope PrivacySurface } ]
      PrimaryScope     [ PrimaryForbidden ]
      PrivacySurface   [ PrivateScopeClosed PublicSurfaceAllowed ]
      A dedicated slot carrying sandbox and privacy posture. It is not a
      generic constraint.

    StopCondition      [ IdleTimeout.{ Integer } TurnCap.{ Integer }
                         CompletionSignal ]

    LaunchConstraint   [ WorkSurface.{ Text } RequiredArtifact.{ Text }
                         ForbiddenPath.{ Text } RequiredWitness.{ Text }
                         ImplementationBoundary.{ Text } ]

`LaunchConstraint` is intentionally residual and closed. It carries only
constraints that do not already have a first-class slot. Session identity,
persistent-session request, sandbox/privacy posture, scaffold
identity/version, and stop conditions stay in their named positions.

## Required Slot Rules

- `SessionIdentity` is always present and is distinct from
  `PersistentSession`.
- `SandboxPrivacy` is always present and may not be represented as a
  `LaunchConstraint`.
- `StopCondition` is a closed typed variant set. A bare timeout number is not
  a stop condition.
- `ScaffoldPointer` always carries `ScaffoldIdentity` and `ScaffoldVersion`.
  `ReuseDeferred` records that scaffold reuse and caching mechanics are not
  part of this slice.
- `ScaffoldPointer.expansion-index` is `skills/skills.dotos`. The scaffold
  remains minimal; agents load further skills and repo context from that
  index.
- Integers are datom integers: signed, 64-bit. `ScaffoldVersion` must be
  positive.

## Adapter And Model Boundary

This guidance adds no positions. The Mentci preflight front door may use a
semantic preflight model profile to produce this packet, but the packet itself
carries no model profile, provider name, adapter identity, terminal-cell
driver identity, command-line argument, or permission policy. Those are
adapter and session launch-plan details below this shape.

If an adapter cannot map its own semantic profile to a locally available
provider model or terminal mode, it fails launch-plan construction with a
typed adapter diagnostic instead of teaching Mentci the provider's model
roster.

## Canonical Example

This is the exact text the type projects, and the fixture that
`tests/preflight.rs` actualizes line by line:

    MentciPreflightLaunch.{
      { { mentci-prompt-scaffold } { 1 } [ { skills/skills.dotos } ] [ { ARCHITECTURE.md } ] { skills/skills.dotos } ReuseDeferred }
      { { mentci-primary-k6va } [ Bead.{ primary-k6va } WorkSurface.{ sandboxed-jj-task } ] { primary-k6va-session } { orchestrate/lanes/primary-k6va } }
      Persistent
      SandboxedJjTask.{ PrimaryForbidden PrivateScopeClosed }
      [ IdleTimeout.{ 600 } TurnCap.{ 8 } CompletionSignal ]
      [ WorkSurface.{ sandboxed-jj-task } ForbiddenPath.{ /home/li/primary } ]
    }

## Invalid Compression Forms

These are refused because they hide first-class positions inside generic
constraints, or collapse typed variants into free text. Datom refuses them at
composition, by arity and by variant head, before any validation rule runs:

    ;; Invalid: session identity and privacy swallowed by constraints.
    MentciPreflightLaunch.{ <scaffold> [] [ Constraint.{ session } ] }

    ;; Invalid: a bare number is not a typed stop condition.
    MentciPreflightLaunch.{ <scaffold> <session-identity> <persistent-session> <sandbox-privacy> 600 [] }

    ;; Invalid: provider and adapter launch-plan details sit below this packet.
    MentciPreflightLaunch.{ <scaffold> { { cheap-contained-preflight } { cheap-harness-session } } <session-identity> <persistent-session> <sandbox-privacy> <stop-conditions> [] }
