# TECHNICAL DESIGN DOCUMENT: SYSTEM 1 DECISION ARCHITECTURE FOR NON-LINEAR NARRATIVE ENGINES VIA QUALITY-BASED STORYLETS

---

## 1. EXECUTIVE SUMMARY & FIRST-PRINCIPLE ARCHITECTURE

Non-linear interactive drama requires balancing meaningful player agency with globally coherent dramatic pacing. For decades, interactive narrative architectures relied on symbolic planning, state-space search, finite state machines, or Markov Decision Processes to guide story progression. While classical drama management frameworks offer guaranteed structural bounds and high authorial control, they suffer from brittle authoring overhead and severe combinatorial explosion when scaling to complex story environments.

The integration of Large Language Models (LLMs) into interactive narrative introduced unprecedented flexibility and dynamic dialogue generation. However, employing standard generative LLMs directly as narrative drama directors introduces severe structural vulnerabilities. Generative models are inherently non-deterministic, prone to output formatting failures, computationally expensive, and characterized by high inference latency (often 1,500ms to 5,000ms+) that disrupts real-time interactive gameplay loops. Furthermore, using generative text to drive discrete narrative logic forces developers to construct extensive parsing and error-handling code to extract usable decision branches from unstructured text outputs.

```
TRADITIONAL GENERATIVE LLM DRAMA DIRECTORS (FLAWED PIPELINE):
[World State] ──> [Monolithic Prompt] ──> [Generative LLM (1500-5000ms)] ──> [Unstructured Text] ──> [Complex Parsing Code] ──> (Risks Format Error / Hallucination)

SYSTEM 1 DECISION ARCHITECTURE (TYPE-SAFE PROBABILISTIC PIPELINE):
[QBN World State] ──> [Compact JSON Payload] ──> [System 1 JEV Engine (70-500ms)] ──> [Typed Schema Outputs] ──> [Direct QBN Storylet Dispatch]
```

To resolve this fundamental design conflict, this document specifies a **System 1 Decision Architecture** utilizing typed, probabilistic decision models—specifically JEV-class models—integrated with a **Quality-Based Narrative (QBN) Storylet System**. 

Borrowing from dual-process cognitive theory, where System 1 represents fast, intuitive judgments and System 2 represents slow, deliberate reasoning, a System 1 decision engine strips away prose generation, explanations, and rationale to execute fast, schema-bound probabilistic evaluations directly over structured application states. By decoupling **structural narrative reasoning** (deciding what storylet or beat should execute next) from **surface realization** (generating text or executing pre-authored dialogue), this architecture achieves sub-second dispatch latency (70ms to 500ms), 100% schema validation, and real-time dramatic pacing without hallucination risks.

---

## 2. SYSTEM ARCHITECTURE & THE CORE NARRATIVE TRIAD

The System 1 Non-Linear Narrative Engine decomposes into three decoupled operational layers:

```
+-----------------------------------------------------------------------------------+
|                            LAYER 1: SYMBOLIC WORLD STATE                          |
|  (Quality Economy: Currencies, Metrics, Tools, Progress Qualities, Menaces)       |
+-----------------------------------------------------------------------------------+
                                          │
                                          ▼
+-----------------------------------------------------------------------------------+
|                       LAYER 2: SYSTEM 1 JEV DRAMA DIRECTOR                        |
|  (Parallel Typed Evaluation Pass: Choice, Score, & Noul Primitives over State)    |
+-----------------------------------------------------------------------------------+
                                          │
                                          ▼
+-----------------------------------------------------------------------------------+
|                        LAYER 3: QUALITY-BASED STORYLET DECK                        |
|  (Atomic Narrative Modules: Preconditions, Payload, State Mutations, Lifecycle)   |
+-----------------------------------------------------------------------------------+
```

### 2.1 Layer 1: Symbolic World State & The Quality Economy
The game state is maintained as a persistent, centralized database of variables called **Qualities**. Qualities serve as a dynamic narrative filter, gatekeeping access to content based on player history and systemic actions. The quality economy is categorized into five functional types:
1. **Currencies:** Consumable resources required to take specific narrative branches (e.g., *Gold*, *Favors*).
2. **Metrics:** Permanent skill levels or attributes gating specialized options (e.g., *Watchful*, *Dangerous*).
3. **Tools:** Unlocked items or key facts that grant unique story options without necessarily being consumed (e.g., *Vault Key*, *Proof of Betrayal*).
4. **Progress Qualities:** Integers or phase counters tracking milestone advancement along major narrative spines or quest arcs.
5. **Menaces:** Threat accumulators tracking danger, risk, or mental degeneration (e.g., *Suspicion*, *Nightmares*). When a menace reaches a critical threshold, it triggers high-priority "crisis storylets".

