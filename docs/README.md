# Documentation

## Product and design

- [Chrono decision engine](design/chrono-decision-engine.md) — deterministic runtime architecture, dense contracts, implementation status, and known limitations.
- [Entity, trait, and quality model](design/entity-trait-quality.md) — entities, traits, qualities, metadata and asset links, quality effect commands, subscriptions, and the traversable entity graph.
- [Storylet studio design](design/storylet-studio.md) — product scope, runtime and authoring architecture, data contracts, playtesting, and acceptance criteria.
- [Studio plugin system](design/studio-plugin-system.md) — extension contracts, graph lowering, lifecycle, and the built-in canvas plugin.
- [Social simulation design](design/social-simulation.md) — setting-neutral principles for actors, relationships, knowledge, institutions, causality, and authored narrative.
- [Proposal: human-first, LLM-assisted non-linear narrative](design/proposal-human-first-nonlinear-narrative.md) — proposal (not approved) for non-linearity in space, time, and participants, with the *Weir's Crossing* example and prototype evidence.
- [Play (playtest UI) spec](design/playtest-ui.md) — the reader-first playtest surface: one-click Play, auto-advance, why-not, rewind and branch, inspect, coverage report.
- [Storylet authoring guide](guides/storylet-authoring.md) — practical checklist for authoring, reviewing, and playtesting project-owned content.
- [Agent handoff](HANDOFF.md) — implementation map, current limitations, verification commands, and next work.

## Measurements

- [Runtime benchmark notes](benchmarks/README.md) explain the workload and limits of the Rust-only measurement.
- [Captured benchmark](benchmarks/rust-runtime-2026-10-14.json) contains the raw run data.

The root [README](../README.md) is the user-facing setup and usage guide. The storylet studio design is the requirements reference; the social simulation document explains reusable narrative principles without defining project setting content.
