# TECHNICAL DESIGN DOCUMENT: CHRONOFLOW-NARRATIVE ENGINE
**Document ID:** CFN-SPEC-001  
**Version:** 5.0 (First-Principles Paradigm Edition — DOD Bitset Gameplay Tags, Persistent State & Bijective Operators)  
**Writing Standard:** ASD-STE100 (Simplified Technical English)  
**Target Audience:** Autonomous Code Agents, Systems Architects, Narrative Engineers  

---

## 1. FIRST-PRINCIPLES RE-EVALUATION AND PARADIGM SELECTION

### 1.1 The Problem with Mashing Object-Oriented Subsystems Together
In earlier iterations, the system treated the **Layered World Graph**, **Qualities**, **NPC Passions**, and **Storylet Preconditions** as separate object-oriented managers with heap-allocated string maps (`Map<string, AttributeValue>`, `TArray<FGameplayTag>`). 

As demonstrated by Jacek Miąskiewicz (*Data Oriented Programming with Gameplay Tags*, Flying Wild Hog), standard `FGameplayTagContainer` implementations store dynamic heap arrays of explicit tags and separate dynamic arrays of parent tags per container. This causes pointer-chasing cache misses when querying thousands of entities or storylets per frame.

### 1.2 Reduction to Core First Principles
When we strip away domain-specific labels, every requirement in our six sources reduces to **four physical and mathematical realities**:

1. **Identity & State Membership is a Bitset (`TagBitset`):** Whether a fact is a Location, a Tool Quality, an Unlocked Clue, a Dramatic Arc Phase, or a Discrete Entity State, it is fundamentally a hierarchical **Gameplay Tag** mapped to a dense integer index $i \in [0, N-1]$. Replacing heap-allocated tag arrays with **Fixed-Size Inline Bitsets** (`uint64[K]`) and precomputed **Shared Hierarchy Masks** makes precondition matching a single CPU bitwise `AND` instruction ($\approx 20\times$ faster across 200,000+ containers).
2. **Continuous State is a Contiguous Float Vector (`MetricVector`):** Currencies, Skills, Menaces, Star Micro-Passions, Macro-Passions, and Waypoint Edge Weights are all continuous `float32` values indexed by Gameplay Tag IDs in a flat **Structure-of-Arrays (SoA)** table. Zero-Sum Passion propagation is simply a **Sparse Matrix-Vector Multiplication** over this array.
3. **Every Transition is a Bijective Delta Operator ($\Delta$):** We do not need different mechanisms to reverse a Graph Rule, a Quality change, a Passion shift, or a Waypoint step. By **Reversible Computing** principles, every state change is an atomic reversible operator:
   $$\text{Apply}(S_t, \Delta) \to S_{t+1} \quad \text{and} \quad \text{Invert}(S_{t+1}, \Delta) \to S_t$$
   Where $\Delta$ consists solely of a **Bitwise XOR Mask** (for tag additions/removals) and an **Index-Value Undo Tuple** (for float overwrites).
4. **Branching & Co-op History is a Pure Functional Persistent DAG:** Instead of mixing in-node mutable stacks with global arrays, the Call History is an append-only **Persistent Directed Acyclic Graph (DAG)** of invoked nodes and their $\Delta$ records. Rolling back a Mission, jumping choice branches, or syncing 4-player Co-op is a pure function: find the **Lowest Common Ancestor (LCA)** in the DAG, apply $\text{Invert}(\Delta)$ up to the LCA, and apply $\text{ForceComplete}(\Delta)$ down to the target leaf.

### 1.3 Selected Programming Paradigms

