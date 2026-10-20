# DecisionNarrative handoff

## Product objective

DecisionNarrative is a local authoring and playtest platform for setting-neutral, nonlinear storylet games. Designers create project-owned entities and storylets; the Rust engine owns deterministic eligibility, world state, choices, effects, and replay. The studio does not use Strata, LLM generation endpoints, image-generation providers, embeddings, or a learned ranker.

External MCP agents remain supported. The Vue studio publishes the active view, project, focused entity/selection, and playtest revision as a local interaction-context snapshot. Agents read `workspace.context` and operate through the existing `narrative_platform` MCP operations without UI automation. This is a context bridge, not an in-app AI provider or a content-generation API.

## Architecture map

- `crates/storylet-engine/`: host-independent deterministic project and playtest runtime, plus native C ABI.
- `crates/storylet-engine-service/`: local HTTP/MCP service and SQLite project/session persistence.
- `src/web/studio/`: Vue authoring UI, interaction-context provider, storylet graph editor, tag manager, Codex, scenarios, playtest, and project interchange.
- `src/web/studio/interactionContext.ts`: typed model-free snapshot of route, project revision, focused entity/selection, and active playtest context.
- `docs/design/storylet-studio.md`: current product boundaries and MCP context contract.
- `docs/guides/storylet-authoring.md`: manual authoring and review workflow.

## Current authoring rules

- Storylets may target location tags; a storylet with no location tags is not location-gated. Preconditions support quality conditions and visited-location/met-character IDs or tags/roles. Role parameters default to current-location binding; `location_policy: "any"` allows remote/abstract actors via JSON/MCP.
- Graphs require exactly one Start and one or more End nodes, with every node reachable from Start and able to reach an End. Each End is terminal or a jump to another storylet; a jump takes precedence, and a dedicated ending requires every End to be terminal with no jump. Save validates before writing and approval checks the saved revision.
- Go To Location is a Choice-branch action and selects an existing project location.
- Tag editing is inline with suggestions; Tag Manager shows tag references and opens the referenced record.
- Content is authored in the studio or by a requested MCP agent. Approval remains an explicit user decision.

## MCP context workflow

1. Read `workspace.context` before acting on the current UI target.
2. Use `content.get` to verify a focused persisted record; reserve `workspace.inspect` for cross-entity or whole-project checks.
3. Use entity-level MCP operations for edits. Validate material changes and preserve optimistic revisions.
4. Do not use whole-project replacement without explicit user confirmation. Do not approve storylets unless requested.
5. If the context is stale, re-read the focused persisted record with `content.get`; ask only if the active target remains ambiguous.

The MCP server and studio use the same local project database. Context is a local UI snapshot with a capture timestamp and freshness signal; it does not authorize unrelated edits.

## Build and verification

```sh
bun install
bun run build
bun run typecheck
bun test
cargo test --manifest-path crates/storylet-engine/Cargo.toml
cargo test --manifest-path crates/storylet-engine-service/Cargo.toml
```

`bun run play` uses `data/storylet_studio_v2.db` and shares it with the default MCP server. `bun run play:fresh` opens the separate `data/storylet_studio_clean.db` workspace and skips legacy imports; it does not alter the v2 database. Use temporary or in-memory databases for isolated checks.

## Workspace safety

- Read `git status --short` before edits; preserve unrelated user changes and untracked project notes.
- Do not run build/test/format checks while delegated edits are still in flight. Run the relevant gates once after integration.
- Keep generated build directories ignored. Review and preserve local data independently from tracked source.
