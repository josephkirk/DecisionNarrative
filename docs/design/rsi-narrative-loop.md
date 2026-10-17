In language-model-driven and procedural storytelling, **"slop"** is the systemic tendency of generative models toward generic, low-friction, sycophantic, and structurally ungrounded content. In non-linear narrative systems (such as storylets, Quality-Based Narratives [QBN], and state-tree dialogue graphs), narrative slop manifests in specific ways:
*   **Prose Slop:** Over-explication, cliché metaphors, generic trope defaults (e.g., "Aria", "celestial", "dark secrets"), and a lack of subtext.
*   **Logical Slop:** Hallucinated state mutations, unfulfilled preconditions, orphaned nodes, and "No Path Found" dead ends.
*   **Agency Slop:** The illusion of choice, where distinct player options lead to identical state changes or sycophantic character compliance without meaningful pushback.

To build a **Recursive Self-Improvement (RSI) Agent System** (utilizing RLAIF, DPO/PPO policy updates, MCMC sampling, or SMT/ASP constraint evaluation loops) capable of autonomously generating and refining non-linear narrative, we must formalize "unslop" into **calculable scoring functions and reward goals**.

Below is a first-principles breakdown of the **6 core score goals** required for an RSI agent to evaluate, filter, and iteratively improve non-linear narratives.

---

### The RSI Composite Reward Function
For an RSI agent loop, the total reward \\(R_{\text{total}}\\) for a candidate narrative fragment or storylet cluster \\(N\\) is defined as a weighted sum of structural, dramatic, and ludonarrative objective scores:

\\[R_{\text{total}}(N) = \alpha S_{\text{causal}} + \beta S_{\text{choice}} + \gamma S_{\text{pushback}} + \delta S_{\text{prose}} + \epsilon S_{\text{tension}} + \zeta S_{\text{ludo}} - P_{\text{hard}}\\]

*(Where \\(P_{\text{hard}}\\) is an absolute penalty function for hard logic failures like dead ends or state invalidations).*

---

### 1. Causal Graph Integrity & Solvability Score (\\(S_{\text{causal}}\\))

#### First-Principle Foundation
A non-linear narrative is fundamentally a **bounded state-space traversal problem**. In modular storylet systems, narrative starvation ("No Path Found") occurs when a player reaches a game state where no available storylet's preconditions evaluate to `TRUE`. Slop systems generate narrative beats that sound coherent in isolation but break the logical continuity of the underlying state machine.

#### Operational Metrics for RSI
*   **Precondition-Effect Satisfiability Rate:** Evaluates whether every generated storylet's preconditions can be satisfied by at least one valid trajectory through preceding storylets' effects.
*   **Orphan & Dead-End Detection:** Uses SMT (Satisfiability Modulo Theories) or ASP (Answer Set Programming) solvers to verify that no non-ending node terminates execution without presenting a valid divert or choice path.
*   **State Delta Consistency:** Penalizes storylets that mutate state variables that are not declared in the global Quality taxonomy or that violate variable bounds (e.g., negative inventory, uninitialized flags).

\\[\text{Formula: } S_{\text{causal}} = 1.0 - \left( \frac{\text{Unreachable Storylets} + \text{Dead-End States}}{\text{Total State Space Nodes}} \right)\\]

---

### 2. Consequential Choice Delta & Divergence Score (\\(S_{\text{choice}}\\))

#### First-Principle Foundation
Player agency in interactive narrative relies on **consequential state mutation**. Slop systems frequently employ "false choices" (CYOA illusion), where multiple choice labels lead to the exact same prose and state modifications. Unslop non-linear narrative requires that choices yield distinct immediate reactions and divergent downstream systemic impacts.

#### Operational Metrics for RSI
*   **State Mutation Divergence (Jaccard Distance):** Measures the semantic and mathematical distance between the effect vectors of choices within a single storylet node.
*   **Downstream Reincorporation Horizon:** Tracks whether a quality mutated by a choice in Scene \\(A\\) is explicitly read as a precondition or variable template in Scene \\(B\\) (\\(N\\) turns later).
*   **Choice Meaning vs. Expression Ratio:** Ensures that options do not merely vary surface text ("tone slop") but execute distinct operations on the World State (e.g., consuming currencies, altering faction standings, incrementing menaces).

