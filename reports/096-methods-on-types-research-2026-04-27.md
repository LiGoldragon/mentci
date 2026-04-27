# 096 — Methods on types, not free functions: why the rule forces correct thinking

*Deep research synthesis on why the
[`rust/style.md` §"Methods on types"](../repos/tools-documentation/rust/style.md)
rule is load-bearing — for human engineers, for the type system,
and especially for LLM-agent code generators. Companion to
[`092`](092-naming-research-and-rule.md), which did the same
job for the naming rule.*

The rule itself, in one line: **every reusable verb belongs to
a noun**. If you can't name the noun, you haven't found the
right model yet — keep looking until you can.

This report defends that rule across four traditions:
cognitive-science / foundational CS (§2), industrial /
practitioner experience (§3), Rust-specific type-system design
(§4), and the LLM-agent code-gen failure mode that the rule was
sharpened against in this project (§5). §6 names the principled
exceptions. §7 returns to the practical formulation.

---

## 1 · The thesis in one paragraph

Methods encode **affordances** — what kinds of things a value
of a given type *can do*. Free functions encode **operations**
that happen to take some arguments. The distinction is not
aesthetic; it is structural. Behavior bound to a type is
discoverable through the type, constrained by the type, and
preserved across refactors of the type. Behavior floating as a
free function is dispatched by argument list, not by ownership;
it has no native home, no privacy contract, and no guarantee
that the type system can track it. The rule "behavior lives on
types" is what makes a Rust codebase carry meaning in its
shape, not just in its commit messages.

The rule's deepest consequence is what it forces the writer to
do *before* writing the verb: it forces the writer to find or
invent the noun. That forcing function is the cognitive lever
that the rest of this report defends.

---

## 2 · The cognitive and foundational case

### 2.1 Naur — programming as theory building

Peter Naur's 1985 essay *Programming as Theory Building*
displaced source code from the centre of software work. The
artifact that matters, he argued, is the programmer's *theory*
of the system — the structured understanding that lets her
"explain [the program], answer queries about [it], argue about
[it]." Source text is residue; documentation is residue; the
live theory in working memory is the real software.

For a theory to cohere, it needs an index. Methods-on-types
provide one: to learn what a `File` *is*, in such a regime,
you read what a `File` *does*. The theory of the system
fragments naturally into theories of its types. In a
free-function regime, the theory has no native shape; every
new reader must reconstruct it from a procedural soup that
respects no boundaries. The cost of onboarding is the cost of
re-deriving the theory from scratch.

### 2.2 Liskov — clusters as the unit of organisation

Barbara Liskov made this organisational claim concrete with
CLU's *cluster* — types and their operations bundled together
in a single syntactic unit. The 1974 Liskov–Zilles paper
introduced abstract data types specifically as an answer to
the question "where do verbs live?" — and the cluster forced
the answer "with the type." Liskov's 2008 Turing lecture, *The
Power of Abstraction*, summarises the position: "abstraction
is the basis for program construction, allowing programs to be
built in a modular fashion."

The CLU cluster is the direct ancestor of Rust's `impl X { … }`
block. Both make the same structural commitment: the type and
its operations live in one place; the privacy boundary cuts
between the cluster's interior and the rest of the program;
adding a verb means deciding (or admitting) the type that owns
it.

### 2.3 Parnas — information hiding as the criterion

David Parnas's 1972 *On the Criteria to Be Used in Decomposing
Systems into Modules* gave the deeper criterion: "Every module
in the second decomposition is characterized by its knowledge
of a design decision which it hides from all others." When
operations are decoupled from data — Parnas's "first
decomposition," organised by flowchart steps — many small
changes touch many modules, because the *secret* (the data
layout) is splattered across procedures that all assume it.

Methods-on-types is information hiding made grammatical: the
only way to manipulate the data is through the operations the
type itself publishes. The discipline names what each module's
secret is, and then defends it.

### 2.4 Kay — cells, membranes, messages

