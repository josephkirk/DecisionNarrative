# Specification: Two-Stage Storylet Selection

**Document status:** Draft 0.1
**Date:** 2026-10-18
**Scope:** Runtime selection of one storylet per beat, from a library of up to 5 million storylets.

## 1. Purpose

The system shall select one storylet for the current world state. The selection shall fit the feel of the state. The selection shall not require an exact match of all quality gates. The system shall return a storylet ID. The system shall not return storylet text.

## 2. Terms

| Term | Meaning |
|---|---|
| Storylet | One authored beat. It has tags, role slots, a hard mask, and a text reference. |
| World state | The current locations, actors, roles, qualities, and recent history. |
| Storylet encoder | The model that turns a storylet's tags and optional text into a vector. |
| State encoder | The model that turns a world state into a query vector. |
| Index | A nearest-neighbor index over storylet vectors. |
| Candidate set | The storylets that the index returns for one query. |
| Hard mask | The short list of facts that must hold for a storylet to be playable. |
| Soft feature | A quality or tag that affects the score but does not block a storylet. |
| Ranker | The small model that scores each surviving candidate. |
| Must-play beat | A storylet that the arc layer requires. It bypasses scoring. |

## 3. Derivation from first principles

1. The game must show a storylet that fits the state.
2. The library is large, so the system cannot score every storylet at each beat.
3. Exact gate matching fails at this scale. Few storylets match every quality exactly.
4. Therefore, the system must narrow the library cheaply, then score the narrowed set carefully.
5. Therefore, the design has two stages: retrieval and ranking.
6. Logic that must never break stays in code. Feel stays in learned models.

## 4. Functional requirements

- **FR-1:** The system shall accept a world state and return one storylet ID, or a fallback signal.
- **FR-2:** The system shall retrieve up to K candidates from the index, where K defaults to 200 and is configurable from 100 to 300.
- **FR-3:** The system shall apply the hard mask to every candidate before scoring.
- **FR-4:** The system shall bind role slots in code. A storylet whose required roles are missing shall not be scored.
- **FR-5:** The system shall score each surviving candidate with the ranker.
- **FR-6:** The system shall sample one storylet from the scores using a temperature and a probability floor.
- **FR-7:** The system shall accept an optional list of must-play beats. A must-play beat shall bypass scoring when its hard mask passes.
- **FR-8:** The system shall apply a repetition penalty for storylets played in the recent window.
- **FR-9:** The system shall give every new storylet a vector from its features at publish time.
- **FR-10:** The system shall reserve an exploration share for storylets with few impressions.
- **FR-11:** The system shall log every decision with the data listed in section 7.
- **FR-12:** The system shall produce the same output for the same state, seed, and versions.

## 5. System overview

The request enters the state encoder. The state encoder produces a query vector. The index returns the top K storylet IDs. The hard mask and role binding remove invalid candidates. The ranker scores the survivors. The sampler picks one ID. The logger records the decision. The response returns the ID and its metadata.

## 6. Components

### 6.1 Storylet encoder

- **Input:** tags from a controlled vocabulary, role slots, location tags, mood tags, arc stage, and an optional short text description.
- **Tag tower:** An embedding table for tags, pooled into one vector. This tower is trained on author rankings and playtest logs.
- **Text path (optional, v1.1):** A frozen text encoder. Google's EmbeddingGemma 2 text configuration is one candidate. It outputs 768 dimensions. Matryoshka truncation supports 128 dimensions.
- **Output:** One vector of 128 dimensions, stored as float16.
- **Rule:** Vectors are computed offline. The runtime shall not run the storylet encoder.

### 6.2 State encoder

- **Input:** location, role tags present, quality buckets, tension level, turn count, and recent storylet IDs.
- **Quality buckets:** Each quality shall be mapped to a named level, such as "low," "medium," or "high." The raw number shall not enter the encoder.
- **Output:** One query vector of 128 dimensions.
- **Runtime budget:** p95 ≤ 1 ms on target hardware.

### 6.3 Index

