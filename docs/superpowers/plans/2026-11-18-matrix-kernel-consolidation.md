# MATRIX round 1: one trusted kernel, no legacy

**Date:** 2026-11-18
**Source:** gap review against `docs/TENNET.md` (PRD v0.1)
**Scope:** PRD Phase 0 plus the kernel half of Phase 1 (problem-ladder levels 0, 1, 2, 5 and 6 as kernel primitives; backlog tickets 1, 2, 3, 4, 6, 7, 8 and the kernel side of 10)
**Out of scope this round:** `crates/agent-cognition`, `crates/emergence-gym`, any model call, the World Compiler, new studio screens

## Decision

The Chrono `RunKernel` over `WorldStateTable` is the single authoritative runtime. It is the one the studio already runs. Everything on the older path is deleted, not deprecated:

- engine: `WorldState`, `WorldCommand`, `Effect`, `apply_command`, `replay_events`, `fork_at`, `storylets.rs`, `simulation.rs`, `runtime.rs`, `project.rs`, the `dn_engine_*` and `dn_playtest_*` C ABI, `bin/host.rs` as it stands, and their tests;
- service: the `/api/v3/playtests/*` routes, the Chrono playtest routes, MCP `playtest.*` and `chrono.playtest.*`, `StoryletRuntime` session handling, and the `runtime_sessions`, `runtime_events` and `chrono_playtest_sessions` tables.

Unwired Chrono contracts (`chrono_triggers`, `chrono_transactions`, `chrono_selection`, `chrono_tables`, `chrono_features`) are future work, not legacy, and stay.

## What gets built on the kernel

| PRD item | Kernel change |
|---|---|
| FR-01 trusted action catalog | `actions.rs`: `ActionProposal` (actor, typed action, expected revision, idempotency key, optional decision provenance) resolved by `RunKernel::propose`. Raw `act` is no longer public. Authority is checked against the participant's controller. |
| FR-02 actor-specific perception | `observation.rs`: `RunKernel::observe(actor)` returns only what that actor can perceive. The global view becomes an explicitly named Architect view. |
| FR-03 evidence-backed memory | Knowledge rows carry the id of the event that produced them and the telling actor where there is one. |
| FR-05 replay | A run replays from recorded proposals, including provenance, with no policy call. |
| FR-06 coordination primitives | Typed `Transfer` and `Tell` actions between co-located participants. |
| FR-07 causal inspection | An event log with causal parents and witnesses; `RunKernel::explain(event)` returns the ancestor chain. |
| Gate §14.1 | `tests/adversarial_actions.rs`: every rejected proposal leaves state and event log byte-identical. |

Review findings fixed along the way: any caller acting as any participant; views leaking global state; knowledge provenance index mismatch for mission-scoped nodes; a bad NPC storylet rolling back the human's step; CORS open to any origin.

## Clean database

New schema version and new default files: `data/storylet_studio_v8.db` and `data/storylet_studio_v8_clean.db`. Only tables still in use are created. No migration or compatibility code. Existing files under `data/` are never modified or deleted.

## Tasks

| Task | Agent | Worktree | Owns | Depends on |
|---|---|---|---|---|
| A. Kernel | Codex (`gpt-6.1-luna`) | `matrix-kernel` | `crates/storylet-engine/**`, `examples/native-host*/**`, `scripts/run-native-host.ps1`, `docs/design/chrono-decision-engine.md`, `docs/benchmarks/**` | none |
| B. Service, studio, database, docs | Muse | `matrix-service` | `crates/storylet-engine-service/**`, `src/web/**`, `src-tauri/**`, `tests/**`, `package.json`, `run.bat`, all other docs | none |
| C. Integration | Codex (`gpt-6.1-luna`) | `matrix-integration` | everything, plus `CHANGELOG.md` | A, B |

A and B run in parallel with disjoint file ownership. In A's worktree the service crate will not compile once the legacy engine API is gone; that is expected and C resolves it. C merges both branches, wires `propose`, `observe` and `explain` through HTTP, MCP and the Play inspector, runs every suite, and starts the service once against a fresh v8 database.

Workers commit on their own worktree branch and do not push. Nothing is merged into `feat/entity-as-template` until you review the integration branch.

## Gates for the round

- Engine, service and Bun suites pass on the integration branch; typecheck and build pass.
- The adversarial suite passes with zero mutations on rejection.
- A recorded run replays to identical state and event log.
- `grep` finds none of the deleted symbols anywhere in the repo.
- A foreign `Origin` gets no CORS grant from the local service.

## Next round (not started)

`crates/agent-cognition` (policy trait, scripted and utility baselines, one model provider behind the trait), `crates/emergence-gym` (multi-seed harness and baseline comparison), then the two-agent exchange scenario from backlog ticket 7 as the first measured experiment.