Alan Kay's 2003 clarification ("OOP to me means only
messaging, local retention and protection and hiding of
state-process, and extreme late-binding of all things") and
his earlier biological imagery — objects as cells "only able
to communicate with messages" — locate the discipline in *what
an object can be asked to do*. The cell metaphor is not
decorative: a cell's membrane defines its affordances. A free
function is not a membrane. It is a verb floating in a
procedural void, accepting any record that structurally
conforms.

### 2.5 Brooks — accidental complexity removed

Brooks's 1986 *No Silver Bullet* divided software difficulty
"following Aristotle … into essence, the difficulties inherent
in the nature of software, and accidents, those difficulties
that today attend its production but are not inherent."
Methods-on-types removes one accidental degree of freedom —
*where does this verb live?* — by answering it structurally.
The reader no longer has to scan the codebase looking for
operations that act on a value of type `T`; they read `T`'s
methods.

The savings compound. Cognitive-load research on program
comprehension (Soloway & Ehrlich's "plans" theory; Letovsky's
mental-model studies) consistently shows that expert
programmers comprehend code by recognising chunks against
schemas in long-term memory. Each discipline that gives the
program a stable, predictable shape lowers the chunk-
recognition cost. Methods-on-types *is* a chunking convention:
the type is the chunk boundary.

### 2.6 Gibson and Norman — affordances

The cleanest cognitive vocabulary comes from outside CS.
James Gibson's 1979 *The Ecological Approach to Visual
Perception* defined an *affordance* as "what [the environment]
offers the animal, what it provides or furnishes, either for
good or ill." Donald Norman's 1988 *Design of Everyday Things*
adapted the concept to artefacts: a door's handle affords
pulling; a flat panel affords pushing; a chair affords
sitting. The affordance is a property of the relationship
between the object and the agent.

A method-bearing type *advertises* its affordances at every
call site. A passive record next to a free-function library
does not. The fruit affords eating; the cloud does not. The
type system knows which is which only when the operations are
attached to the things that own them.

This is the seed of the rule's most-quoted line: "in the real
world, fruits can be eaten and clouds cannot; code that models
the world correctly says `fruit.eat()`, not `eat(fruit)`."

---

## 3 · The industrial and empirical case

### 3.1 Tell, Don't Ask

The canonical industrial formulation is Hunt and Thomas's
adaptation of Alec Sharp's slogan **"Tell, Don't Ask."**
Martin Fowler attributes the principle to "Andy Hunt and 'Prag'
Dave Thomas" and summarises it as a reminder that
"object-orientation is about bundling data with the functions
that operate on that data" — rather than asking an object for
data and then acting on that data, "we should instead tell an
object what to do."

The failure mode is well-named in Fowler's *Refactoring*
catalogue. **Feature Envy** is "a method that seems more
interested in a class other than the one it is in"; the
prescribed cure is *Move Function* "to get it [there]." Its
sibling **Data Class** — a struct with no behaviour — is the
same drift seen from the other side. Its scaled-up form is the
**Anemic Domain Model**, which Fowler calls "contrary to the
basic idea of object-oriented design." When behaviour floats
free, a single conceptual change forces **Shotgun Surgery**:
"you want to change one thing and it ends up you have to make
a lot of additional changes … all over the place."

Every named smell here points the same direction: behaviour
that has drifted off its owning type, with the consequent
maintenance tax.

### 3.2 Feathers — legacy code and seams

Michael Feathers's 2004 *Working Effectively with Legacy Code*
defines "legacy code" as "simply code without tests," and the
structural corollary is that legacy systems are dominated by
procedural code where data and behaviour have drifted apart.
Introducing a **seam** — "a place where you can alter behaviour
in your program without editing in that place" — is
dramatically harder when behaviour lives in free functions,
because there is no type whose dispatch you can substitute.
Polymorphic seams are the most flexible kind, and they
presuppose methods-on-types.

### 3.3 Linus — methods-on-types in C, by hand

