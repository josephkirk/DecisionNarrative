# Proposal: A Human-First, LLM-Assisted Narrative System for Games Non-Linear in Space, Time, and Participants

**Status:** Proposal. Not approved for implementation.
**Date:** 2026-11-05
**Branch audited:** `refactor-backend`, working tree with uncommitted changes
**Evidence:** the prototype at [`examples/proposal-weirs-crossing/explore.ts`](../../examples/proposal-weirs-crossing/explore.ts) and its saved output in [`results.txt`](../../examples/proposal-weirs-crossing/results.txt)
**Related:** [TENNET.md](../TENNET.md), [chrono-decision-engine.md](chrono-decision-engine.md), [social-simulation.md](social-simulation.md), [storylet-design-limitation.md](storylet-design-limitation.md)

---

## 0. Summary

1. **Keep the core rule as it is.** Humans author what is true and what can happen. The deterministic Rust kernel decides what did happen. An LLM may suggest drafts, wordings, interpretations and rankings, but every suggestion passes through a human or a validator. The game must stay fully playable with the LLM turned off.
2. **Make each of the three non-linear axes a first-class data type:**
   - **space:** locations connected by edges whose availability changes over time; being in the same place is what makes someone a witness
   - **time:** keep three timelines apart. The *world clock* is simulation time. The *chronicle* is what actually happened, in story order. The *telling* is the order in which each participant learns about it.
   - **participants:** players, playable points of view and NPCs share one participant model. Each participant has its own location, its own knowledge with a source for every fact, and its own place in the telling.
3. **Make verification part of authoring.** Headless traversal bots, invariant checks and coverage-hole reports run in the authoring loop. Without them, a world that is non-linear on three axes cannot be authored safely. In the prototype, these tools found a space-time bug and a reachability gap that depends on how many people are playing. A human did not spot either one.
4. **Use LLMs mainly at authoring time.** At runtime they are optional, sandboxed adapters with written contracts. Every output is recorded so a replay never calls the model again.
5. **State the hard limits up front.** No world can let players change the past, visit it in any order, and stay free of contradictions all at once. A shared world cannot pace every player's story at the same time. No validator can prove that a generated sentence implies nothing it shouldn't. Section 6 covers what is impossible and what is only expensive.

---

## 1. Definitions

| Axis | "Non-linear" means | Example |
|---|---|---|
| **Space** | Content is attached to places, not to a sequence. Players can be anywhere, several places can be active at the same time, and the map itself can change. | A tidal causeway is open only at low tide. Two players are in different buildings during the same hour. |
| **Time** | The order of telling differs from the order of events. Players can investigate the past, have flashbacks, play recollections from several points of view, or fork a timeline. | One run reveals the 23:00 event before the 22:00 event. Another run reveals them the other way round. |
| **Participants** | There is more than one point of view or agent whose choices matter: co-op players, alternating playable characters, NPCs acting on their own goals. Each knows different things. | Teodor remembers the lantern on the beach. Mara does not, until they meet and compare notes. |

**Human design first** means humans own the *possibility space*: entities, rules, canon truths, storylets, voice, milestones and approval. Tools may *propose* changes to that space. They never commit changes to it on their own.

---

## 2. Current state of the software (audit, 2026-11-05)

### 2.1 What exists and works

The Rust tests pass on this branch: about 127 engine tests and 41 service tests. The documented counts are 144 and 68, so the documentation is out of date (see 2.3).