| System Layer | Chosen Paradigm | Why It Fits First Principles |
| :--- | :--- | :--- |
| **1. World State, Qualities & Tags** | **Data-Oriented Design (DOD) & Bit-Vector Programming** | Stores all entity tags in inline fixed-size bitsets (`uint64[4]`) and all numerical qualities/passions in cache-aligned flat arrays indexed by Tag ID. |
| **2. Precondition, QBN & Salience Matching** | **Array / SIMD Vector & Constraint Logic** | Evaluates storylet eligibility across thousands of candidates using branchless bitwise operations (`(StateBits & ReqMask) == ReqMask`) and SIMD Euclidean distance. |
| **3. State Mutation & Rollback** | **Reversible Computing & Pure Functional Persistent DAG** | Every mutation produces an invertible $\Delta$ (`XOR` bitmask + prior float value). World state is a pure projection of the Call-History DAG. |
| **4. Flow Graph & Scene Execution** | **Dataflow Token Processing** | Fixed-size tokens flow through a flat node table on an integer millisecond clock, evaluating half-open time windows $[t_0, t_1)$ forward or $(t_1, t_0]$ backward. |

---

## 2. SYSTEM ARCHITECTURE OVERVIEW (STREAMLINED CORE)

```
+===================================================================================+
|                      CHRONOFLOW-NARRATIVE ENGINE (v5.0 CORE)                      |
+===================================================================================+
|                                                                                   |
|  [1. STATIC GAMEPLAY TAG REGISTRY (Immutable Shared Lookup Tables)]               |
|   ├── Tag String -> Dense uint16 Index (0 .. MAX_TAGS - 1)                        |
|   ├── Shared Hierarchy Closure Masks (Parent/Child Bitsets per Tag Index)         |
|   └── Sparse Zero-Sum Coupling Matrix (Rival Passion & Menace Weights)            |
|                                                                                   |
|  [2. DATA-ORIENTED WORLD STATE TABLE (Contiguous SoA Memory Block)]               |
|   ├── Global & Per-Entity Inline TagBitsets (uint64[4] = 256 Hierarchical Tags)   |
|   ├── Continuous Metric/Passion Array (float32[MAX_METRICS] indexed by Tag ID)    |
|   └── Relational Adjacency BitMatrix (Layer 0..3 Edges encoded as Tag Bitsets)    |
|          ▲                                                       │                |
|          │ (Applies Bijective Delta:                             │ (Bitwise AND + |
|          │  Bits ^= Mask, Float[i] = Val)                        ▼  SIMD Distance)|
|  [3. UNIFIED STORYLET & FLOW DISPATCHER (Dataflow + Term Rewriting)]              |
|   ├── Branchless Bitset Gate Evaluator (Unifies Flow Gates, QBN & Salience)       |
|   ├── Waypoint Topic Pathfinder (Dijkstra over Tag-Indexed Topic Matrix)          |
|   └── Fixed-Point Token Ring-Buffer & Scrubbable [t0, t1) Timeline Sorter         |
|          ▲                                                       │                |
|          │ (Unwinds / Replays Delta Frames)                      │ (Appends       |
|          │                                                       ▼  Commit Node)  |
|  [4. PERSISTENT CALL-HISTORY DAG & PATCH REBASER]                                 |
|   ├── Immutable Commit Nodes (ParentHash, NodeGUID, ChosenTag, BijectiveDelta)    |
|   └── Pure Functional Reconciler: Unwind(Current -> LCA) + Invoke(LCA -> Target)  |
+===================================================================================+
```

---

## 3. MODULE 1: DATA-ORIENTED GAMEPLAY TAG BITSETS & UNIFIED STATE TABLE

### 3.1 Requirements (Jacek Miąskiewicz DOD Pattern)
* **REQ-DOD-01 (Dense Tag Indexing):** At initialization, the system SHALL compile all hierarchical Gameplay Tags (`Location.*`, `Character.*`, `Quality.*`, `Passion.*`, `Clue.*`, `ArcPhase.*`, `Topic.*`) into dense `uint16` indices in the range $[0, N_{tags}-1]$.
* **REQ-DOD-02 (Fixed-Size Inline Bitsets):** The system SHALL replace heap-allocated Gameplay Tag containers with a fixed-size inline bitset structure (`TagBitset`) composed of $W$ 64-bit unsigned integers (`uint64 words[W]`). No dynamic memory allocation SHALL occur when adding, removing, or querying tags.
* **REQ-DOD-03 (Shared Hierarchy Bitmasks):** The system SHALL NOT store parent tag arrays inside individual tag containers. Instead, the static `GameplayTagRegistry` SHALL precompute two read-only bitset lookup tables:
  1. `AncestorsMask[tag_id]`: Bits set for `tag_id` and all of its parent tags (used when adding a tag or checking exact/parent inclusion).
  2. `DescendantsMask[tag_id]`: Bits set for `tag_id` and all of its child sub-tags (used to test hierarchical queries such as `HasAny(Tag.Location.Kitchen)` in a single bitwise `AND`).