Linus Torvalds is sometimes invoked against OO ("Bad
programmers worry about the code. Good programmers worry about
data structures and their relationships," 2006). But the Linux
kernel itself is *not* a free-function codebase: as Neil Brown
documents in LWN, the kernel is full of "vtables" — `struct
file_operations`, `struct inode_operations`, `struct
backlight_ops` — methods-on-types hand-rolled in C. The
convention is even codified in the suffix `_operations`.
Behaviour is attached to the type; only the dispatch is
manual.

What Linus rejects is *bad* OO — class hierarchies as
decoration, virtual calls hiding the data layout. The kernel's
own discipline is exactly methods-on-types, given the only
syntactic affordances C provides for it.

### 3.4 Carmack — inlining vs abstraction, properly read

John Carmack is widely cited as an "abstraction is overhead"
voice. His 2007 essay (republished in 2014 by Jonathan Blow)
argues that "the function that is least likely to cause a
problem is one that doesn't exist," and that "code duplication
is a greater evil than whatever second-order problems arise
from functions being called in different circumstances."

But by 2012, in his *Gamasutra* essay on functional programming
in C++, Carmack reframes: "A large fraction of the flaws in
software development are due to programmers not fully
understanding all the possible states their code may execute
in." His 2014 commentary identifies the real enemy as
"unexpected dependency and mutation of state." That target —
*state confusion* — is what methods-on-types address by
encapsulating state behind a typed surface. Carmack's argument
against premature abstraction is not an argument for free
functions; it is an argument for clarity about ownership.

### 3.5 Acton — data-oriented design as a different concern

Mike Acton's 2014 CppCon keynote *Data-Oriented Design and C++*
is the strongest principled critique of OO design: "The purpose
of all programs, and all parts of those programs, is to
transform data from one form to another. If you don't
understand the data, you don't understand the problem."

DOD attacks designing-around-objects for *cache-layout*
reasons, not for cognitive ones. A cache-friendly
Struct-of-Arrays system can still attach its operations to a
*system type*; the rule "behaviour lives with its owning
data" is preserved at the system level, not the row level.
DOD and methods-on-types are about different scales of the
same data; they don't conflict.

### 3.6 Cook — ADTs vs objects as duals

William Cook's 2009 OOPSLA essay *On Understanding Data
Abstraction, Revisited* gives the cleanest theoretical frame
for both the rule and its principled exceptions: "objects and
abstract data types are not the same thing, and neither one is
a variation of the other. They are fundamentally different and
in many ways complementary, in that the strengths of one are
the weaknesses of the other."

In Cook's taxonomy:

- **ADTs** facilitate adding new *operations* over a fixed
  representation. Functions live outside the data; the data is
  pattern-matched.
- **Objects** facilitate adding new *representations* under a
  fixed interface. Functions live inside, dispatched on the
  receiver.

Rust uniquely sits on *both* axes. `enum` plus pattern matching
is pure ADT; `trait` objects and trait-bounded generics are
object-style abstraction. The methods-on-types rule says: even
when you're using ADTs (e.g., `enum AssertOp`), attach the
operations as inherent methods on the enum, not as free fns —
because the *receiver-dot-method* shape is what lets the rest
of the system (privacy, auto-deref, trait coherence, IDE
discovery) work.

### 3.7 Go — when the discipline is forfeited

The Go ecosystem provides a contemporary case study. Go also
supports methods-on-types without inheritance, but its tooling
makes free functions in packages so easy that long-lived Go
codebases drift into `helpers.go`, `util.go`, `common.go`. The
official Go package-naming guidance attacks the pattern
explicitly: "Packages named `util`, `common`, or `misc`
provide clients with no sense of what the package contains …
Over time, they accumulate dependencies that can make
compilation significantly and unnecessarily slower."

The same pathology recurs in long-lived C codebases as
`helpers.c`, `misc.c`, `support.c` — exactly the gravitational
sink that methods-on-types prevents by giving every function a
type-shaped home.

---

## 4 · The Rust-specific case

### 4.1 Methods without inheritance

Rust deliberately rejects both the Java pole
(classes-with-inheritance) and the Haskell pole (free
functions dispatched by type-class constraint). It lands on a
third shape: behaviour bundled with types via `impl` blocks,
but without subtyping. Aaron Turon's 2015 launch post for the
trait system describes traits as Rust's "sole notion of
interface," noting that "unlike interfaces in languages like
Java, C# or Scala, new traits can be implemented for existing
types." Methods that aren't tied to any abstract interface
live in *inherent* `impl` blocks.

The split between inherent impls and trait impls is the design
fact: Rust gives you the syntactic affordance of methods
(`x.foo()`, autoborrow, IDE discoverability) for both ad-hoc
and abstract behaviour, and reserves traits for the genuinely
polymorphic case.

### 4.2 Coherence and the orphan rule

Rust's coherence story is what keeps the methods-on-types
discipline honest. Niko Matsakis frames the goal as "given a
trait and some set of types for its type parameters, there
should be exactly one impl that applies." The *orphan rule* —
"either the trait must be local or the self-type must be
local" — pushes behaviour to where it belongs: the crate that
defined the trait, or the crate that defined the type.

RFC 1023 makes the underlying tradeoff explicit: "the ability
to define impls is a zero-sum game: every impl that is legal
to add in a child crate is also an impl that a parent crate
cannot add without fear of breaking downstream crates." This
is a deliberate ergonomic cost paid for a cohesion property:
the rule that *only certain crates can extend a type* keeps
behaviour from drifting arbitrarily across the dependency
graph.

### 4.3 Privacy reinforces the same direction

Rust struct fields are private by default. The combination of
private fields plus inherent methods means the type owns
*both* its representation and its operations. The same rule
that says `pub struct Slot(pub u64)` is wrong (because
`Slot.0` can be read and written by anyone) is just the
methods-on-types rule applied to data: the value is constructed
through `Slot::new` and read through `Slot::value`, not by
poking the field directly.

This is why both rules in the project's style guide — "wrapped
field is private" and "methods on types, not free functions" —
land in the same section. They are two faces of one
discipline.

### 4.4 Why not Haskell's free-fn-with-constraint dispatch?

Wadler & Blott's 1989 *How to make ad-hoc polymorphism less
ad hoc* gave us `Eq a => a -> a -> Bool`, conceptually a method
on `a` written as a top-level function. Rust could have copied
this; it didn't. The Rust API guidelines' C-METHOD rule reads:
"Prefer `impl Foo { pub fn frob(&self, w: widget) }` over
`pub fn frob(foo: &Foo, w: widget)` for any operation that is
clearly associated with a particular type." The reasons given
are concrete: methods "do not need to be imported or
qualified," "their invocation performs autoborrowing," and
"they make it easy to answer the question 'what can I do with
a value of type T'."

The last point is the discoverability argument. The free-fn
form scatters the answer across the entire universe of
imports.

### 4.5 The extension-trait safety valve

When you genuinely need to add a method to a type you don't
own, Rust's idiomatic answer is the *extension trait* (RFC
0445, convention `FooExt`). `itertools::Itertools` is the
canonical example, adding ~170 methods to anything implementing
`Iterator` via a blanket impl.

