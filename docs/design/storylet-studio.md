# DecisionNarrative Storylet Studio

**Status:** Current implementation and architecture direction

## Summary

DecisionNarrative is a local, setting-neutral authoring and playtest platform for storylet-driven games. Designers author entities and traits, locations, events, scenarios, and storylet graphs. The deterministic Rust run kernel validates eligibility and applies world changes. The Vue studio exposes authoring, review, project interchange, and the Play view.

The studio has no Strata integration, text or image generation API, embedding service, learned ranker, or model-training pipeline. Storylet content is authored directly in the editor or by an external MCP agent acting on a user's request. Designers retain control of content review and approval.

## Product boundaries

- Projects own their cast, setting, typed entity properties, locations, events, and storylets.
- Authoring data and runs are local and persistent; run actions use the deterministic Rust kernel.
- AI providers are not part of the product runtime. No hidden remote or local model fallback exists.
- External agents can work directly through MCP. The agent reads the current interaction context, then uses ordinary project, content, scenario, or run operations; it does not need to automate the UI.
- Context identifies what the user is viewing or editing. It is not authorization to mutate unrelated project data.

## Architecture

### Rust engine

The host-independent Rust library owns the Chrono run kernel: participants and knowledge with provenance, the canon chronicle, space and clock, dense storylet dispatch and selection, deterministic choices and effects, and exact replay from recorded actions. The core has no HTTP, SQLite, browser, Python, or model-provider dependency.

The C ABI exposes the run kernel operations to native hosts.

### Local service

The Rust service provides the local HTTP and MCP surfaces and stores project and run data in SQLite. It validates graph and project changes, persists runs as start, seed, and actions, and rebuilds them by deterministic replay. Its MCP tool supports project inspection and mutation, storylet approval, scenario operations, runs, and coverage. It does not call generative or ranking models.

### Web studio

The Vue 3 application provides a storylet library, dual-tab graph editor, Codex, tag manager, starting scenarios, Play runs with a coverage report, and project interchange. The Story Graph authors scene flow; boolean predicates feed `And`/`Or` or the Boolean input on `Condition Node`, whose `true`/`false` branches route flow. The separate Preconditions graph evaluates read-only candidate gates before storylets enter consideration pools. Use `Get Entity`, `Get Entity Property`, `Compare`, and `Has Entity Tag` for typed entity reads and predicates; `Apply Command` mutates a typed Entity property. Location and character membership use Has Entity Tag dot-notation. There is no Quality Check node or storylet-local variable node.

### Interaction context and MCP agents

The studio publishes one local, structured snapshot of its active interaction to the local service. The snapshot contains:

- active route/view and project identity/revision;
- focused entity identity and its current editor data when available;
- selected graph node or other focused item when available;
- capture timestamp for freshness checks.

The `workspace.context` MCP operation returns that snapshot together with a freshness indication. A configured MCP agent reads it before choosing a target, then uses the existing `narrative_platform` operations directly. The MCP process and the studio share the same local project database. Context is local application state, not a model prompt service or a provider configuration.

## Authoring and validation

Designers author text, tags, Story Graph flow, Preconditions graphs, choices, and effects directly. The platform does not generate drafts; review and approval remain explicit author decisions.


A valid Story Graph has exactly one Start and one or more End nodes. Every node must be reachable from Start and able to reach an End. Each End is terminal (ends the run) or a jump to another storylet. A valid Preconditions graph uses read-only predicate/value nodes, typed property comparisons, And/Or combinators, an empty Condition Node boolean switch, and at least one valid-candidate End; a false-result End rejects the candidate. `Has Entity Tag` may be used in both graphs with mode-specific outputs. Both graphs and their references are project-validated on save and approval. Chrono operations appear as graph nodes only where supported by the runtime. A Go To Location node must be targeted directly by a Choice and refer to an existing location.
Tags are unique within their category, normalized for comparison, and shared through inline tag editing and the Tag Manager. Location tags determine where a storylet can be eligible. Membership/state tags use dot notation such as `location.market.visited` and `character.guide.met`. Typed entity properties replace a separate quality model.
## Runs and replay

A run starts from an authored scenario and a deterministic seed. The service compiles the workspace into run content, records each action, and rebuilds the run by replaying the record, so the same content revision, seed, and actions always produce the same state.

Run operations carry an expected revision and idempotency key. Stale or duplicate operations cannot partially mutate state. Content edits mark open runs stale; reloading replays the record on the new content and reports where it diverged.

The Play view reads scenes and choices, explains near misses, and inspects participant knowledge through the run kernel. The bot explorer checks coverage without playing.

## MCP operating rules

- Read `workspace.context` before acting on a user's active view or selection.
- Use `content.get` for the focused persisted record; reserve `workspace.inspect` for cross-entity or whole-project checks.
- Use entity-level operations for ordinary edits. Whole-project replacement requires explicit confirmation.
- Preserve current revisions and validate material changes. Approval remains a separate explicit operation.
- If context is absent, or the focused target remains ambiguous after `content.get`, do not guess; ask a focused question.
- Do not automate the UI when the MCP operation can perform the requested work.

## Failure handling and acceptance

HTTP and MCP mutations report validation and revision errors without partial updates. Imported project content is data, never executable code. The service binds to loopback by default and refuses state-changing and MCP requests from browser origins outside its allow-list.

Acceptance focuses on project CRUD and import/export, graph/precondition validation, deterministic run transitions, stale revision handling, replay and rewind, run reload divergence, native C ABI behavior, and MCP context-to-entity workflows. No model, provider, training, generation, semantic ranking, or inference acceptance gate remains.