* **REQ-DOD-04 (Unified Quality & Passion Array):** All numerical state variables—including QBN `Currencies`, `Metrics`, `Progress`, `Menaces`, and Ken Levine's `MicroPassions` and `MacroPassions`—SHALL reside in a single contiguous `float32` array (`metrics[]`) indexed by `TagIndex`.
* **REQ-DOD-05 (Matrix-Driven Zero-Sum & Macro-Passion Propagation):** Star NPC Macro-Passions and Zero-Sum Rival links SHALL be stored as a static Compressed Sparse Row (CSR) weight matrix. Whenever a `Passion.*` metric changes by $\delta$, the system SHALL propagate updates across connected Zero-Sum rivals and recalculate Macro-Passions in a single linear pass.

### 3.2 Data-Oriented Structures

```python
# --- 1. FIXED-SIZE INLINE GAMEPLAY TAG BITSET (ZERO HEAP ALLOCATION) ---

const WORD_COUNT: int32 = 4            # 4 * 64 = 256 Tags per container (configurable at compile time)
const MAX_METRICS: int32 = 512         # Total numerical Qualities + Passions

struct TagBitset:
    words: uint64[WORD_COUNT]          # Inline contiguous storage (32 bytes)

    function SetBit(index: uint16) -> void:
        words[index >> 6] |= (1ULL << (index & 63))

    function ClearBit(index: uint16) -> void:
        words[index >> 6] &= ~(1ULL << (index & 63))

    function ApplyXOR(mask: TagBitset) -> void:
        # Bijective toggle: applying the same XOR mask twice restores the exact original bits
        for w from 0 to (WORD_COUNT - 1):
            words[w] ^= mask.words[w]

    function HasAll(required: TagBitset) -> bool:
        for w from 0 to (WORD_COUNT - 1):
            if (words[w] & required.words[w]) != required.words[w]:
                return false
        return true

    function HasAny(filter: TagBitset) -> bool:
        for w from 0 to (WORD_COUNT - 1):
            if (words[w] & filter.words[w]) != 0ULL:
                return true
        return false

    function MatchesQuery(required_all: TagBitset, forbidden_any: TagBitset) -> bool:
        # Evaluates complex precondition in 4 CPU cycles with zero pointer chasing
        for w from 0 to (WORD_COUNT - 1):
            if (words[w] & required_all.words[w]) != required_all.words[w]:
                return false
            if (words[w] & forbidden_any.words[w]) != 0ULL:
                return false
        return true


# --- 2. SHARED HIERARCHICAL TAG REGISTRY ---

struct ZeroSumLink:
    target_metric_id: uint16           # Rival Passion or Macro-Passion Tag ID
    weight: float32                    # e.g., -1.0 for Zero-Sum rival, +0.33 for Macro contribution

struct MacroThresholdRule:
    star_macro_id: uint16
    threshold_tag_bit: uint16          # Tag bit set in WorldState when threshold is active
    trigger_value: float32
    hysteresis: float32                # Prevents rapid toggling (Ken Levine trust inertia)
    spawned_subgraph_id: uint32

class GameplayTagRegistry (Immutable Singleton):
    tag_to_id: Map<string, uint16>
    ancestors_mask: TagBitset[MAX_TAGS]   # Precomputed parent closure per tag
    descendants_mask: TagBitset[MAX_TAGS] # Precomputed child closure per tag
    
    # Sparse adjacency list for Zero-Sum Passions and Macro-Passion aggregation
    coupling_links: List<ZeroSumLink>[MAX_METRICS]
    threshold_rules: List<MacroThresholdRule>

    function HasHierarchicalTag(container: TagBitset, parent_tag_id: uint16) -> bool:
        # 20x faster hierarchical check: tests parent and all children via shared mask
        return container.HasAny(descendants_mask[parent_tag_id])


# --- 3. CONTIGUOUS WORLD STATE BLACKBOARD (SoA) ---

struct WorldStateTable:
    # A. Qualitative State (Tools, Clues, Active Arc Phase, Discovered Passions, Active Thresholds)
    global_tags: TagBitset
    
    # B. Per-Entity Tag Containers (Locations, Characters, Items in flat SoA array)
    entity_tags: TagBitset[MAX_ENTITIES]
    
    # C. Quantitative State (Currencies, Metrics, Progress, Menaces, Micro/Macro Passions)
    metrics: float32[MAX_METRICS]
```