\\[\text{Formula: } S_{\text{choice}} = \frac{1}{|C|} \sum_{c_i \neq c_j \in C} D_{\text{Jaccard}}(\text{Effects}(c_i), \text{Effects}(c_j)) \times \text{ReincorporationFactor}(c_i)\\]

---

### 3. Character Autonomy & Non-Sycophancy Score (\\(S_{\text{pushback}}\\))

#### First-Principle Foundation
Large Language Models exhibit an inherent alignment bias toward sycophancy—they tend to validate whatever the player suggests, collapsing narrative tension. Lifelike non-linear storytelling requires NPC autonomy, where characters maintain persistent goal/belief states and exert genuine friction, resistance, or betrayal when player actions contradict their interests.

#### Operational Metrics for RSI
*   **Role-Identity & Goal Consistency:** Evaluates whether NPC dialogue and actions strictly align with their authored character prompt and belief/desire state vector across multi-turn interactions (measuring consistency against character profiles).
*   **Friction & Pushback Index:** Scores the frequency with which NPCs reject, counter-offer, or react hostilely to player actions that violate their internal motivations, rather than passively complying.
*   **Subtext-to-Exposition Ratio:** Penalizes dialogue where NPCs explicitly state their internal emotional mechanics or plot facts ("telling"), rewarding dialogue where motivation is conveyed implicitly through subtext.

\\[\text{Formula: } S_{\text{pushback}} = \text{CosineSim}(\text{NPC\_Action}, \text{NPC\_BeliefState}) \times \left( 1.0 - \text{SycophancyIndex} \right)\\]

---

### 4. Lexical & Semantic Anti-Triteness Score (\\(S_{\text{prose}}\\))

#### First-Principle Foundation
LLMs operating autoregressively draw heavily from the center of their probability distributions, generating predictable, formulaic prose ("trite content"). In storylet systems, this results in repetitive scene openings and canned phrasing. Unslop narrative demands high semantic specificity, concrete spatial details, and stylistic variance.

#### Operational Metrics for RSI
*   **Slop Corpus N-Gram Overlap:** Measures \\(k\\)-gram overlap (\\(k=3,4,5\\)) against a curated negative dataset of AI narrative clichés (e.g., *"testament to..."*, *"delicate dance"*, *"shivers down the spine"*, *"Aria"*).
*   **Lexical Diversity (Type-Token Ratio / TTR):** Calculates the ratio of unique terms to total words within a storylet payload to ensure vibrant, non-repetitive vocabulary.
*   **Concrete Grounding Ratio:** Evaluates the presence of specific, sensory, and world-grounded entities (e.g., named locations, items, physical actions) versus abstract conceptual fluff.

\\[\text{Formula: } S_{\text{prose}} = \text{TTR} \times \left(1.0 - \text{Overlap}_{\text{SlopCorpus}}\right) \times \text{Density}_{\text{ConcreteEntities}}\\]

---

### 5. Macro-Dramatic Pacing & Tension Alignment Score (\\(S_{\text{tension}}\\))

#### First-Principle Foundation
Without structural orchestration, modular storylets risk feeling like a chaotic collection of disconnected vignettes. Narrative mechanics must adhere to macro-level pacing—managing rising action, climaxes, and recovery cycles.