### 2.2 Layer 2: The Quality-Based Storylet Deck
The narrative content is structured as an unsnapped library of atomic narrative modules called **storylets**. Each storylet encapsulates three core components:
* **Preconditions (Gates):** Logical predicates evaluating world qualities, location tags, or character relationships to determine node availability.
* **Content Payload:** The narrative material presented—ranging from branching dialogue choices, templated text, or voiceover barks to full interactive scenes.
* **Effects (Mutations):** State modifications executed upon node completion, updating qualities, adjusting menaces, or altering world flags.
* **Lifecycle Policies:** Metadata defining whether a storylet is `once_ever`, endlessly repeatable, or subject to cooldown timers.

### 2.3 Layer 3: System 1 JEV Dispatcher Engine
The Content Selection Architecture (the Dispatcher) replaces traditional manual search algorithms or heavy generative LLM prompts with a System 1 decision pass. It continuously receives compact JSON state payloads from Layer 1, evaluates active candidate storylets from Layer 2 against authorial drama criteria, and outputs typed, schema-valid storylet selections and pacing adjustments at ultra-low latencies.

---

## 3. FORMAL SPECIFICATION OF SYSTEM 1 JEV DECISION MECHANICS

System 1 JEV decision models operate by replacing open-ended natural language prompt engineering with three strictly typed primitive question types:

```
+-----------------------------------------------------------------------------------+
|                         SYSTEM 1 JEV PRIMITIVE TYPES                              |
+-------------------+---------------------------------------------------------------+
| PRIMITIVE TYPE    | OUTPUT STRUCTURE & MATHEMATICAL BEHAVIOR                      |
+-------------------+---------------------------------------------------------------+
| 1. CHOICE         | Selects 1 option from up to 255 discrete candidates.          |
|                   | Returns a continuous probability distribution across all       |
|                   | options + an overall system confidence score.           |
+-------------------+---------------------------------------------------------------+
| 2. SCORE          | Maps state onto an ordinal scale (2 to 10 qualitative levels).|
|                   | Interpolates a continuous real-valued rating between points   |
|                   | + probability distribution + confidence metric.          |
+-------------------+---------------------------------------------------------------+
| 3. NOUL           | Evaluates a binary proposition (True/False).                  |
|                   | Returns float probability P in representing truth       |
|                   | likelihood; P equals certainty score.                   |
+-------------------+---------------------------------------------------------------+
```

### 3.1 Primitive Question Definitions
1. **Choice Primitives:** The engine selects a single option from an author-defined array of up to 255 discrete candidate storylets or director actions. JEV evaluates all candidate options in parallel, returning a continuous probability distribution across the candidate array alongside a system confidence score.
2. **Score Primitives:** The engine maps the current game state onto an ordered ordinal scale comprising between 2 and 10 qualitative levels defined in natural language. Rather than locking to an integer index, JEV interpolates a continuous real-valued rating between defined scale points (e.g., outputting a target tension rating of `6.4` on a 1.0–10.0 scale).
3. **Noul Primitives:** The engine evaluates binary propositions (true/false). The output is a single float probability value \\(P \in\\) representing the truth likelihood of the statement, where \\(P\\) is identically equal to the certainty score.

### 3.2 Parallel Single-Pass Execution
Instead of running multiple sequential LLM calls, a JEV drama director processes narrative state through a **single, parallel inference pass** containing multiple typed questions. In a single 70–500ms pass, the engine simultaneously submits:
* A **Score Question** rating required dramatic tension adjustments on a 1.0–10.0 scale.
* A **Choice Question** selecting the optimal storylet or director action from valid candidates (e.g., `cause_npc_confession`, `deny_library_exit`, `hint_secret_passage`).
* A **Noul Question** returning the exact probability that conditions for triggering a major Act climax are satisfied.

```json
{
  "state_payload": {
    "location": "bakery_cellar",
    "turn_count": 14,
    "qualities": {
      "investigation_stage": 3,
      "suspicion_menace": 72,
      "baker_relationship": -15
    },
    "recent_player_actions": ["searched_flour_sacks", "accused_baker_of_forgery"],
    "active_candidate_storylets": ["stl_baker_confession", "stl_guard_interruption", "stl_cellar_trapdoor"]
  },
  "jev_parallel_query": {
    "target_tension_score": { "primitive": "Score", "scale": [1.0, 10.0], "instruction": "Rate ideal dramatic tension based on suspicion and turn count." },
    "selected_storylet": { "primitive": "Choice", "candidates": ["stl_baker_confession", "stl_guard_interruption", "stl_cellar_trapdoor"] },
    "act_climax_ready": { "primitive": "Noul", "instruction": "Is the investigation stage sufficient for Act climax transition?" }
  }
}
```