---

## 4. MODULE 2: BIJECTIVE DELTA OPERATORS (UNIFIED REVERSIBLE COMPUTING)

### 4.1 First-Principles Simplification of Rollback
Previously, each node class (`Node_SetQuality`, `Node_QBNStoryletHub`, `Node_WaypointConversation`) maintained its own custom history stack. By applying **Reversible Computing**, *every* state mutation in the engine reduces to a single universal data structure: `BijectiveDelta`.

1. **Tag Mutations are Self-Inverting via `XOR`:**
   If a state has tag bitset $B_{old}$ and a rule changes it to $B_{new}$, the delta is $M_{xor} = B_{old} \oplus B_{new}$.
   * **Forward Application:** $B_{new} = B_{old} \oplus M_{xor}$
   * **Backward Reversal:** $B_{old} = B_{new} \oplus M_{xor}$ (identical instruction!).
2. **Float Metric Mutations Store Exact Prior Values:**
   To avoid floating-point rounding drift during reversal, any change to `metrics[id]` records `(id, prior_float32)` in a small inline tuple buffer before writing the new value.

### 4.2 Requirements
* **REQ-REV-01:** Every mutation to `WorldStateTable` SHALL execute through `ApplyMutationSpec()` and return a `BijectiveDelta` record.
* **REQ-REV-02:** Calling `RevertBijectiveDelta(state, delta)` SHALL restore `global_tags`, `entity_tags`, and `metrics` to their exact bit-for-bit prior values in $O(k)$ time, where $k$ is the number of modified fields.

### 4.3 Algorithm: Unified Forward Mutation and Bijective Reversal

```python
struct MetricUndoPair:
    metric_id: uint16
    old_value: float32

struct EntityMaskPair:
    entity_id: uint16
    xor_mask: TagBitset

struct BijectiveDelta:
    global_xor_mask: TagBitset
    entity_xor_masks: List<EntityMaskPair>
    metric_undos: List<MetricUndoPair>

struct MutationOp:
    add_global_tags: TagBitset
    remove_global_tags: TagBitset
    metric_deltas: List<Pair<uint16, float32>> # (metric_id, +/- delta)

class ReversibleStateEngine:

    function ApplyMutation(
        state: WorldStateTable, 
        op: MutationOp, 
        registry: GameplayTagRegistry
    ) -> BijectiveDelta:
        
        delta = BijectiveDelta()

        # 1. Compute new Global Tag Bitset and store XOR mask
        old_tags = state.global_tags
        new_tags = old_tags
        for w from 0 to (WORD_COUNT - 1):
            new_tags.words[w] = (old_tags.words[w] | op.add_global_tags.words[w]) & (~op.remove_global_tags.words[w])

        # 2. Apply Metric & Passion Deltas + Zero-Sum Matrix Propagation
        for each (m_id, change) in op.metric_deltas:
            RecordAndAddMetric(state, delta, m_id, change)
            
            # Propagate across Zero-Sum Rival edges & Macro-Passion weights
            for each link in registry.coupling_links[m_id]:
                RecordAndAddMetric(state, delta, link.target_metric_id, change * link.weight)

        # 3. Evaluate Macro-Passion & Menace Thresholds into Tag Bits
        for each rule in registry.threshold_rules:
            val = state.metrics[rule.star_macro_id]
            was_active = new_tags.HasBit(rule.threshold_tag_bit)
            if not was_active and val >= rule.trigger_value:
                new_tags.SetBit(rule.threshold_tag_bit)
            else if was_active and val < (rule.trigger_value - rule.hysteresis):
                new_tags.ClearBit(rule.threshold_tag_bit)

        # 4. Commit Global Tag XOR Mask
        for w from 0 to (WORD_COUNT - 1):
            delta.global_xor_mask.words[w] = old_tags.words[w] ^ new_tags.words[w]
        state.global_tags = new_tags

        return delta

    function RecordAndAddMetric(state: WorldStateTable, delta: BijectiveDelta, id: uint16, change: float32):
        # Only record the FIRST old_value per metric_id within a single atomic transaction
        if not delta.HasRecordedMetric(id):
            delta.metric_undos.Add(MetricUndoPair(id, state.metrics[id]))
        state.metrics[id] = Clamp(state.metrics[id] + change, -100.0, 100.0)

    function RevertDelta(state: WorldStateTable, delta: BijectiveDelta) -> void:
        # 1. Self-inverting XOR restores exact TagBitset state (including threshold bits)
        state.global_tags.ApplyXOR(delta.global_xor_mask)
        
        for each pair in delta.entity_xor_masks:
            state.entity_tags[pair.entity_id].ApplyXOR(pair.xor_mask)

        # 2. Restore exact prior floating-point values (Zero float drift)
        for each undo in delta.metric_undos:
            state.metrics[undo.metric_id] = undo.old_value
```