This is the giveaway: even when method-syntax requires more
ceremony than a free function would, the ecosystem chooses
ceremony over the free-function form. The reason is exactly
the discoverability argument from C-METHOD — even a costly
trait import is preferred over a free-fn import, because the
result is the dot-syntax that lets future readers ask "what
can I do with a `T`?" and get an answer from the type alone.

---

## 5 · The LLM agent code-gen case

This is the slice where the literature is thin and the
argument has to do work. The empirical work on LLM-generated
code documents structural defects (Tambon et al. 2024 found
LLM output is "shorter yet more complicated" than canonical
solutions, with "misunderstanding and logic errors" as the
largest bug category; Spinellis et al. 2025 found 33.7% of
LLM-generated JS contains "unused code segments" and 83.4% of
Python shows "invalid naming conventions"), but it does not
name the underlying cause: **verbs without owning nouns**.
Naming conventions go bad because there is no type to anchor a
name to; unused code accumulates because nothing carries a
clean responsibility.

### 5.1 The Fowler/Joshi diagnosis

The most direct primary source is Martin Fowler and Unmesh
Joshi's 2024 *Conversation: LLMs and Building Abstractions*:
"discovering and stabilizing abstractions … cannot be reduced
to a static prompt for an LLM" and "reviewing LLM-generated
code is rarely enough — you miss the deep thinking that
happens when you are coding yourself." The framing is that
LLMs *apply* abstractions competently once they exist, but
*discovering* the right abstraction is the human's job.

