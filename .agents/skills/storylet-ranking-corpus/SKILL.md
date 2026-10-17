---
name: storylet-ranking-corpus
description: Use when creating large synthetic storylet-ranking corpora for DecisionNarrative model training or evaluation preparation; dispatches OMP agents by disjoint scenario family and uses the local Strata MCP tool. Never use synthetic rows for validation or held-out evaluation.
---

# Storylet Ranking Corpus

Generate diverse, provenance-preserving storylet preference examples for `model/ranking-example.schema.json`. This skill creates **training-only synthetic** rows; it does not establish evaluation truth or approve production storylets.

## Dispatch contract

1. Read `model/README.md`, `model/ranking-example.schema.json`, and `model/storylet_rank_data.py` before generation. Read relevant project craft skills when the brief needs them: `scene-craft`, `character-management`, `worldbuilding`, `interactive-fiction`, or `voice-style`.
2. Split the request into explicit, disjoint `scenario_family` slugs (for example `harbor-flooded-archive`, `orbital-customs-dispute`). Each family belongs to exactly one dataset split. Never let two workers share a family.
3. Use OMP's native `task` dispatch to start one worker per family. Give each worker the relevant skill names, its unique family slug, project brief/canon constraints, and row quota. The task `tools` allowlist accepts eval-registered tools, not project MCP tools; omit that field and explicitly instruct the worker to call only `mcp__decisionnarrative_strata_generate_storylet_ranking_batch`. OMP does not hard-isolate the worker to that MCP tool, so require JSONL/count output only and have the parent write files. Do not let workers edit application code or the ranking schema.
4. Each tool call makes 1–24 rows. For a larger quota, call it repeatedly with the same family and a distinct increasing `batch_index` (starting at 1). IDs include family, batch index, and row number, so they remain stable and unique across batches. Keep `scenario_family`, `split=train`, `label_source=strata_synthetic`, and provenance unchanged. A call failure or malformed response invalidates that call as a whole; retry only that batch, never fabricate replacements.
5. Ask each worker to return JSONL rows only, plus a short count/family manifest. The parent merges worker output in sorted family/batch/id order; reject duplicate IDs, a family appearing in more than one split, schema errors, missing feature keys, non-finite or out-of-range feature values, empty strings, and any validation/held-out row not marked `human_reviewed`.
6. Save synthetic rows separately, e.g. `data/generated-ranking/train-strata.jsonl`. Never append generated rows to human-reviewed validation or held-out files. Do not infer feature labels are objectively true: Strata-generated rankings are weak synthetic supervision and need human sampling/review before any claim of training readiness.
7. Validate a training-only file row-by-row with the repository contract: `uv run --project model --no-sync python -c "import json,sys; from pathlib import Path; from model.storylet_rank_data import RankingExample; rows=[RankingExample.from_dict(json.loads(line)) for line in Path(sys.argv[1]).read_text(encoding='utf-8').splitlines() if line.strip()]; assert rows and len({row.example_id for row in rows}) == len(rows); print(f'{len(rows)} rows validated')" data/generated-ranking/train-strata.jsonl`. The full `read_jsonl` training-dataset gate requires all three splits; do not add synthetic rows to satisfy it. For training, merge only after separately sourced, human-reviewed validation and held-out partitions have been created with disjoint scenario families.

## MCP connection

The repository MCP entry in `.mcp.json` launches `scripts/strata-storylet-mcp.ts` with Bun. It calls a loopback OpenAI-compatible Strata endpoint. Configure `STRATA_BASE_URL` (default `http://127.0.0.1:8080/v1`) and `STRATA_MODEL` (default `strata-uncensored`) in the OMP host environment. Keep the endpoint loopback-only; this server is a local generator, not a general-purpose proxy.

The only exposed tool, `generate_storylet_ranking_batch`, requests and validates exact-size training batches against the nine feature names in `model/ranking-example.schema.json`. The server assigns the training split, synthetic label, provenance, and per-call IDs; validates all feature values; and refuses malformed or partial batches. It does not write files, create human labels, score validation data, or update the model.

## Stop conditions

Stop when the requested number of distinct rows is produced, the combined training-only JSONL parses against the schema, and the family/ID manifest is complete. Report requested/generated counts, families, validation command/result, and the explicit synthetic-label limitation. If Strata is unavailable, return the exact failed family/batch and error; never switch to another provider or invent rows.