---

## 5. MODULE 3: UNIFIED BEYOND-BRANCHING & DATAFLOW ENGINE

### 5.1 First-Principles Unification of QBN, Salience, Waypoint, and Flow Nodes
By using `TagBitset` and `metrics[]`, we eliminate redundant node machinery:
* **Dramatic Arc Phases, Tools, Clues, and Completed Storylets are all bits in `global_tags`!**
  * Instead of a separate `completed_non_repeatable_storylets` hash set, each non-repeatable storylet has a `Storylet.Completed.<Name>` tag bit.
  * Instead of a separate `required_arc_phases` integer set, Dramatic Arc Phases are tags (`Arc.Phase1_Setup`, `Arc.Phase2_Rising`, `Arc.Phase3_Climax`) checked inside the same 4-cycle `MatchesQuery()` bitset instruction!
* **QBN, Salience, and Waypoint are Pure Selection Functions over Storylet Rows:**
  * **QBN:** Returns all rows where `MatchesQuery(state.global_tags)` and `MatchesMetricGates(state.metrics)` are true, sorted by priority.
  * **Salience:** Filters eligible rows with the exact same bitwise check, then scores each passing row by `PopCount(state.global_tags & storylet.salience_bonus_tags)` minus the SIMD Euclidean distance to `ideal_metrics`.
  * **Waypoint:** Represents conversational topics as tag indices and edge reuse penalties as entries in `state.metrics[]`—meaning edge reuse penalties are **automatically rolled back by `RevertDelta()`** without custom Waypoint undo code!

### 5.2 Requirements
* **REQ-BB-01 (Branchless Bitset Preconditions):** Every `StoryletDef` SHALL compile its tag preconditions (including Dramatic Arc Phase gates, required clues/tools, and non-repeatability locks) into two `TagBitset` masks: `required_tags` and `forbidden_tags`.
* **REQ-BB-02 (SIMD Salience Scoring):** The Salience score of a candidate storylet SHALL be computed as:
  $$S(c) = w_{spec} \cdot \text{PopCount}\left(\text{global\_tags} \ \& \ c.\text{bonus\_tags}\right) - \alpha \sqrt{\sum_{j \in c.\text{ideal}} \left(\text{metrics}[j] - c.\text{ideal}[j]\right)^2}$$
* **REQ-BB-03 (Unified Node Execution Table):** Every Flow Graph node SHALL be a lightweight data row in a `FlowGraphAsset` with a `NodeCategory` (`MUTATOR`, `WAITER`, `LOGIC_ALL`, `LOGIC_ANY`, `QBN_HUB`, `SALIENCE_ROUTER`, `WAYPOINT_GRAPH`, `SCENE_SECTION`, `MISSION_SCOPE`, `POI`).