The methods-on-types rule pushes that discovery work back into
the loop. By demanding `QueryParser::into_query` instead of
`parse_query`, the rule forces the agent to enact the
noun-creation step that Fowler/Joshi say cannot be skipped.

### 5.2 Why LLMs need this rule more than humans do

This is the load-bearing original argument; flagged as such.

Humans procrastinate creating types because typing out
`struct QueryParser { … }` *feels heavier* than `fn
parse_query(…)` — there is tactile friction in declaring a
noun, naming its fields, deciding its constructor. That
friction is a feature: it makes humans ask "is this type
pulling its weight?" before paying the cost.

LLMs have no such friction. Generating `struct QueryParser`
and generating `fn parse_query` cost the same number of
tokens, take the same wall-clock time, and produce no felt
sense of "this is heavy." The result, predictable from first
principles and consistent with the maintainability data, is
that LLMs default to whichever shape is *shorter* — almost
always the free function.

The methods-on-types rule reintroduces, by fiat in a style
guide, the friction the substrate has erased. It is a Sapir-
Whorf-style intervention on agent cognition: the rule changes
what the agent can think, by changing what it is *required* to
write.

### 5.3 The concrete case from this project

The project has direct evidence. When the QueryParser was
first drafted in the nexus crate, the initial form was:

```rust
fn parse_query(text: &str) -> Result<QueryOp, Error> { … }
```

A free function. The shape *let the parser get away with* not
naming itself as a noun. There was no `QueryParser` type. The
parser's state (input cursor, dialect, lexer position, error
context) lived as locals inside a single function body. Any
future need — preserving parser state across calls, returning
richer diagnostics with line/column info, stream-mode parsing
— would have required the `QueryParser` type to be invented
*later*, retroactively, against the grain of the existing
code.

Li caught the pattern, pointed at the rule, and the refactor
became:

```rust
pub struct QueryParser<'input> { lexer: Lexer<'input> }

impl<'input> QueryParser<'input> {
    pub fn new(input: &'input str) -> Self { … }
    pub fn into_query(self) -> Result<QueryOp, Error> { … }
}
```

The type now carries weight. It owns the lexer, names the
parser as a noun, and provides an obvious place to attach
future capabilities (line/column tracking, recovery state, …)
without restructuring the call graph. The fix took ten
minutes; the absence of the type would have cost much more
later.

This is the rule doing its job.

### 5.4 The Karpathy nuance

Andrej Karpathy's 2026 distilled-into-CLAUDE.md observations
identify failure modes including "agents over-complicate code
and APIs with bloated abstractions" and prescribe "no
abstractions for single-use code." This *appears* to point the
opposite direction from "create more types," and the
reconciliation is the load-bearing nuance.

Karpathy is rejecting *speculative* abstraction — interfaces
invented for hypothetical second callers, generic trait
hierarchies that nothing concrete needs. The methods-on-types
rule is about *naming an abstraction that already exists
implicitly*. A `parse_query` free function already has a
`QueryParser` inside it: the parser state, the input cursor,
the error context. The rule just makes the existing
abstraction wear its name.