- **Type:** Approximate nearest-neighbor index with cosine similarity. HNSW is the default candidate. The team shall choose the library after benchmarking.
- **Size target:** 5 million vectors.
- **Memory estimate:** About 1.3 GB for raw float16 vectors at 128 dimensions, before index overhead. The team shall measure index overhead on target hardware and record the result.
- **Retrieval channel union (recommended):** The retrieval step should combine the vector top 150 with a tag-match top 50 from an inverted index. This reduces the recall ceiling for storylets with rare tags.
- **Updates:** New storylets shall be inserted incrementally. Retired storylets shall stay in the index until the next full rebuild, and the hard mask shall filter them out.
- **Full rebuild:** Nightly, and after any batch of more than 10,000 changes.
- **Latency target:** p95 ≤ 5 ms for K = 200 on target hardware.

### 6.4 Hard mask and role binding

- **Hard mask:** A list of at most 20 boolean facts per storylet. Examples: an entity is alive, an item is owned, a location is reachable.
- **Role binding:** Each role slot names required tags. The system shall check that a matching actor exists. The system shall bind actors to slots deterministically, using the lowest actor ID when several match.
- **Quality gates are not filters.** Quality conditions become soft features for the ranker. Only the hard mask and role binding remove candidates.

### 6.5 Ranker

- **Default model:** A gradient-boosted tree model trained with LambdaRank. The fallback is a two-layer network. The team shall choose the model by offline evaluation.
- **Inputs per candidate:**
  - Retrieval similarity score.
  - State features: location, role count, quality buckets, tension, turn count.
  - Storylet features: arc stage, mood match, tag overlap with the state.
  - Soft quality distance: how far each quality sits from the storylet's preferred band.
  - History: turns since the storylet last played, and play count in this session.
  - Role binding margin: how many optional roles match.
- **Output:** One score per candidate.
- **Latency target:** p95 ≤ 2 ms for 300 candidates.

### 6.6 Sampler

- **Probability:** p(i) ∝ exp(score_i / T). Default T = 1.0. The team shall tune T in simulation.
- **Floor:** Every eligible candidate shall have probability ≥ 0.001 before normalization.
- **Repetition penalty:** Subtract λ from the score of any storylet played in the last N beats. Defaults: λ = 1.0, N = 5.
- **Exploration:** With probability ε = 0.05, the sampler shall choose from candidates with fewer than 200 impressions. If none exist, the sampler shall use the normal rule.
- **Seeding:** The random generator shall be seeded from the session ID and the turn number.

### 6.7 Logger

Logs shall be written for every decision. Section 7 lists the fields.

## 7. Data contracts

### 7.1 Storylet record

```
id                  string, unique, immutable
version             integer
status              draft | active | retired
tags                list of controlled-vocabulary tags
location_tags       list
mood_tags           list
arc_stage           enum
role_slots          list of { slot, required_tags, optional_tags }
hard_mask           list of up to 20 fact keys
text_ref            pointer into text database
author_weight       optional float, for training labels only
updated_at          timestamp
```

**Publish checks:**
- Every tag shall exist in the controlled vocabulary.
- Every role slot shall have at least one required tag.
- The storylet encoder shall produce a vector without error.
- A near-duplicate check shall warn when cosine similarity to an active storylet is ≥ 0.98.

### 7.2 Select request

```
session_id          string
turn                integer
location            string
actors              list of { actor_id, role_tags, quality_buckets }
qualities           map of name to bucket
recent_ids          list of up to 10 storylet IDs
must_play           optional list of storylet IDs
versions            { index, model, feature_schema }
```

### 7.3 Select response

```
storylet_id         string or null
probability         float
score               float
top_reasons         list of up to 3 strings
fallback_used       boolean
versions            { index, model, feature_schema }
latency_ms          object with stage timings
```

### 7.4 Decision log

Every decision shall log: request ID, session ID, turn, state hash, the top 20 candidates with scores, the chosen ID and its probability, top reasons, all three version fields, the seed, and stage latencies. Logs shall follow the data retention policy.

## 8. Offline pipelines

| Pipeline | Trigger | Output |
|---|---|---|
| Storylet encoding | On publish | Vector per storylet |
| Index update | On publish (incremental) | Updated index |
| Index rebuild | Nightly, or after 10,000 changes | New index version |
| Ranker training | Weekly, or after new labels | New model version |
| Calibration | After each training run | Temperature T |
| Simulation | After each training run | Coverage and balance report |