### 5.3 Data Structures & Algorithms (DOD Storylet & Waypoint Table)

```python
struct MetricGate:
    metric_id: uint16
    min_val: float32
    max_val: float32

struct IdealMetricTarget:
    metric_id: uint16
    ideal_val: float32
    weight: float32

struct StoryletDef:
    storylet_id: uint16
    display_priority: int16
    
    # Compiled 32-byte Bitset Masks (Includes Clues, Tools, Arc Phase, and Repeat-Lock bit)
    required_tags: TagBitset
    forbidden_tags: TagBitset
    salience_bonus_tags: TagBitset     # Optional tags that boost specificity via hardware POPCNT
    
    metric_gates: List<MetricGate>
    ideal_vector: List<IdealMetricTarget>
    on_complete_mutation: MutationOp   # Automatically sets 'Storylet.Completed.X' bit if non-repeatable
    payload_node_guid: uint32

class UnifiedDispatcher:
    storylets: StoryletDef[MAX_STORYLETS] # Contiguous array for cache-friendly scanning

    function IsEligible(s: StoryletDef, state: WorldStateTable) -> bool:
        # 1. Fast Bitwise Filter (Checks Clues, Arc Phase, and Completed Lock in 1 step)
        if not state.global_tags.MatchesQuery(s.required_tags, s.forbidden_tags):
            return false
        # 2. Numeric Metric Range Check
        for each gate in s.metric_gates:
            val = state.metrics[gate.metric_id]
            if val < gate.min_val or val > gate.max_val:
                return false
        return true

    function QueryQBNDeck(candidate_ids: Span<uint16>, state: WorldStateTable) -> List<uint16>:
        out_list = List<uint16>()
        for each id in candidate_ids:
            if IsEligible(storylets[id], state):
                out_list.Add(id)
        SortByPriorityDescending(out_list)
        return out_list

    function MatchMostSalient(candidate_ids: Span<uint16>, state: WorldStateTable, seed: uint64) -> uint16:
        best_id = INVALID_ID
        highest_score = -Infinity

        for each id in candidate_ids:
            s = storylets[id]
            if not IsEligible(s, state):
                continue

            # Hardware POPCNT instruction counts matched specificity bonus tags
            matched_bonus_bits = 0
            for w from 0 to (WORD_COUNT - 1):
                matched_bonus_bits += HardwarePopCount64(state.global_tags.words[w] & s.salience_bonus_tags.words[w])

            dist_sq = 0.0
            for each target in s.ideal_vector:
                diff = state.metrics[target.metric_id] - target.ideal_val
                dist_sq += target.weight * (diff * diff)

            score = (matched_bonus_bits * 10.0) - Sqrt(dist_sq)
            if score > highest_score:
                highest_score = score
                best_id = id

        return best_id
```

---

## 6. MODULE 4: SCRUBBABLE TIME-WINDOW SCENE COMPOSER (DATAFLOW)

### 6.1 Requirements (CD Projekt RED Token & Time-Window Pattern)
* **REQ-SC-01:** Scene timelines SHALL store events in a contiguous array sorted by `start_time_ms`.
* **REQ-SC-02:** Forward execution ($dt \ge 0$) SHALL evaluate the half-open interval $[t_0, t_1)$. Backward scrubbing ($dt < 0$) SHALL evaluate $(t_1, t_0]$ and swap each event's target payload with its `cached_prior_payload_tag` (e.g., restoring the previous actor Workspot pose).
* **REQ-SC-03:** Overlapping events SHALL be decomposed into `ActionPart` structs, sorted by `(priority_key, target_entity_id)`, and dispatched to the `SceneComposer`.

---

## 7. MODULE 5: PURE FUNCTIONAL PERSISTENT CALL-HISTORY DAG & PATCH REBASER

### 7.1 First-Principles Unification of Save/Load, Co-op Sync, and Choice Branching
By combining **Dying Light 2's Call History** ("The Story State defines the World State") with **Pure Functional Persistent Data Structures** (Git-like immutable DAG nodes):
* Every step in the story appends an immutable `CommitNode` pointing to its `parent_commit_index`.
* Each `CommitNode` stores:
  1. `node_guid`: Which Flow Graph node completed.
  2. `chosen_branch_or_storylet`: Which output pin or QBN/Salience `storylet_id` was taken.
  3. `delta`: The `BijectiveDelta` produced by that step.