The two pieces of advice converge: don't invent unused types;
do name the types that are already there.

### 5.5 The agent-API parallel

Anthropic's tool-use API, LangChain's `bind_tools()`, and
AutoGen's Pydantic-typed tool registration all model agent
capabilities as **methods on a typed world object** — typed
inputs, typed outputs, named verbs dispatched on a typed
surface. The Anthropic engineering blog explicitly frames this
as affordance design: "agents have distinct 'affordances' to
traditional software."

If affordance-typing is how we make agents reason well *across
the API boundary*, methods-on-types is how we make agents
reason well *across the function boundary inside the code they
write*. It is the same discipline, one fractal level down.

### 5.6 The Karlton bridge

Phil Karlton's old saw — "There are only two hard things in
Computer Science: cache invalidation and naming things" — gets
its sharpest LLM reading here. **When an LLM agent skips
creating a type, it skips the naming step entirely.** The hard
thing is not avoided; it is hidden. The methods-on-types rule
restores the hard step into the workflow, where it belongs.

This is the cleanest one-line statement of the rule's purpose:
*the rule exists to make sure naming happens.*

---

## 6 · The principled exceptions

The rule has carve-outs, named directly:

### 6.1 The local-helper carve-out

The style guide already says: "A small private helper inside
one module is fine if it is genuinely local." A three-line
`fn hex(h: &Hash) -> String` next to a single Display impl is
not a missing noun; it is a private fragment of one impl. The
rule kicks in when the verb is *reusable* — when more than one
caller might want it, when it would be discoverable from
multiple sites, when its life as a free function would let it
spread.

### 6.2 The ADT-axis carve-out (Cook 2009)

Cook's distinction makes clear that some operations live
properly outside the type. Pure mathematical operations like
`add(a, b)` over numbers fit the ADT axis: there is no
privileged owning type, no encapsulated state, no
representation to hide. Haskell-style top-level definitions are
appropriate here. In Rust this is what `trait Add` already
does — implemented by both operands' types, but *invoked* via
`a + b` rather than `a.add(b)`. The dispatch is on the type
structurally; the operator syntax is the syntactic sugar for
the methods-on-types form.

The general principle: when an operation is genuinely
**relational** between two values of equal status, with no
state on either side, the free-function form (or its operator-
overload sugar) is principled. This is rare in practice.
Most "operations on data" actually have an asymmetric owner.

### 6.3 The serde-ecosystem carve-out

`serde_json::from_str` / `serde_json::to_string` are free
functions because the ecosystem convention demands them. A
serde-format crate that hides this convention behind methods-
on-`Deserializer` would surprise every user who has ever
reached for `serde_json`. This is exception 5 from the naming
rule — names inherited from well-known libraries — applied to
function shape.

The carve-out is **narrow**: the crate-root `from_str` /
`to_string` shape is preserved; everything inside the crate's
own implementation should still attach behaviour to its
owning types (a `Deserializer` type with methods, not a soup
of free helpers).

### 6.4 The Hickey / Clojure case

Rich Hickey's *Simple Made Easy* and the broader Clojure
tradition explicitly favour values-plus-functions over open
data ("the majority of functions in Clojure just take data"),
and the discipline works for human Clojure programmers because
they carry a *very strong tacit data-shape discipline* —
they hold the schema of the map in their heads.

LLMs do not reliably hold schemas across edits; they re-derive
them from textual context. A free-function-over-untyped-map
style gives the LLM nothing to anchor to between turns, which
is precisely the failure mode the maintainability surveys
describe. Hickey's pattern is right for humans with strong
tacit models; it is wrong as a default for LLM agents because
the tacit model is not durable across context windows.

This is why the rule is in our style guide and not in the
Clojure community's: we need it more.

---

## 7 · How the rule reads in this codebase

The practical formulation in
[`rust/style.md`](../repos/tools-documentation/rust/style.md):