#### Operational Metrics for RSI
*   **Tension Slope Delta (\\(\Delta T\\)):** Scores candidate storylets based on how closely their actual tension effect matches the target slope of the current dramatic act (using Aristotelian curve fitting or Façade's beat sequencer algorithm).
*   **Cluster Pattern Integrity:** Verifies that content selection respects structural cluster types—ensuring high-intensity *Aftermaths* follow major turning points, and *Wheels/Decks* manage downtime and recovery without interrupting climactic sequences.
*   **Opposing Clock Progression:** Evaluates whether narrative timers and menace accumulators (e.g., *Suspicion* vs. *Investigation Progress*) increment at rates that heighten precariousness as the player approaches boundary conditions.

\\[\text{Formula: } S_{\text{tension}} = \frac{1}{e^{|\text{Slope}_{\text{Target}} - \Delta T_{\text{Beat}}|}}\\]

---

### 6. Ludonarrative State-Coupling Score (\\(S_{\text{ludo}}\\))

#### First-Principle Foundation
Ludonarrative dissonance occurs when written prose is completely detached from underlying gameplay mechanics. Unslop narrative achieves ludonarrative convergence by making prose a dynamic surface layer of underlying game variables (qualities, inventory, world states).

#### Operational Metrics for RSI
*   **Parameterization Density:** Measures the ratio of dynamic variable templates (e.g., `$suspect`, `$location`, `$item`) and conditional text switches within a storylet relative to static text.
*   **Fungible Quality Leverage:** Evaluates how well a storylet leverages fungible qualities (stats that hold transferable meaning across multiple quests/regions) to maximize content reusability without creating hyper-specific authoring silos.
*   **Verb-Mechanical Alignment:** Ensures that the player choices presented in a storylet directly map to core gameplay verbs (e.g., trading, stealth, combat) and execute valid operations within the game engine.

\\[\text{Formula: } S_{\text{ludo}} = \frac{\text{Count}(\text{Templated Variables}) + \text{Count}(\text{Systemic Quality Checks})}{\text{Total Word Count}}\\]

---

### Summary Rubric for RSI Agent Training Loops

| Metric Goal | Evaluator Engine | Primary Slop Failure Mode Prevented |
| :--- | :--- | :--- |
| **1. Causal Integrity (\\(S_{\text{causal}}\\))** | SMT / ASP Solvers + Planner | "No Path Found" dead ends, orphaned nodes, broken logic. |
| **2. Consequential Choice (\\(S_{\text{choice}}\\))** | Quality Delta Analysis | Illusion of choice, flat branching, zero-consequence options. |
| **3. Character Autonomy (\\(S_{\text{pushback}}\\))** | Role-Play LLM Judge + Utility Maps | Character sycophancy, lack of friction, over-explication. |
| **4. Anti-Triteness (\\(S_{\text{prose}}\\))** | N-gram Overlap & Lexical Profiler | Canned prose, repetitive tropes, "stochastic parrot" fluff. |
| **5. Pacing & Tension (\\(S_{\text{tension}}\\))** | Drama Manager / Tension Arc Fitting | Erratic scene order, ruined dramatic momentum, flat pacing. |
| **6. Ludonarrative Coupling (\\(S_{\text{ludo}}\\))** | Templating Parser & Variable Audit | Written narrative detached from systemic gameplay variables. |

---

### Implementation in an RSI Loop
When an RSI agent attempts to self-improve narrative generation (e.g., fine-tuning a Small Language Model or updating a storylet dispatcher policy):

1. **Generation:** Candidate storylet fragments or scene structures are synthesized.
2. **Hard-Gate Filtering:** The candidate is passed through symbolic verifiers (SMT/ASP logic engines). If \\(S_{\text{causal}}\\) detects a dead end or precondition contradiction, the candidate is discarded instantly with a hard penalty \\(P_{\text{hard}}\\).
3. **Soft-Reward Optimization:** Valid candidates are evaluated against the soft-scoring matrix (\\(S_{\text{choice}}, S_{\text{pushback}}, S_{\text{prose}}, S_{\text{tension}}, S_{\text{ludo}}\\)) using an LLM-as-a-Judge paired with heuristic statistical profilers.
4. **Policy Improvement (RLAIF/DPO):** The loss gradient optimizes the generator to maximize high-scoring non-linear structures, forcing the model out of generic probability wells and into dense, highly reactive, unsloped narrative design.

1. **Mathematical Formalization:** Writing exact Python/Pytorch loss functions or reward-modeling code for any of these 6 scoring metrics.
2. **Synthetic Dataset Generation:** Designing an instruction dataset (similar to *TinyStories-Instruct* or *Drama Llama* prompts) to train an evaluator LLM judge.
3. **Dispatcher Integration:** Connecting these scoring goals directly to an storylet narrative framework.