* **Why a Persistent DAG Solves Everything:**
  * **Save File Serialization:** When writing to disk or replicating over the network for 4-player Co-op, the engine **strips `delta`** and only serializes `(node_guid, chosen_branch_or_storylet)`. Replaying those calls from index `0` reconstructs both the `WorldStateTable` and the runtime `delta` records deterministically!
  * **1-Frame Timeline Reconciliation (Branch Jumping & Co-op Sync):** Switching from Commit $A$ to Commit $B$ (whether rolling back a choice, rewinding a failed `Mission` sub-graph to its start commit, or syncing a client to a Co-op host) is a single pure algorithm:
    1. Find `LCA = LowestCommonAncestor(Commit_A, Commit_B)`.
    2. Walk parent pointers from `Commit_A` down to `LCA`, calling `RevertDelta(state, commit.delta)`.
    3. Walk forward from `LCA` to `Commit_B`, calling `ForceCompleteNode(state, commit.node_guid, commit.chosen_branch_or_storylet)`.

### 7.2 Algorithms: Pure Functional Timeline Reconciler & Hybrid Story Invoke

```python
struct CallCommit:
    commit_id: uint32
    parent_commit_id: uint32           # Forms a Persistent Tree/DAG of alternate player timelines
    node_guid: uint32
    chosen_pin_or_storylet: uint16
    delta: BijectiveDelta              # Runtime-only inverse cache (omitted from disk save files!)

class PersistentHistoryEngine:
    commits: List<CallCommit>          # Append-only arena of timeline commits
    head_commit_id: uint32 = 0

    # --- 1. UNIFIED ROLLBACK / BRANCH JUMP / CO-OP SYNC ---
    function CheckoutCommit(
        state: WorldStateTable, 
        graph: FlowGraphInstance, 
        target_commit_id: uint32
    ) -> void:
        
        lca_id = FindLCA(head_commit_id, target_commit_id)

        # Step A: Unwind current branch back to LCA using BijectiveDeltas
        curr = head_commit_id
        while curr != lca_id:
            commit = commits[curr]
            ReversibleStateEngine.RevertDelta(state, commit.delta)
            graph.SetNodeState(commit.node_guid, IDLE)
            curr = commit.parent_commit_id

        # Step B: Collect path from LCA to target_commit_id and ForceComplete forward
        forward_path = GetPathFromAncestor(lca_id, target_commit_id)
        for each step in forward_path:
            step.delta = graph.ForceCompleteNode(state, step.node_guid, step.chosen_pin_or_storylet)

        head_commit_id = target_commit_id


    # --- 2. 1-FRAME HYBRID STORY INVOKE (WIRES + BITSET QBN PREREQUISITES) ---
    function InvokeStoryToPOI(
        state: WorldStateTable,
        graph: FlowGraphInstance,
        target_poi_guid: uint32,
        declared_any_routes: Map<uint32, uint16>
    ) -> void:
        
        plan = List<Pair<uint32, uint16>>() #Ordered (node_guid, chosen_pin_or_storylet)
        visited = Set<uint32>()

        function SeekBackward(guid: uint32):
            if visited.Contains(guid) or graph.IsNodeCompleted(guid):
                return
            visited.Add(guid)

            node = graph.GetNode(guid)
            incoming = graph.GetIncomingEdges(guid)

            # Logic ALL (AND) pulls all branches; Logic ANY (OR) pulls declared route pin
            selected_edges = (node.category == LOGIC_ALL) 
                ? incoming 
                : FilterByPin(incoming, declared_any_routes.GetOrDefault(guid, 0))

            for each edge in selected_edges:
                SeekBackward(edge.source_guid)
                src_node = graph.GetNode(edge.source_guid)
                
                if not graph.IsNodeCompleted(edge.source_guid):
                    if src_node.category == QBN_HUB:
                        # Bitwise Goal Chaining: Find deck storylets whose add_global_tags 
                        # set the missing required_tags bits of the hub's exit storylet!
                        exit_storylet = Dispatcher.storylets[src_node.hub_exit_storylet_id]
                        missing_bits = exit_storylet.required_tags & (~state.global_tags)
                        
                        for each deck_s_id in src_node.deck_storylet_ids:
                            deck_s = Dispatcher.storylets[deck_s_id]
                            if deck_s.on_complete_mutation.add_global_tags.HasAny(missing_bits):
                                plan.Add(Pair(src_node.guid, deck_s_id))
                        plan.Add(Pair(src_node.guid, src_node.hub_exit_storylet_id))
                    else:
                        plan.Add(Pair(edge.source_guid, edge.source_pin_id))

        SeekBackward(target_poi_guid)
        plan.Add(Pair(target_poi_guid, 0))

        # Execute synthesized plan in 1 frame
        for each (guid, choice) in DeduplicatePreserveOrder(plan):
            delta = graph.ForceCompleteNode(state, guid, choice)
            AppendCommit(guid, choice, delta)
```