---

## 4. TWO-TIERED ORCHESTRATION MODEL (MACRO VS. MICRO AI)

To achieve real-time performance without sacrificing physical NPC believability or narrative structure, the architecture adopts a **two-tiered orchestration pattern**:

```
+-----------------------------------------------------------------------------------+
|                        MACRO AI DIRECTOR (System 1 / JEV)                         |
|  - Privileged global access to player qualities, menaces, & tension curves. |
|  - Operates at sub-second intervals issuing high-level pacing jobs.         |
+-----------------------------------------------------------------------------------+
                                         │
                                         ▼
+-----------------------------------------------------------------------------------+
|                        MICRO AI LAYER (Local NPC Execution)                       |
|  - Sensorially constrained local agents driven by Behavior Trees/ABL.  |
|  - Executes physical movement, VO barks, and surface realization text. |
+-----------------------------------------------------------------------------------+
```

1. **Macro AI Director (System 1 JEV Engine):** Possesses privileged global access to player qualities, narrative call history, menace levels, and world state. Operating at sub-second intervals, the Macro Director does not control character physical movement frame-by-frame; instead, it selects active storylet decks, sets pacing constraints, and issues high-level narrative commands.
2. **Micro AI Layer (Local Execution Agents):** Consists of sensorially constrained NPC agents governed by local behavior trees, A Behavior Language (ABL), or utility systems. Micro agents receive the Macro Director's high-level storylet orders and execute them locally in the game world using believable physical actions, animations, or dialogue barks.

---

## 5. TECHNICAL PRODUCTION PIPELINE (5-PHASE WORKFLOW)

The runtime execution pipeline follows a strict 5-phase loop to ensure low-latency, deterministic, and secure storylet dispatch:

```
  [1. TRIGGER PHASE] ──> [2. STATE ASSEMBLY] ──> [3. PARALLEL JEV PASS] ──> [4. CONFIDENCE ROUTING] ──> [5. COMPOSITE VALIDATION]
   (Ticks / Events)      (Token-Minimized JSON)    (Sub-500ms Inference)    (High/Med/Low Bands)       (Hard Gate Checks)
```

### Phase 1: Trigger Phase
The system initiates a drama evaluation pass based on two types of world triggers:
* **Temporal Triggers:** Periodic execution ticks (e.g., re-evaluating ambient tension and menace levels every 5 seconds).
* **Event Triggers:** Discrete event signals driven by player actions (e.g., entering a room, discovering evidence, selecting a key choice, or reaching a quality threshold).

### Phase 2: State Assembly
The engine queries Layer 1 (Symbolic World State) and Layer 2 (Storylet Database) to assemble a compact JSON state payload. Non-essential engine markup, asset file paths, and raw spatial telemetry are filtered out to minimize token footprint and focus JEV's evaluation purely on narrative state variables.

### Phase 3: Parallel JEV Execution
The assembled state payload and author-defined primitive questions (Choice, Score, Noul) are transmitted to the JEV decision model. JEV executes the parallel classification pass, returning typed probability distributions and confidence scores in a single execution.

### Phase 4: Confidence Banding & Decision Routing
The application processes JEV's continuous outputs against author-defined confidence bands:
* **High Confidence Band (Confidence \\(\ge 0.85\\)):** Triggers direct, automated execution. The storylet engine immediately executes the chosen storylet, mutating game qualities and updating world state.
* **Medium Confidence Band (\\(0.50 \le \text{Confidence} < 0.85\\)):** Triggers contextual mitigation. The engine applies subtle interventions, such as dropping an ambient NPC rumor bark or granting a hint, rather than forcing a major structural act shift.
* **Low Confidence Band (Confidence \\(< 0.50\\)):** Triggers safe fallback execution. The system maintains the current baseline narrative trajectory, avoids abrupt interventions, and logs the state payload for human authorial review.

### Phase 5: Adversarial Defense & Composite Validation
Because user dialogue or input logs could contain indirect prompt injection, narrative state payloads strictly isolate untrusted player text into designated, non-executable string fields. Furthermore, mission-critical story gates require **composite validation**—combining JEV's probabilistic routing signal with hardcoded boolean quality checks—ensuring that prompt injection cannot bypass mandatory narrative preconditions.

---

## 6. COMPARATIVE ANALYSIS OF DRAMA MANAGEMENT PARADIGMS