> The only free function in a binary crate is `main`. Reusable
> behavior is a method on a type or a trait impl. Test helpers
> are methods on a fixture struct.
>
> A small private helper inside one module is fine if it is
> genuinely local. Anything that smells reusable becomes a
> method.
>
> The rule of thumb: **every reusable verb belongs to a noun**.
> If you can't name the noun, you haven't found the right
> model yet — keep looking until you can.

The rule's role in the project's daily practice:

- It is the cognitive forcing function that turns a draft into
  a properly-typed model. A free-function draft is acceptable
  as a sketch; a free-function commit usually isn't.
- It pairs directly with the **wrapped-field-is-private** rule
  — both push the type to own its representation *and* its
  operations.
- It pairs with **perfect specificity** (criome
  ARCHITECTURE.md §2 Invariant D) — every typed boundary
  names exactly what flows through it. Methods-on-types extend
  the same discipline inward: every typed verb is dispatched
  on the type that owns it.
- It is especially load-bearing for LLM-generated code, which
  lacks the tactile friction that makes humans economize on
  type creation.

When in doubt: ask "what type owns this verb?" If the answer
isn't obvious, that is the design problem to solve before
writing the verb. The rule's discomfort is the signal that
something structural is missing.

---

## 8 · Citations

### Cognitive / foundational

- Naur, P. (1985). *Programming as Theory Building.*
  *Microprocessing and Microprogramming*, 15(5).
  https://pages.cs.wisc.edu/~remzi/Naur.pdf
- Liskov, B. & Zilles, S. (1974). *Programming with Abstract
  Data Types.* *ACM SIGPLAN Notices*, 9(4).
  https://dl.acm.org/doi/10.1145/942572.807045
- Liskov, B. (2008). *The Power of Abstraction.* Turing Award
  Lecture. http://www.pmg.csail.mit.edu/~liskov/turing-09-5.pdf
- Parnas, D. L. (1972). *On the Criteria to Be Used in
  Decomposing Systems into Modules.* *CACM*, 15(12).
  http://sunnyday.mit.edu/16.355/parnas-criteria.html
- Kay, A. (1993). *The Early History of Smalltalk.* HOPL-II.
  https://worrydream.com/EarlyHistoryOfSmalltalk/
- Kay, A. (2003). Email to Stefan Ram, 23 July 2003.
  http://userpage.fu-berlin.de/~ram/pub/pub_jf47ht81Ht/doc_kay_oop_en
- Brooks, F. P. (1986). *No Silver Bullet — Essence and
  Accidents of Software Engineering.* *IEEE Computer*, 20(4).
  https://www.cs.unc.edu/techreports/86-020.pdf
- Soloway, E. & Ehrlich, K. (1984). *Empirical Studies of
  Programming Knowledge.* *IEEE TSE*, SE-10(5).
- Letovsky, S. (1987). *Cognitive Processes in Program
  Comprehension.* *J. Sys. Soft.*, 7(4).
- Gibson, J. J. (1979). *The Ecological Approach to Visual
  Perception.* Houghton Mifflin.
- Norman, D. A. (1988). *The Design of Everyday Things.* Basic
  Books.

### Industrial / practitioner

- Hunt, A. & Thomas, D. (2003). *The Art of Enbugging.*
  *IEEE Software.*
  https://media.pragprog.com/articles/may_04_oo1.pdf
- Fowler, M. (2013). *TellDontAsk.* martinfowler.com bliki.
  https://martinfowler.com/bliki/TellDontAsk.html
- Sharp, A. (1997). *Smalltalk by Example: The Developer's
  Guide.* McGraw-Hill.
- Fowler, M. (1999, 2nd ed. 2018). *Refactoring: Improving the
  Design of Existing Code.* Addison-Wesley.
- Fowler, M. (2003). *AnemicDomainModel.*
  https://martinfowler.com/bliki/AnemicDomainModel.html
