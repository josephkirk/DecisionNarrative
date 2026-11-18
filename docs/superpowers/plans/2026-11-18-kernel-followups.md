# Kernel follow-ups: rollback cost, one idempotency mechanism, Clippy

**Date:** 2026-11-18
**Branch:** `chore/kernel-perf-idempotency-clippy`, from `main` at `c823814` (PR #10)
**Source:** known follow-ups listed in PR #10

## 1. Rollback cost grows with the length of the run

### What happens now

`RunKernel::propose` takes `let backup = self.clone()` before acting, and `act_internal` takes a second full clone (`crates/storylet-engine/src/run.rs`). A full clone copies everything the run has accumulated:
- every branch's step list;
- every snapshot (one world plus one `FlowGraphInstance` per step);
- the proposal log, the idempotency map and the event log;
- the registry, passion registry and dispatcher, which never change during a run.

On top of that, each `Snapshot` also clones the whole `proposals` and `event_log` vectors. Snapshots therefore cost O(steps) each, and memory is O(steps²).

Baseline: run `cargo test --release --test long_run -- --ignored --nocapture` in `crates/storylet-engine` on the unchanged branch first and record the early and late mean per-step times (2,000 Wait proposals). The test is committed with this plan.

### Change

1. **Checkpoint instead of a full clone.** Replace both `self.clone()` backups with a `Checkpoint` that holds:
   - clones of only what an action can change: `world`, `instance`, `storylet`, `selections`, `next_invocation`, `stalled`, `rule_played`, `revision`, `next_event_id`, `pending_deltas`, `pending_rule_events`, `branch`, `current_step`, `rewound`;
   - the lengths of the append-only containers: `branches`, `branches[branch].steps`, `snapshots`, `snapshots[branch]`, `proposals`, `event_log`.

   Rollback restores the clones and truncates the containers back to those lengths. That includes a branch pushed by the `rewound` path inside `act_internal`. Fields set once at start (`registry`, `passion_registry`, `dispatcher`, `run_seed`) are never copied. The knowledge and telling comparison in `propose` that currently reads `backup.world` reads the checkpoint's world instead.
2. **Snapshots store log lengths, not log copies.** `Snapshot.proposals` and `Snapshot.event_log` become lengths. Rewind truncates the live logs to them. This is sound only because a rewind is always followed by a new branch whose history is a prefix of the current logs. Add a test that rewinds, acts on the new branch, rewinds again inside that branch, and gets the same logs as the current implementation.
3. **Leave the per-step world and instance in each snapshot for now.** Rewind depends on them, and dropping them means rebuilding from the instance's commit history. That is a bigger change; measure after steps 1 and 2 and record the result instead of doing it in this branch.

### Proof

- `tests/long_run.rs` (ignored by default, run with `--ignored --nocapture`) prints the early/late per-step ratio before and after. The goal: the late mean is within 2x of the early mean, where today it grows with the run. Record both measurements in the PR.
- The adversarial suite still shows a rejected proposal leaves the serialized kernel byte-identical, now including a rejection after a rewind and a rejection that would have opened a new branch.
- The whole engine suite passes unchanged.

## 2. One idempotency mechanism per operation

### What happens now

Run actions use the kernel's idempotency. Start, rewind, reload and delete still go through the service table `run_idempotency` (`check_precondition`, `save_atomic`, `prior` in `crates/storylet-engine-service/src/runs.rs`). Separately, `RunKernel::rewind` restores the snapshot's revision, so a run's revision goes backwards and a client holding an old revision can act again after a rewind.

### Change

1. **Rewind becomes a kernel operation with the same guarantees as an action.** `rewind(authority, RewindRequest { to_step, expected_revision, idempotency_key })`:
   - requires Architect authority;
   - checks the revision and records the key in the kernel's idempotency map;
   - advances the revision instead of restoring the old one;
   - is recorded in the proposal log so replay reproduces it.

   The service's `RunOp::Rewind` path and its idempotency calls go away.
2. **Reload is idempotent by state, not by key.** Reload rebuilds the run from its proposal log against the current project revision. If the run is already at the current project revision, reload returns the current envelope without changing anything. A repeated reload therefore has no effect, and no stored key is needed.
3. **Delete is idempotent by state.** Deleting a run that no longer exists returns the same success response as the first delete. No stored key is needed.
4. **Start keeps one key, on the run row.** A run cannot deduplicate its own creation, so the start key is stored as a unique `start_key` column on `runs`, together with the request body. A repeated start with the same key and body returns the existing run; the same key with a different body is a conflict.
5. Drop the `run_idempotency` table and `prior`, `check_precondition` and `save_atomic`, replacing them with a single transactional save. The v8 schema has not been released outside this repo, so change it in place with no migration.
6. Update MCP tool descriptions, the studio run client and view-model if request or response shapes change, and `docs/HANDOFF.md`.

### Proof

Service tests for each operation:
- repeat with the same key or same state returns the same result;
- a key reused with a different body is refused;
- a stale revision is refused for rewind;
- the revision never decreases across rewind;
- a run restored after a service restart replays rewinds correctly.

## 3. Clippy

Baseline: roughly 90 warnings in `storylet-engine` and 45 in `storylet-engine-service` with `--all-targets`, no errors. The exact count does not matter; the target is zero under `-D warnings`.

- Fix every warning, by hand or with `cargo clippy --fix`, in both crates.
- Delete the unused test fixtures it reports (`content`, `content_without_causeway_close_rule`, unused constants, `edge`, `mission_node`) rather than allowing them.
- `unsafe` FFI functions get `# Safety` sections. Those are doc comments, which conflicts with the project's NO CODE COMMENTS rule; resolve that conflict by moving the safety contract into `include/storylet_engine.h` and adding `#![allow(clippy::missing_safety_doc)]` to `ffi.rs` only.
- Functions with too many arguments get a parameter struct only where that reads better; otherwise allow the lint on that function.
- Done means `cargo clippy --all-targets -- -D warnings` passes for both crates.

## Order and gates

Commit the work in this order: Clippy first (mechanical, keeps later diffs readable), then rollback cost, then idempotency.

Gates for the PR:
- both cargo suites (engine with `--all-targets`);
- `bun install`, `bun run typecheck`, `bun test`, `bun run build`;
- Clippy with `-D warnings` on both crates;
- the visual Play suite;
- the measured before/after numbers in the PR description.

No code comments; nothing deprecated kept.