| Capability | Where | Relevance to the three axes |
|---|---|---|
| Deterministic, idempotent, revision-checked world commands with causal `caused_by` events | `crates/storylet-engine/src/lib.rs:26-125` | Foundation for replay, forks and causal inspection. **Keep.** |
| Actors with location, role tags and a `busy` flag | `storylets.rs:10` | The start of "participants" and co-presence. |
| Directional relationships `relationships[a][b]: f64` | `lib.rs:43` | Social state that differs per participant. |
| `knowledge: actor → set<claim>` and a `Knows` predicate | `lib.rs:45`, `storylets.rs:33,456` | Per-participant knowledge can be **read** but not **written** (see gaps). |
| Rule-authored NPC actions: priority-ordered, bounded, at most one action per actor per pass, atomic batch | `simulation.rs:9-80` | NPC agency on the world clock. |
| Storylets with location tags, roles, repeatability and cluster type (Emily Short's cluster types) | `storylets.rs:97-137` | Content attached to places, with pacing metadata. |
| Chrono flow graphs: tokens, time budgets, QBN hub, salience, waypoint conversations | `chrono_runtime.rs` (5.8k lines) | Scene-level non-linearity. |
| Timeline scene sections with scrubbing over `[t0,t1)` | `chrono_runtime.rs`, `chrono_playtest.rs:2218` | Intra-scene time. Already reversible. |
| Persistent history as a DAG, `checkout`, `reconcile_history`, `invoke_story_to_poi` | `chrono_runtime.rs:4710,4751,4813` | Forks, counterfactuals and jump-to-point-of-interest: the hardest part of "time" is already present. |
| Write-set conflict detection | `chrono_transactions.rs` | The primitive needed when several participants act at once. **Contract only.** |
| Trigger table with a CSR cursor `process_tick` | `chrono_triggers.rs` | World-clock events. **Contract only; not used by the live scheduler.** |
| Passion model (micro/macro passions, zero-sum rival links, thresholds) | `chrono_state.rs:507-556` | Character motivation that is visible to the author. |
| MCP `narrative_platform` with `workspace.*`, `content.*`, `scenarios.*`, `playtest.*` (incl. `fork`, `events`) and `storylet.approve` | `storylet-engine-service/src/mcp.rs` | The LLM-assist surface already exists and is model-free. |

### 2.2 Gaps relevant to the goal

| Gap | Evidence | Consequence |
|---|---|---|
| **Single-player assumption.** The world has one `current_location`, and `MovePlayer` is a dedicated effect. | `lib.rs:37,84` | No co-op, alternating points of view, or simultaneous participants. |
| **Knowledge cannot be written.** No `Effect` variant adds a claim, and the claim set has no source, kind or confidence. | `lib.rs:80-87` | No witnesses, no rumour vs. evidence, and no provenance invariants. TENNET FR-03 is unmet. |
| **One clock.** `turn: u64` is the only time. There is no story time (chronicle) and no per-participant telling order. | `lib.rs:28` | Players cannot investigate a fixed past out of order without hand-rolled flags. |
| **Static space.** Locations have no time-varying connectivity. | project contracts | No tides, curfews, closing hours or changing routes. |
| **No narrative spine contract.** Milestones and their alternative routes are not declared data, so they cannot be checked. | design docs only (social-simulation.md §7) | "Preserve multiple routes" cannot be checked automatically. |
| **No traversal bots or coverage report.** | n/a | Running out of content ("starvation") and gaps are found by hand. storylet-design-limitation.md §2 names this as the main failure mode. |
| Trigger table and transactions are contract-only. | chrono-decision-engine.md §5.3-5.4 | The world clock and simultaneous participants depend on these being wired up. |

### 2.3 Drift found during the audit

These should be fixed separately from this proposal.

- `docs/design/chrono-decision-engine.md §6` records 144 engine / 68 service tests. The current run on this branch is about 127 / 41.
- `README.md` describes the Playtest and Starting Scenarios studio views. This branch deletes `PlaytestView.vue`, `ScenarioView.vue` and their view-models, and `router.ts` no longer routes to them. Playtest remains available through HTTP and MCP only.
- `examples/native-host/storylet-project.json` (used by `examples/native-host-dotnet/Program.cs`) uses the removed `qualities` / `set_quality` schema. The engine has no `set_quality` effect, so the example is stale.

### 2.4 Design intent, and one tension to resolve

The tenets in TENNET.md fit this goal almost exactly: *"Reality is authoritative; intelligence is advisory"*, *"Humans remain creative authorities"*, *"Memory is evidence-based"*, *"Every consequence has a cause"*, *"Intelligence is a scarce resource"*, *"Delete before automating."*

**Tension:** README.md and storylet-studio.md say the product has no AI provider and no generation API. The goal asks for LLM assistance. **Resolution proposed here:**

- The core and the studio stay model-free.
- LLM assistance enters through two doors only, both outside the kernel:
  1. **Authoring time:** an external MCP agent, which exists today.
  2. **Runtime:** an optional adapter process that implements typed, recorded contracts.
- With the adapter off, the game is still complete.

This keeps every statement in the current README true, and adds a boundary rather than a dependency.

---

## 3. Design principles for the ideal system

1. **Truth, belief and telling are different data.** World truth is the chronicle. Belief is per-participant knowledge with provenance. Telling is the order and manner in which a participant learned something. Mixing them causes the classic failures: a rumour becomes fact by repetition, an NPC uses knowledge it could not have, or a flashback contradicts the present.
2. **Place and time make a witness.** Nobody learns anything by magic. Knowledge moves only through an in-world act (witnessing, a document, being told, inference) that the kernel can record.
3. **The past is either fixed or explicitly forked.** Recollections reveal canon. Changing something creates a new timeline branch that is labelled as such.
4. **Storylets cast roles, not people.** In Versu's terms these are social practices: role-agnostic reactive plans that offer suggestions rather than control. Any participant who satisfies a role can fill it. This is what lets one library serve one, two or four participants.
5. **Milestones are declared, routes are counted.** If the design promises resilience, every milestone must have at least two routes, and the tools prove reachability for every supported participant configuration.
6. **Every model output is a proposal with a receipt.** It arrives in draft state, or as a recorded decision envelope, and it never becomes a direct mutation.
7. **Verification is part of authoring.** A save can trigger a bounded exploration. The author sees holes in the same session in which they create them.

---

## 4. The proposed system

### 4.1 Architecture

```text
 HUMAN (Architect)                 LLM (advisory, optional)
   │ authors / approves              │ proposes drafts, wordings, rankings, intent maps
   ▼                                 ▼
┌──────────────────────── Authoring layer (studio + MCP) ───────────────────────┐
│ World constitution · Chronicle canon · Storylets · Spine (milestones/routes)  │
│ Voice guides · Line contracts · Approval gate · Coverage & invariant reports  │
└───────────────────────────────┬───────────────────────────────────────────────┘
                                │ validated, versioned project data
┌───────────────────────────────▼──────────────── Rust kernel (authoritative) ──┐
│ Space graph (time-varying edges) · World clock (TriggerTable)                 │
│ Participants (location, knowledge+provenance, telling cursor, controller)     │
│ Chronicle (canon past + committed events, story-time ordered)                 │
│ Eligibility → Selection profiles → Transactions (write-set conflicts)         │
│ History DAG (replay · fork · reconcile · invoke)                              │
└───────────────┬─────────────────────────────────────────────┬─────────────────┘
                │ observations (participant-scoped)           │ decision envelopes (recorded)
        ┌───────▼────────┐                           ┌────────▼─────────┐
        │ Player clients │                           │ Runtime adapters │ ← optional LLM:
        │ (1..N, async)  │                           │ (out of process) │   intent, lines, NPC proposals
        └────────────────┘                           └──────────────────┘
```

### 4.2 Space: a graph whose edges change over time

```rust
// data-oriented: dense rows, stable keys at rest
struct LocationRow   { key: StableKey, tags: TagBitset }
struct EdgeRow       { from: LocIx, to: LocIx, cost: u16, schedule: ScheduleIx, requires: PredicateIx }
struct ScheduleRow   { open_slots: SlotBitset }          // e.g. low tide = slots {1,4}
```

- Moving is a command, validated against the edge's schedule and its `requires` predicate. For example, crossing the causeway requires knowing the tide table.
- **Occupancy rules are mandatory for edges that close.** When an edge closes while someone is beyond it, an authored rule must decide what happens: stranded, returned or rescued. The prototype found this omission automatically (§8.2).
- **Co-presence at commit time decides who witnesses.** A storylet's visible outcomes are copied into the knowledge of every participant at that location, with `kind = witnessed` and the event ID as source.

### 4.3 Time: three timelines

| Timeline | Owner | Ordered by | Mutable? |
|---|---|---|---|
| **World clock:** slots and ticks, schedules, NPC routines, deadlines | kernel | simulation time | advances only |
| **Chronicle:** canon past entries plus committed present events | authored canon + kernel | *story time* | the canon past is fixed; present events are appended; changes fork the history DAG |
| **Telling:** each participant's sequence of revelations | kernel, per participant | *discourse time* (when that participant learned it) | appended |

```rust
struct ChronicleEntry { key: StableKey, story_time: StoryTime, location: LocIx,
                        facts: FactRange, commitment: Commitment }
enum   Commitment     { Canon, Open { allowed: ValueSetIx }, Derived }
enum   Effect { /* existing */ ..., Reveal { entry: ChronicleIx }, Learn { fact: FactIx, kind: KnowKind } }
```

- **Canon:** fixed by the author. A recollection can reveal it, never change it. This is how players investigate a fixed past in any order.
- **Open:** the author lists the allowed values, and the kernel fixes one when it is first observed (deferred commitment). This allows "the past is unknown until looked at" without contradictions, because once fixed it is canon for that branch. Use it sparingly; §6 explains why.
- **Derived:** computed from other facts.
- A **recollection** is an ordinary storylet with `Reveal` effects. It can be played by the participant who remembers it, or drawn out of them by another participant (§7).
- The existing **history DAG** handles forks and counterfactuals ("what if I had…"). A forked branch is a *new chronicle*, never an edit of the old one, which matches TENNET §10.4.

### 4.4 Participants: one model for players, playable points of view and NPCs

```rust
struct ParticipantRow { key: StableKey, controller: Controller, location: LocIx,
                        roles: TagBitset, busy_until: Slot }
enum   Controller     { Human { seat: u8 }, Rule, Proposer { adapter: AdapterIx } } // Proposer = LLM/utility, validated
struct KnowledgeRow   { holder: PartIx, fact: FactIx, kind: KnowKind, source_event: EventIx, confidence: u8 }
enum   KnowKind       { Evidence, Witnessed, Told, Rumor, Recollection, Testimony, Inference }
struct TellingRow     { holder: PartIx, entry: ChronicleIx, at_event: EventIx }
```

- **Commands carry the participant ID.** The existing `WorldCommand.actor_id` already does this. `MovePlayer` becomes `Move { participant }`.
- **Simultaneous actions** are validated as transactions. Conflicting write sets, for example two players confronting the same NPC, are resolved by an authored policy. The policies `RejectLater`, `PriorityWins` and `Serialize` are already specified. A scene with a shared actor becomes a **joint scene** with all co-located participants rather than a race.
- **Knowledge transfer is an in-world act.** It can be "compare notes", a letter, an overheard line, or gossip. It always creates an event, and each knowledge row it creates points back to that event.
- **Observation APIs are participant-scoped,** as TENNET FR-02 requires. A client, or an LLM adapter acting for an NPC, receives only that participant's knowledge and surroundings.

### 4.5 The storylet contract, extended

The current fields stay. They are:

- location tags, roles, triggers, choices and effects
- repeatability and cluster type

The additions are:

- `at`: location tag(s), or anywhere
- `window`: world-clock slots in which the storylet is eligible
- `cast`: role slots bound from the participants present, with conditions
- `knower_needs` / `knower_lacks`: knowledge preconditions **on the participant playing it**, not on global flags
- `reveals`: chronicle entries
- `visibility`: who witnesses the outcome (private, co-located, or public via an institution)
- `scope`: once per participant, once per world, or repeatable
- `line_contracts`: optional, per line (§4.8)

### 4.6 Spine: milestones, routes and endings

```yaml
milestone: benedek_identified
routes: [customs_ledger, piece_together, bluff_benedek]   # ≥2 required when resilience is promised
must_be_reachable_for: [solo, coop2]
deadline: before(benedek.departs)
```

Endings stay storylets selected by a profile, as Chrono already specifies. The spine is **data the tools check**, not a script the runtime forces. This follows the canon-event layer in event-type-in-modular-narrative.md and the "story spec" idea from StoryAssembler, but it is used for verification rather than planning.

### 4.7 Director

1. **Hard eligibility in code.** Location, window, cast, knowledge, world state. Never delegated to a model.
2. **Selection profile** per surface, as already specified: priority, salience, weighted or seeded.
3. **Optional soft score** from an adapter. It may *reorder* eligible candidates only, as specified by the two-stage selection backlog and TENNET pillar 4.
4. **Per-participant pacing.** Each participant has a tension and recency budget. In a shared world the director optimizes each participant's next beat only within the shared truth (see §6.5 for the limit).
5. **Story sifting** over the event log turns emergent patterns into authored *recognition* storylets. "Story sifting" is James Ryan's term, and Felt is Max Kreminski's sifter; the sifted pattern is authored, so the director recognizes emergence rather than fabricating it (TENNET tenet 3).

### 4.8 Where an LLM helps, and where it is not allowed

**Authoring time:** the main value, low risk. All of it goes through MCP into **Draft**; approval stays human.

| Assist | Input | Output | Guardrail |
|---|---|---|---|
| Hole filling | coverage report (§4.9) | draft storylet aimed at a specific hole | must pass validation; lands in draft; human edits and approves |
| Variant lines | one authored line + voice guide + line contract | 3–5 expression variants (depth branching: meaning vs. expression) | author picks; variants are stored as authored text |
| Consistency review | storylet + canon chronicle + knowledge graph | list of possible contradictions with citations | advisory only; never edits |
| Persona bots | persona description | traversal policy parameters | runs inside the deterministic explorer; results are reproducible |
| Recap and explanation | a participant's telling + events | "why did this happen" summaries for designers | read-only; cites event IDs |
| Localization drafts | authored text + glossary | translated draft | human review; templated grammar issues remain (§6) |

**Runtime:** optional, out of process, recorded, and each assist has an authored fallback.

| Assist | Contract | Fallback |
|---|---|---|
| **Intent mapping:** free text → one of the *currently eligible* choices or waypoint topics | output ∈ eligible set, with confidence; below the threshold, show the menu | the choice menu |
| **Line realization:** paraphrase or bark inside a *line contract* | `may_mention: [facts in speaker's knowledge]`, `must_not_mention: [secrets]`, tone, length; a validator checks named entities and facts | the authored line |
| **NPC proposer:** suggests an action for a `Controller::Proposer` participant | must be a typed action from the action catalog; the kernel validates it (TENNET FR-01) | the rule-authored NPC behaviour |
| **Soft ranking** among eligible storylets | scores only | deterministic profile |

Every runtime call produces a **decision envelope**: adapter version, prompt/context hash, eligible set, output, accepted or rejected, and fallback used. Replay reads the envelope and never calls the model again (TENNET §10.4).

**An LLM never:**

- creates or changes facts
- decides eligibility
- writes world state
- reveals a canon secret
- approves content
- authors a milestone reveal: key revelations are authored word for word

### 4.9 The verification loop

The explorer runs headlessly in the kernel. It is cheap: the prototype ran 164,000 runs in about 6 seconds in TypeScript. It reports five things.

- **Invariant violations:**
  - knowledge without a source event
  - past known without a telling
  - an occupant beyond a closed edge
  - two contradictory states, e.g. an NPC both "delayed" and "departed"
- **Milestone reachability per participant configuration** (solo, co-op 2, …) and per persona (wanderer, investigator, chatterbox, …)
- **Coverage holes:** situations where a participant plausibly *holds an intent* but no storylet affords it. Each project declares *intent probes*, e.g. "suspects X and is facing X".
- **Dead air:** the share of actions where only fallback content was available at the current location
- **Telling diversity:** how many distinct orders the past was revealed in. This shows whether the time axis is real or nominal.

This loop is what makes non-linearity on three axes authorable. storylet-design-limitation.md §2 identifies precondition fragility and authorial capitulation as the main failure modes; the explorer gives the author a short feedback loop instead of the "Gulf of Evaluation".

### 4.10 The human-first authoring loop

1. Author the world constitution: places, edges and schedules, cast, and properties.
2. Author the canon chronicle: what really happened.
3. Author the spine: milestones, routes, deadlines and endings.
4. Author storylets: scenes, recollections and their line contracts.
5. Run the explorer. Read violations, holes and reachability.
6. *(Optional)* Ask an MCP agent to draft fills for specific holes. Edit them, then approve.
7. Playtest, fork and compare. Go back to step 2–4.

---

## 5. What the system can do

| Capability | How |
|---|---|
| Investigate a fixed past in any order without contradictions | canon chronicle + `Reveal` + per-participant telling. 29 distinct telling orders appeared in the prototype's co-op runs, with 0 violations. |
| Several participants at different places at the same time | participant rows + location-scoped eligibility + transactions |
| Asymmetric knowledge and dramatic irony between players | per-participant knowledge with provenance; transfer only by in-world acts |
| NPCs pursuing their own goals on the world clock, with consequences the player caused but did not choose directly | rule NPCs (existing) + triggers + causal events. Prototype: gossip tipped off the culprit in 22–47% of runs, with a full causal chain. |
| A map that changes with time | scheduled edges + mandatory occupancy rules |
| "What if I had…" | the existing history DAG: fork, reconcile, invoke-to-POI |
| Resilient story progress | declared milestones with ≥2 routes, verified per configuration |
| Free-text input without free-text consequences | intent mapping onto the eligible set |
| Variety in surface text without losing authorial control | variant lines authored with LLM help; optional runtime realization inside line contracts |
| Exact replay with or without an LLM | deterministic kernel + decision envelopes. Prototype: 2,000 re-run seeds, 0 mismatches. |
| Content scaled to one or many participants from one library | role casting + participant-scoped preconditions |

---

## 6. What the system cannot do: hard limits and trade-offs

### 6.1 The time trilemma (hard)

A world cannot have all three of:

- **(a)** participants change the past,
- **(b)** participants can visit or reveal the past in any order,
- **(c)** no contradiction between what was revealed earlier and what is true later.

Any two are possible:

| Keep | Give up | Pattern |
|---|---|---|
| b + c | a | **Fixed past** (default here): recollections reveal canon |
| a + c | b | **Ordered time travel**: the past can be changed only from a forced vantage point, and changes fork the timeline |
| a + b | c | **Contradiction is a feature**: unreliable narrators, dream logic; the author must make it legible |

`Commitment::Open` (deferred commitment) looks like a way out, but it only moves the problem. Once observed, the value is fixed, and every storylet that might fix it must work with every allowed value. The authoring cost grows with |allowed values| × |storylets that can observe it|. **Use it for a handful of facts, not as the default.**

### 6.2 The closed ontology (hard)

The system can only produce consequences the authors anticipated. An LLM can *understand* "I swim to the rock" and map it to the nearest authored intent ("attempt the crossing → the current drives you back"). It cannot add a new consequential verb at runtime without breaking tenet 2 ("Reality is authoritative"). Free text widens **input**, not **outcomes**.

### 6.3 Verification is bounded (hard)

The state space grows multiplicatively with places × slots × participants × knowledge. Exploration finds bugs; it does not prove there are none.

- The prototype explores a 5-place, 6-slot, 4-actor world, and its results are statistical.
- Guarantees exist only for **declared invariants**, and for reachability under the **declared personas**.
- Exhaustive model checking is feasible only for abstracted sub-problems: one milestone, a small cast.

### 6.4 Generated text cannot be proven safe (hard)

A line-contract validator can check named facts and entities. It cannot check implication, subtext or tone. "She always did love the rock at dawn" leaks a secret without naming it. So:

- milestone reveals, secrets and endings are authored word for word;
- runtime generation is limited to low-stakes lines with authored fallbacks.

### 6.5 Shared-world pacing (hard)

In a shared world, one participant's climax happens at the same moment as another participant's quiet stretch. The director can make each participant's *next beat* fit, but it cannot give every participant an ideal arc without either:

- separating them (instancing, which loses shared consequence), or
- bending truth (forbidden).

**Decide per game which you value more.** The prototype shows the tension: in co-op, the best ending E1 needs both players and stays rare.

### 6.6 Expensive but possible (soft limits)

| Limit | Why | Mitigation |
|---|---|---|
| Critical mass of content is still needed | storylet systems feel sparse until the library is dense (time-to-bootstrap) | LLM drafts speed up first drafts; **review cost remains human** |
| Determinism of *text* across model versions | models are not deterministic | replay uses recorded text; regenerating creates a new branch |
| Real-time massive concurrency | scenes with a shared actor are serialized; reservation contention | joint scenes; per-region shards; asynchronous turns |
| Localization of templated or generated text | grammatical agreement (gender, case) | prefer whole-line variants over templates; human review |
| Emergent drama on demand | a simulation can be dull; sifting only finds patterns someone authored | author sift patterns; accept quiet periods (TENNET pillar 4) |
| Per-participant knowledge storage | rows grow with participants × facts | dense fact IDs, bitsets for common kinds, provenance kept only for facts that matter to the narrative |

---

## 7. Narrative example: *Weir's Crossing*

An original, small mystery written for this proposal. It is built to exercise all three axes with the smallest cast that still produces each behaviour.

### 7.1 Premise

During a storm, the light at Weir's Crossing went dark, and the keeper, **Ilse Varga**, vanished. Three days later her daughter **Mara** comes home. The harbour pilot **Teodor** was on the shingle that night. He was drunk, and he blames Ilse. The customs clerk **Benedek** has a ferry ticket for the second evening. **Rosa** at the inn hears everything and repeats most of it.

**Canon truth (authored, fixed):**

- Benedek doused the lamp so an unlit boat could land contraband.
- Ilse rowed out to the outer rock to warn ships off the reef. She is alive there.
- The rock can be reached on foot only at low tide.

### 7.2 Space

```text
        [rock] ── causeway (open only at low tide: D1 evening, D3 morning;
          │                 crossing requires knowing the tide table)
   [lighthouse] ── [harbor] ── [customs]
                       │           │
                     [inn] ────────┘
```

### 7.3 Time

**World clock:** 6 slots (D1–D3, morning and evening), 2 actions per participant per slot.

- Benedek's routine: customs in the mornings, the inn in the evenings.
- He takes the ferry at the end of D2 evening.

**Chronicle, canon past of the storm night:**

| ID | Story time | Place | Fact |
|---|---|---|---|
| P1 | 22:00 | lighthouse | Ilse lights the lamp and logs the storm |
| P2 | 23:00 | harbor | An unlit boat lands crates; a customs lantern on the shingle |
| P3 | 23:30 | lighthouse | The lamp is doused; lamp oil signed out to "B." |
| P4 | 00:30 | sea | Ilse rows for the outer rock |

### 7.4 Participants

| Participant | Solo game | Co-op game | Knows at start |
|---|---|---|---|
| Mara | player | player 1 | nothing |
| Teodor | rule NPC at the harbour | player 2: plays his own recollection | (memory of P2, released by playing it) |
| Benedek | rule NPC with a goal (leave) and a reaction (leave early if named) | same | his guilt |
| Rosa | rule NPC: a rumour hub | same | the ferryman's rumour about a customs lantern (`kind = rumor`) |

### 7.5 Storylets

There are 12. One of them, the bluff, was added in v2 from an LLM draft.

| Storylet | At | Needs (of the participant playing it) | Effect |
|---|---|---|---|
| lamp_soot | lighthouse | — | learn `light_doused` (evidence) |
| ilse_logbook | lighthouse | — | reveal P1; learn `where.rock` (evidence) |
| harbor_master_tides | harbor | — | learn `tide.table` (told) |
| teodor_lantern_memory | harbor | played **by Teodor** | reveal P2; learn `crates_landed`, `benedek.suspected` (recollection); `belief.ilse_neglected` (**testimony, false**) |
| press_teodor | harbor | played by Mara, Teodor present, knows `light_doused` | the same reveal, as testimony |
| customs_ledger | customs | Benedek **absent**, knows `crates_landed` | reveal P3; learn `benedek.identified` (evidence) |
| ask_rosa_openly | inn | — | learn rumour; **Rosa hears your suspicions** |
| listen_quietly | inn | — | learn rumour; Rosa hears nothing |
| piece_together | anywhere | suspected + light_doused + crates_landed | learn `benedek.identified` (inference) |
| confront_benedek | wherever Benedek is | identified | `benedek.delayed`; witnesses learn the confession |
| find_ilse | rock | — | reveal P4; `ilse.rescued`; witnesses learn |
| watch_the_water | anywhere | — | fallback filler |
| **bluff_benedek** *(v2)* | wherever Benedek is | suspected, not identified, plus any one piece of evidence | identified + delayed (witnessed) |

**Milestones:**

- **M1** light doused
- **M2** Benedek identified, before he leaves
- **M3** Ilse found at low tide

**Endings:**

| Ending | Condition |
|---|---|
| **E1** Light Restored | rescued + held |
| **E2** The Keeper Comes Home | rescued |
| **E3** A Ledger With a Name | held |
| **E4** Low Water | neither |

### 7.6 Two real playthroughs

These are copied from the prototype's traces; seeds are shown.

**Co-op, seed 1003 → E1 Light Restored.** Teodor's memory comes first, so the past is told in the order **P2 → P1 → P3 → P4**.

```text
D1 morning  teodor: teodor_lantern_memory @harbor   (reveals P2, false belief about Ilse recorded as testimony)
D1 morning  mara:   ask_rosa_openly @inn            (Mara knows nothing yet: nothing to leak)
D1 evening  teodor: bluff_benedek @inn              (Benedek is at the inn in the evening; Teodor has suspicion + crates)
D2 morning  teodor: ask_rosa_openly @inn            (now Rosa hears "Teodor suspects Benedek")
D2 evening  rosa:   gossips_to_benedek <- #10       (too late: Benedek is already held)
D2 evening  mara:   ilse_logbook @lighthouse        (reveals P1; learns where.rock)
D2 evening  teodor: customs_ledger @customs         (reveals P3)
D3 morning  mara:   move:rock, find_ilse            (low tide; reveals P4)
D3 evening  tide:   drives_mara_back                (authored occupancy rule)
```

**Solo, seed 1011 → E2 The Keeper Comes Home.** Mara starts at the inn, so the past is told **P1 → P4 → P2**. The culprit escapes *because of something she said*:

```text
D1 morning  mara:    listen_quietly @inn     (learns the ferryman's rumour)
D1 morning  mara:    ask_rosa_openly @inn    (repeats it back: Rosa now holds "Mara suspects Benedek")
D1 evening  rosa:    gossips_to_benedek <- #2
D1 evening  benedek: takes_ferry <- #3       (a full day before his ticket)
...
D3 morning  mara:    move:rock, find_ilse
D3 evening  mara:    press_teodor            (P2 arrives last: Teodor's testimony now reads as tragic irony)
```

The causal chain `ask_rosa_openly → gossips_to_benedek → takes_ferry` is stored as event parents. The designer and the player can both ask *"why did he leave early?"* and get a real answer.

### 7.7 LLM assistance in this example

1. **Authoring, hole filling.** The v1 explorer reported 44,775 "intent without affordance" moments in co-op: a participant suspects Benedek and is facing him, but has no way to act on it. An MCP agent read the report and drafted a "Bluff" storylet. The designer tightened its precondition (at least one real piece of evidence, so a rumour alone cannot win) and approved it. Result in §8.
2. **Runtime, Rosa's gossip (optional).** Rosa's line has a line contract:
   - `may_mention: rosa.knowledge`, each fact marked with its source ("so the ferryman says…")
   - `must_not_mention: [ilse.alive, where.rock]`
   - tone: warm, nosy; at most 2 sentences
   - fallback: the authored line

   The validator rejects any output that names a forbidden fact. As §6.4 explains, it cannot catch hints, which is why Rosa never holds `ilse.alive` in her knowledge in the first place. **Structure, not the validator, protects the secret.**
3. **Runtime, intent mapping (optional).** The player types *"Ask Teodor whether my mother let the light die."* The eligible set is {press_teodor, harbor_master_tides, watch_the_water, move…}. The mapper selects `press_teodor` with confidence 0.91, and the decision is recorded. If Mara does not yet know `light_doused`, `press_teodor` is not eligible, so the mapper *cannot* select it and the menu is shown.

### 7.8 What the example cannot do, and why

| Player wants to… | Result | Limit |
|---|---|---|
| Go back to 23:30 and stop Benedek | impossible: recollections reveal, never change | §6.1 (fixed past chosen) |
| Swim to the rock at high tide | intent maps to "attempt the crossing", and the authored outcome turns them back | §6.2 closed ontology |
| Get Rosa to say where Ilse is | Rosa doesn't know it, so no generated line can contain it | §6.4: structure, not validation |
| Have both players experience the climax together while one is still at customs | the director cannot schedule both arcs; one player hears about it afterwards | §6.5 shared-world pacing |
| Trust Teodor's claim that Ilse neglected the light | the journal marks it as *testimony*, and canon contradicts it | belief ≠ truth (by design) |

---

## 8. Prototype evidence

**What it is:** [`examples/proposal-weirs-crossing/explore.ts`](../../examples/proposal-weirs-crossing/explore.ts), about 350 lines of TypeScript run with Bun. It is a *viability prototype*, not engine code; production code belongs in Rust, per AGENTS.md. It implements the proposed contracts in miniature:

- scheduled edges
- participant-scoped knowledge with kinds and source events
- a canon chronicle with `Reveal`
- co-presence witnessing
- rule NPCs on a clock
- an explorer with two personas

**Run it:** `bun examples/proposal-weirs-crossing/explore.ts 20000`

### 8.1 Results

20,000 seeded runs per row. Full output is in `results.txt`.

| Configuration | Persona | E1 | E2 | E3 | E4 | Telling orders | Holes filled by bluff | Violations |
|---|---|---|---|---|---|---|---|---|
| v1 solo | wanderer | 0% | 2.4% | 0% | 97.6% | 11 | — | 0 |
| v1 co-op | wanderer | 0.1% | 4.8% | 4.0% | 91.1% | 29 | — | 0 |
| v2 co-op | wanderer | 0.7% | 4.2% | **21.0%** | 74.2% | 29 | 9,468 / 40,730 | 0 |
| v1 co-op | investigator | 0.8% | 74.1% | 0.7% | 24.4% | 17 | — | 0 |
| v2 co-op | investigator | **3.6%** | 71.1% | 2.5% | 22.9% | 17 | 1,059 / 22,111 | 0 |
| v1/v2 solo | investigator | 0% | 17.9% | **0%** | 82.1% | 9 | 0 | 0 |

**Replay:** 2,000 seeds run twice, 0 mismatches.

### 8.2 What the prototype taught

1. **It found a space-time bug that nobody saw.** On the first run, 1,902 solo runs and 4,745 co-op runs left a participant on the rock after the causeway flooded. The author had attached content to the rock but no rule to the edge closing. The invariant check produced an exact count and a reproducing seed (1002). Adding the occupancy rule brought it to 0, and you can re-run it with `NO_TIDE_RULE=1`. This is why §4.2 makes occupancy rules mandatory.
2. **Reachability depends on participant count.** The justice endings (E1, E3) are effectively unreachable solo, under both personas, even with the bluff: 0 of 20,000 investigator runs. Identifying Benedek solo takes a 4-storylet chain across 3 locations before his D2 ferry. The content is tuned for co-op without anyone having decided that. This is the reason for `must_be_reachable_for: [solo, coop2]` in §4.6. **The fix is a human decision:** add a solo route, or declare E1/E3 co-op-only.
3. **An LLM-drafted, human-tightened storylet has a measurable effect.** In co-op, E3 rose from 4.0% to 21.0% for wanderers and E1 rose from 0.8% to 3.6% for investigators. The draft did almost nothing solo (E3 rose from 0% to 0.1% for wanderers and stayed at 0% for investigators), because its precondition requires a piece of evidence that solo play rarely has in time. That is the authoring loop working as intended: the tool measures, the human decides.
4. **The time axis is real, not nominal.** The same canon past was told in 29 different orders in co-op and 11 solo, with 0 contradictions. Who is playing changes the dominant order: P1 first solo, P2 first co-op. **Participants and time interact**, so they need to be designed together.
5. **NPC agency creates consequences players cause without choosing them.** Gossip tipped Benedek off in 22–47% of runs, depending on the persona. Every case has a complete causal chain back to the player's words.
6. **Dead air is high (40–50%).** Close to half of all actions had only filler available at the current place. In a real game this would feel empty, which is the time-to-bootstrap problem in numbers. The explorer gives the author a direct "where to write next" signal.

**Limits of this evidence:**

- Personas are bots, not players.
- The world is tiny.
- The LLM step was simulated by a designer-written storylet labelled as LLM-drafted; no model was called.
- Enjoyment was not measured. TENNET gate D still requires human playtests.

---

## 9. Mapping onto the codebase

Estimates assume one engineer who knows the codebase. They are rough, ±50%.

| Step | Change | Files | Estimate |
|---|---|---|---|
| 0 | Fix the drift in §2.3: test counts, README playtest claims, stale native example | docs, `examples/native-host/` | 2–3 hours |
| 1 | `Effect::Learn { holder, fact, kind }` + `KnowledgeRow` with `source_event`; keep the `Knows` predicate | `lib.rs`, `storylets.rs` | 1–2 days |
| 2 | Participants: replace `current_location`/`MovePlayer` with `Move { participant }`; participant-scoped observations | `lib.rs`, `runtime.rs`, service contracts | 3–5 days |
| 3 | Witnessing: co-presence at commit → `Learn(kind=Witnessed)` for visible outcomes | `lib.rs`, `storylets.rs` | 1 day |
| 4 | Chronicle: `ChronicleEntry`, `Effect::Reveal`, `TellingRow`; canon authoring in the Codex (world/canon events already exist there) | engine + `src/web/studio/views/CodexView.vue` | 1 week |
| 5 | Scheduled edges + occupancy rules; wire `TriggerTable::process_tick` into the live scheduler for the world clock | `chrono_triggers.rs`, project contracts | 1 week |
| 6 | Spine data (milestones, routes, configurations) + a Rust explorer with personas, invariants, holes and telling diversity; MCP `workspace.coverage` | new `crates/storylet-engine/src/explore.rs`, `mcp.rs` | 1–2 weeks |
| 7 | Simultaneous participants: wire write-set conflict policies; joint scenes | `chrono_transactions.rs`, service | 1–2 weeks |
| 8 | LLM authoring assist: no code in the core; document MCP recipes (hole → draft → validate → human approve) | docs, `docs/guides/` | 2–3 days |
| 9 | *(Optional)* runtime adapter protocol: decision envelopes, line contracts + validator, intent mapping; off by default | new `crates/narrative-adapters/` | 2–3 weeks |

**Recommended order:** 0 → 1 → 3 → 6 → 2 → 4 → 5 → 7 → 8 → 9. Build the explorer early (step 6) so every later step is measured as it lands (TENNET: "No new complexity without a measured increase in product value").

---

## 10. Decisions needed from the designer

1. **Time default:** fixed past (recommended), ordered time travel, or contradiction as a feature?
2. **Participant target for the first vertical slice:** solo with alternating points of view (cheapest), or 2-player co-op (exercises step 7)?
3. **Runtime LLM:** out of scope for the first slice (recommended), or include intent mapping only?
4. **Shared-world pacing priority:** shared consequence, or per-player arcs (§6.5)?

---

## 11. Sources

**Checked this session (title, authors and venue confirmed):**

- Kreminski, M. & Wardrip-Fruin, N. (2018). *Sketching a Map of the Storylets Design Space.* ICIDS 2018, LNCS 11318, pp. 160–164. https://mkremins.github.io/publications/Storylets_SketchingAMap.pdf
- Sun, Y. et al. (2025). *Drama Llama: An LLM-Powered Storylets Framework for Authorable Responsiveness in Interactive Narrative.* arXiv:2501.09099. https://arxiv.org/abs/2501.09099 Uses natural-language triggers; the paper's own limitations informed §6.4.
- Wang et al. (2025). *Dramamancer: Interactive Narratives with LLM-powered Storylets* (UIST 2025 demo; I3D 2026 companion). https://mkremins.github.io/publications/Dramamancer_I3D2026.pdf
- Garbe, J., Kreminski, M., Samuel, B., Wardrip-Fruin, N. & Mateas, M. (2019). *StoryAssembler: An Engine for Generating Dynamic Choice-Driven Narratives.* FDG 2019. doi:10.1145/3337722.3337732. https://mkremins.github.io/publications/StoryAssembler.pdf
- Evans, R. & Short, E. (2014). *Versu — A Simulationist Storytelling System.* IEEE TCIAIG 6(2), 113–130. https://www.cs.uky.edu/~sgware/reading/papers/evans2014versu.pdf
- Ryan, J. et al. *Toward Characters Who Observe, Tell, Misremember, and Lie* (Talk of the Town knowledge model). https://aitopics.org/doc/conferences:EA0682D9; and Kreminski et al., *Gossamer* (CoG 2023). https://mkremins.github.io/publications/Gossamer_CoG2023.pdf

**Referenced but not re-checked this session:** Ryan, *Curating Simulated Storyworlds* (PhD thesis, UCSC 2018), the origin of "story sifting"; Kreminski et al., *Felt: A Simple Story Sifter* (ICIDS 2019); Mateas & Stern, *Façade* (beat-based drama management); Emily Short's storylet cluster types (emshort.blog), already summarised in `docs/design/non-linear-narrative-system.md`.

**Could not identify:** `docs/design/3437378.3442693.pdf` has an ACM DOI prefix, but the web search returned no record and no local PDF renderer was available. It is not cited.
