---
name: storylet-content-deck
description: Use when dispatching OMP agents to generate large, state-coupled storylet decks for DecisionNarrative authoring, model training, or evaluation preparation; requires deterministic engine checks, training/evaluation separation, and playtest evidence.
---

# Storylet Content Deck

Generate content as data for the existing storylet engine. A prose-quality pass never substitutes for precondition satisfiability, valid destinations, schema validation, or replayable engine behavior. Read docs/design/storylet-content-generation-guideline.md, docs/design/branching-narrative-within-storylet-content-structure.md, model/README.md, and model/ranking-example.schema.json before generation; docs are read-only source of truth. Read only relevant craft skills from scene-craft, character-management, worldbuilding, interactive-fiction, and voice-style.

## OMP dispatch

1. Inspect narrative_platform using the decisionnarrative_strata MCP server. The narrative_platform tool operates the running local studio over its loopback HTTP API; it is not a general URL proxy. Use workspace.inspect first to read current canon, qualities, characters, locations, storylets, and scenarios.
2. Partition the requested deck into disjoint scenario families and bounded storylet quotas. Dispatch one OMP task worker per family with its exact craft skill names, canon excerpts, family scope, quota, and acceptance rules. Instruct each worker to use the narrative_platform MCP tool for generation.create, generation.get, generation.advance, generation.edit_stage, and generation.save_draft. Do not ask workers to edit source, docs, schema, or project-wide state. Avoid concurrent project-bundle replacement and shared scenario edits.
3. Generation jobs are staged and revision-checked. Save drafts only after the generated storylet has been reviewed; saving yields a draft, not approval. Approve only via storylet.approve with the current expected revision after deterministic project validation and manual narrative review. Never infer approval from successful generation.
4. Give each task isolated IDs and record worker, family, job IDs, revision, storylet IDs, and validation outcomes. Parent verifies returned payloads, deduplicates IDs, and resolves cross-family canon conflicts before merging. A failed call is a failed batch; do not fabricate completions.
5. Keep generated prose and generated ranking labels distinct. generate_storylet_ranking_batch assigns split=train and label_source=strata_synthetic; those weak synthetic labels are training-only. Never use synthetic examples as validation or held-out evaluation, and never claim model-judge scores are objective evaluation truth.

## Required verification

Before deployment, validate the entire candidate deck, not a sample: strict schema and ID uniqueness; every precondition has a satisfiable trajectory; every non-ending storylet has a valid choice/divert/explicit ending; all referenced characters, locations, qualities, and effects exist; and traversal finds no unhandled starvation/dead-end under the declared archetypes. Report compiler success rate and retries with denominator and cap. Do not represent a static local check as formal SMT/ASP proof unless a solver actually ran.

Run the repository prose audit. Require zero banned phrase hits; enforce sentence-burstiness CV >= 0.35 where the audit is applicable; reject final participial closers and em-dash violations; check paragraph topology for roadmap/coda repeats; and compare exact protected names, numbers, dates, and state-variable tokens before and after editing. Apply the actual UI character and line constraints to every text field. Evaluate choice/path coverage against the full declared graph and publish numerator/denominator.

For confidence bands, execute only if the deployed dispatcher emits a calibrated confidence: C >= 0.85 auto-execute; 0.50 <= C < 0.85 hint; C < 0.50 fallback and human review. The repository ranking confidence is uncalibrated; never assert this gate passed based on rank scores.

A judge score >= 3.5/4 or >= 8/10 and A/B improvement with p < 0.05 plus a positive confidence-interval lower bound require an actual blinded/paired evaluation, a declared rubric, dataset provenance, and reproducible statistics. Without those, report them as unmeasured, not passed. UI checks require a real browser/engine playtest, not a source-only test.

## Platform operations

narrative_platform exposes workspace inspection and bundle validation/replacement; content and scenario CRUD; storylet approval with optimistic revision checks; generation-job lifecycle; craft resource inventory; and playtest creation, snapshots, director evaluation, commands, choices, branching, events, and deletion. Bundle replacement requires explicit confirmReplace=true and is disruptive; prefer ordinary per-entity edits. Playtests mutate their session state; use disposable sessions and delete only after preserving needed evidence. The local studio must already be running and configured to the intended database. The tool refuses non-loopback API origins.

## Report

Return requested/produced counts, scenario families, generated vs approved counts, schema/compiler/precondition/traversal/prose/UI results with denominators, path coverage, retry behavior, playtest observations, and unresolved gates. Preserve training/evaluation provenance and clearly label any metric not exercised.

## Large evaluation decks

When a task separates player-facing traversal from candidate inventory, enforce separate counts. For a 50-storylet run, build 50 ordered beats with at least ten eligible variants per beat (500 candidates minimum); every choice from a nonfinal beat advances exactly one beat. Mark only final-beat storylets as explicit endings. Do not pad with duplicate prose or unreachable rows. Keep candidate count and experienced count separate in all reports.

Use data-oriented scenario policies for player start, actor population, and actor locations: fixed, seeded-random, or attribute-matched. Report actors at each location and role/attribute-compatible actors for each candidate; a storylet with unsatisfied role requirements is not eligible. Seed at least one authored canon event and one consequential world event, then observe both in the actual playtest event log.

For large decks, dispatch OMP workers by disjoint story-family quotas. Give each only the relevant craft skill, canon excerpts, quota, and required tools. Use [4mgenerate_storylet_content_batch[0m for bounded real storylet draft candidates from local Strata, then inspect and persist via [4mnarrative_platform[0m [4mcontent.save[0m. The batch generator returns drafts only; it does not save or approve them. Do not use ranking-pair generation as a substitute for storylet content. Review, validate, and approve drafts separately; never approve on generation success.

Prove the full candidate pool with deterministic schema/ID/effect/precondition/location/role validation and seeded traversal over the actual graph. A run passes the 50-storylet gate only after observing 50 unique storylets and selecting a designated ending as the 50th. No eligible storylet, an error, or a timeout is not an ending. Report compiled/generated/approved/traversed counts and every denominator; do not claim formal SMT/ASP, Monte Carlo, judge, burstiness, paired-lift, or path-coverage thresholds without running their corresponding checks.