Deploying a System 1 JEV decision engine over Quality-Based Storylets bridges the historical gap between rigid symbolic architectures and unpredictable open-ended generative language models:

| Paradigm | Underlying Mechanism | Latency & Execution Time | Output Determinism & Validation | Authorial Leverage & Scalability | Replayability & Emergence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Search-Based Drama Management (SBDM / DODM)** | Game-tree expectimax search over discrete plot graphs. | Exponential with search depth \\(O(b^d)\\); severe horizon limits. | 100% deterministic code branching; zero formatting errors. | Low leverage; author must manually define all plot points, preconditions, and weights. | Moderate; strictly constrained by static search heuristics. |
| **Targeted Trajectory Distribution (TTD-MDP)** | Online policy search matching target path distributions \\(P(T)\\). | Sub-second via offline policy caching or local approximations. | Strictly bounded by target trajectory space definitions. | Moderate leverage; relies on authoring exemplar trajectories and distance metrics. | High; explicitly optimizes for target path distribution across replays. |
| **Generative LLM Director** | Next-token text generation via large neural networks. | High latency (1,500ms to 5,000ms+); breaks frame-rate pacing. | Non-deterministic; frequent format errors, markdown wrapping, and type mismatches. | High textual flexibility, but low structural reliability; prone to hallucination. | High emergence, but high risk of narrative derailment or logic breakage. |
| **System 1 Decision Engine (JEV + QBN)** | Parallel probabilistic evaluation pass over typed schema questions. | Ultra-low latency (70ms to 500ms); enables continuous micro-pacing. | 100% schema-valid; probabilistic outputs bounded by author schema. | High leverage; author defines typed schema and storylets, letting JEV handle state classification. | High; probability distributions adapt smoothly to unexpected player quality variations. |

---

## 7. LUDONARRATIVE CONVERGENCE & DRAMATIC PACING

### 7.1 Continuous Pacing via Jevons Paradox
The naming of TypeSafe AI's decision model references **Jevons Paradox**: making a resource drastically faster and cheaper vastly increases overall consumption rather than reducing it. In classical interactive drama, drama management calls were compute-heavy operations executed sparingly at major chapter transitions. By reducing individual narrative decisions to sub-half-second execution windows at negligible compute cost, JEV enables **continuous drama management**. Rather than evaluating story direction once per act, a JEV director continuously monitors player stress, menace stats, and local spatial context on a second-by-second basis.

### 7.2 Aristotelian Tension Curve Fitting
The JEV Score primitive continuously evaluates current game qualities against an ideal Aristotelian dramatic arc. By scoring candidate storylets based on how closely their anticipated tension deltas \\(\Delta T_{\text{beat}}\\) align with the target tension slope \\(\text{slope}_{\text{target}}\\), the dispatcher guides the narrative along a satisfying emotional contour:

\\[\text{Score} = \frac{1}{e^{|\text{slope}_{\text{target}} - \Delta T_{\text{beat}}|}}\\]

Player choices that alter menace qualities (e.g., increasing *Suspicion*) cause deviations from the target arc. JEV detects these deltas on subsequent ticks and dynamically selects storylets with opposing tension deltas to restore balance or force climactic confrontation.

```
DRAMATIC TENSION ARC FITTING:
Tension Level
     ▲                                                [Act Climax: JEV Score = Max]
     │                                                     / \
     │                                                    /   \
     │                               [Rising Action]     /     \
     │                                  /  \            /       \
     │               [Inciting Event]  /    \          /         \ [Resolution]
     │                    / \         /      \________/           \
     └───────────────────┴───┴───────┴─────────────────────────────┴──────────► Play Time
     Target Arc:  ─────── Ideal Curve
     Actual Play: ─────── Dynamic Deviations (Adjusted continuously by JEV Score Primitives)
```

### 7.3 Hybrid System 1 / System 2 Future Outlook
The future of non-linear interactive storytelling relies on combining fast **System 1 decision models** with deliberate **System 2 generative models**:
* **System 1 (JEV Engine):** Functions as the real-time narrative router, continuously evaluating incoming state, monitoring tension curves, enforcing authorial QBN preconditions, and directing NPC behavior modes at high frequencies.
* **System 2 (Generative LLMs):** Invoked selectively and asynchronously *only* when open-ended natural language generation is required—such as rendering complex dynamic NPC dialogue or generating in-game letters—operating entirely within the structural guardrails established by the System 1 director.

This hybrid architecture resolves the historical tension between authorial intent and player autonomy, delivering structural coherence, rapid response times, and deterministic reliability alongside personal player agency in modern non-linear narrative experiences.

---