---

## 8. REFERENCE MAPPING TO UNREAL ENGINE (`MothCocoon/FlowGraph`)

| Abstract v5.0 Component | Unreal Engine 5 / `FlowGraph` Mapping | DOD Optimization Note |
| :--- | :--- | :--- |
| `TagBitset` (`uint64[4]`) | Custom `FFixedGameplayTagBitset` struct | Replaces `FGameplayTagContainer` inside narrative hot loops ($20\times$ faster queries). Bridges to `FGameplayTagContainer` at editor boundaries. |
| `GameplayTagRegistry` | `UGameplayTagsManager` + `UChronoTagTable` | Precomputes `AncestorsMask` and `DescendantsMask` bitsets on Asset Registry load. |
| `WorldStateTable` | `UChronoWorldSubsystem` (Flat SoA Buffers) | Stores `FFixedGameplayTagBitset` and `TArray<float>` metrics contiguously. |
| `BijectiveDelta` | `FChronoBijectiveDelta` | Uses `XOR` bitmasks and `(uint16, float)` undo pairs; zero UObject overhead. |
| `PersistentHistoryEngine` | `TArray<FChronoCallCommit>` | Serializes only `(FGuid, uint16)` to `USaveGame`; reconstructs `BijectiveDelta` on load. |

---

## 9. VERIFICATION AND BENCHMARK SUITE

1. **Test Suite 1: Bitset Tag Throughput Benchmark (`BENCH_TAG_BITSET_DOD`)**
   * **Procedure:** Populate 200,000 `TagBitset` containers with 1 to 16 random hierarchical tags and evaluate 1,000 storylet `MatchesQuery` + `HasHierarchicalTag` checks.
   * **Acceptance Criterion:** Execution MUST allocate `0 bytes` on the heap and achieve $\ge 15\times$ speedup over dynamic-array tag containers.
2. **Test Suite 2: Bijective XOR & Float Reversal Identity (`TEST_BIJECTIVE_IDENTITY`)**
   * **Procedure:** Hash the entire `WorldStateTable` (`Hash_0`). Apply 10,000 random `MutationOp` transactions (including Zero-Sum rival passion ripples, Macro-Passion threshold crossings, QBN clue bits, and Waypoint edge penalties). Call `CheckoutCommit(0)` to unwind all 10,000 commits. Compute `Hash_Final`.
   * **Acceptance Criterion:** `Hash_Final == Hash_0` bit-for-bit across all bitsets and IEEE-754 floats.
3. **Test Suite 3: 1-Frame Bitwise QBN Story Invoke (`TEST_BITWISE_QBN_INVOKE`)**
   * **Procedure:** Configure an investigation hub where `Storylet.AccuseMurderer` requires `TagBitset` bits `{Clue.Means, Clue.Motive, Clue.Opportunity}`. Run `InvokeStoryToPOI(POI_End)` from a fresh state.
   * **Acceptance Criterion:** The backward seeker MUST compute `missing_bits = required_tags & (~global_tags)`, automatically select and complete the three clue storylets via bitwise intersection, and reach `POI_End` in $< 1.0\text{ ms}$.