- Feathers, M. (2004). *Working Effectively with Legacy Code.*
  Prentice Hall.
- Torvalds, L. (2006-06-27). git mailing list message.
  https://lore.kernel.org/all/Pine.LNX.4.64.0607270936200.4168@g5.osdl.org/
- Brown, N. (2011). *Object-oriented design patterns in the
  kernel.* LWN. https://lwn.net/Articles/444910/
- Carmack, J. (2007, republished 2014). *On Inlined Code.*
  http://number-none.com/blow/blog/programming/2014/09/26/carmack-on-inlined-code.html
- Carmack, J. (2012). *In-Depth: Functional Programming in
  C++.* Game Developer.
  https://www.gamedeveloper.com/programming/in-depth-functional-programming-in-c-
- Acton, M. (2014). *Data-Oriented Design and C++.* CppCon
  2014. https://www.youtube.com/watch?v=rX0ItVEVjHc
- Cook, W. R. (2009). *On Understanding Data Abstraction,
  Revisited.* OOPSLA.
  https://www.cs.utexas.edu/~wcook/Drafts/2009/essay.pdf
- Cook, W. R. (1990). *OOP vs ADTs.*
  https://www.cs.utexas.edu/~wcook/papers/OOPvsADT/CookOOPvsADT90.pdf
- Go team (2017). *Package names.* https://go.dev/blog/package-names

### Rust-specific

- Wadler, P. & Blott, S. (1989). *How to make ad-hoc
  polymorphism less ad hoc.* POPL '89.
  https://dl.acm.org/doi/10.1145/75277.75283
- Matsakis, N. (2015). RFC 1023: *Rebalancing Coherence.*
  https://rust-lang.github.io/rfcs/1023-rebalancing-coherence.html
- Matsakis, N. (2015). *Little Orphan Impls.*
  https://smallcultfollowing.com/babysteps/blog/2015/01/14/little-orphan-impls/
- Turon, A. (2015). *Abstraction without overhead: traits in
  Rust.* https://blog.rust-lang.org/2015/05/11/traits/
- Turon, A. (2017). *Specialization, coherence, and API
  evolution.*
  https://aturon.github.io/tech/2017/02/06/specialization-and-coherence/
- RFC 0445: *Extension trait conventions.*
  https://rust-lang.github.io/rfcs/0445-extension-trait-conventions.html
- Rust Project. *API Guidelines — Predictability* (C-METHOD).
  https://rust-lang.github.io/api-guidelines/predictability.html
- Rust Reference. *Visibility and Privacy.*
  https://doc.rust-lang.org/reference/visibility-and-privacy.html

### LLM / agent code-gen

- Tambon, F. et al. (2024). *What's Wrong with Your Code
  Generated by Large Language Models?* arXiv:2407.06153.
  https://arxiv.org/html/2407.06153v1
- Spinellis, D. et al. (2025). *Quality Assurance of
  LLM-generated Code.* arXiv:2511.10271.
  https://arxiv.org/html/2511.10271v1
- Fowler, M. & Joshi, U. (2024). *Conversation: LLMs and
  Building Abstractions.*
  https://martinfowler.com/articles/convo-llm-abstractions.html
- Willison, S. (2025-03-11). *Here's how I use LLMs to help me
  write code.*
  https://simonwillison.net/2025/Mar/11/using-llms-for-code/
- Anthropic Engineering (2025). *Writing Tools for Agents.*
  https://www.anthropic.com/engineering/writing-tools-for-agents
- Karpathy, A. / Chang, F. (2026). *Andrej Karpathy Skills
  CLAUDE.md.*
  https://github.com/forrestchang/andrej-karpathy-skills/blob/main/CLAUDE.md
- Hickey, R. (2011). *Simple Made Easy.* InfoQ.
  https://www.infoq.com/presentations/Simple-Made-Easy/
- Karlton, D. (2017). *Naming things is hard.*
  https://www.karlton.org/2017/12/naming-things-hard/

---

*End 096.*