Each pipeline shall produce a version. Each pipeline shall write its output to a versioned store. The runtime shall load one version of each artifact.

## 9. Non-functional requirements

| Requirement | Target | Method of check |
|---|---|---|
| Select latency, p95 | ≤ 10 ms, excluding fallback | Load test on target hardware |
| Library size | 5 million storylets | Index benchmark |
| Memory, index | Within budget set by the team | Measured after build |
| Determinism | Identical output for identical input | Replay test |
| Availability | Fallback path works without index or model | Fault injection |

## 10. Cold start and popular storylets

**New storylets.** A new storylet shall receive a vector at publish time. The vector comes from its tags and optional text. The storylet shall enter the exploration pool until it reaches 200 impressions.

**Popular storylets.** When a storylet reaches 5,000 impressions in the training window, it may receive a learned residual. The final vector is the base vector plus the residual. The residual shall be regularized with an L2 penalty. The residual norm shall not exceed 0.5 times the base vector norm. If the residual causes a regression on the holdout set, the system shall drop it.

## 11. Failure modes and fallback

| Failure | Behavior |
|---|---|
| Index unavailable | Use the weighted-sum baseline over a cached list of active storylets for the location. |
| Model unavailable | Use the retrieval similarity as the score. |
| No candidates pass the mask | Return the location's default storylet. If none exists, return null and log an error. |
| Version mismatch | Refuse to load. Use the last compatible version. Alert the on-call owner. |
| Latency budget exceeded | Return the best candidate found so far. Log the timeout. |

## 12. Versioning

- The system shall maintain three version fields: `index_version`, `model_version`, and `feature_schema_version`.
- The loader shall refuse any combination whose feature schema versions differ.
- A change to the feature schema shall create a new model version, and the team shall retrain the ranker.

## 13. Testing and acceptance

| Test | Method | Acceptance |
|---|---|---|
| Recall at K | Compare index top K with exact search on 10,000 held-out states | Recall ≥ 0.95 |
| Ranker quality | Compare ranker order with author rankings on held-out data | Beats the weighted-sum baseline by a margin the team sets in advance |
| Calibration | Expected calibration error on held-out picks | ≤ 0.05 |
| Simulation coverage | 10,000 simulated sessions per location | At least 90% of eligible active storylets are chosen at least once |
| Balance | Share of picks per storylet in simulation | No storylet above 2% of its location's picks, except must-play beats |
| Hard mask | Unit tests for every mask fact | 100% of masked storylets excluded |
| Role binding | Unit tests for missing and multiple matches | Deterministic result in every case |
| Determinism | Replay 1,000 logged decisions | Identical picks |
| Latency | Load test at target concurrency | Section 9 targets met |
| Fallback | Fault injection for each row in section 11 | Game continues with a valid ID |

## 14. Rollout

1. **Shadow mode:** Run the system beside the current selector. Log its choices. Do not show them.
2. **Playtest cohort:** Show the system's choices to a small playtest group. Compare with the baseline.
3. **Staged release:** Increase the share of sessions in steps. Stop if coverage or balance fails.
4. **Kill switch:** A single flag shall switch the runtime to the weighted-sum baseline.

## 15. Risks and mitigations

| Risk (from the approach's cons) | Mitigation |
|---|---|
| Two models and an index to maintain | One repository, one contract test, one pipeline definition |
| Recall ceiling | Retrieval channel union, and the recall test in section 13 |
| Feature design is most of the work | A feature spec file, reviewed by writers and engineers. Ablation tests for each feature group. |
| Index memory and rebuild schedule | Memory budget, nightly rebuild, incremental inserts |
| Popular storylets need their own vectors | Learned residual with a norm bound and holdout check |

## 16. Open decisions

1. Vector size: 128 or 256 dimensions. The default is 128.
2. Index library and parameters. Benchmark two candidates.
3. Ranker family: tree model or small network.
4. Whether v1 uses the text path, or tags only.
5. Target hardware for latency and memory budgets.
6. Owner of the feature schema, and the review process for changes.
7. Retention period for decision logs.

*Requirements in this document use "shall" for mandatory items and "should" for recommended items. Numeric targets are starting values. The team shall confirm each one on target hardware before release.*
