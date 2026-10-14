# Runtime benchmarks

Run `bun run bench -- --iterations 100` from the repository root. The Rust release harness uses freshly allocated in-memory world state and no project database. It measures deterministic eligibility checks, role binding, scoring, sorting, and a complete synthetic turn with 10,000 definitions, 100 NPC actors/rules, and 20 locations by default. Use `--definitions`, `--actors`, `--locations`, and `--iterations` to vary the workload.

The report at `rust-runtime-2026-10-14.json` was measured on Windows x86_64 with an AMD Ryzen 9 7950X (16 cores / 32 logical processors). At 100 warm samples, deterministic selection measured 1.84 ms p95 and the complete candidate-evaluation plus player/NPC turn measured 3.32 ms p95. These numbers cover the Rust baseline and simulation path only. They do not measure EmbeddingGemma inference, a trained ranking head, memory use, or end-to-end web latency; those remain separate acceptance work.
