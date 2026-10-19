# TECHNICAL DESIGN DOCUMENT: CHRONOFLOW-NARRATIVE ENGINE

**Document ID:** CFN-SPEC-001

**Version:** 4.0 (Engine-Agnostic Code Agent Specification — First-Class QBN, Salience & Waypoint Edition)

**Writing Standard:** ASD-STE100 (Simplified Technical English)

**Target Audience:** Autonomous Code Agents, Systems Architects, Narrative Engineers

## 1. PURPOSE AND SCOPE

### 1.1 System Purpose

This document specifies the architecture, data structures, and algorithms for the **ChronoFlow-Narrative Engine**. The system executes non-linear interactive narratives, systemic character relationships, dynamic storylet selection, and time-synchronized scenes. The system guarantees 100% deterministic world-state reversal and choice rollback.

### 1.2 Architectural Pillars

The system integrates six core engineering principles into one unified runtime engine:

1. **Layered Attributed World Graphs:** The system stores the game world state as a formal graph with four distinct semantic layers and mutates this state through reversible subgraph transformation rules.

2. **Unified Ludic and Narrative Agency:** Player gameplay actions (combat, movement, item interaction) and dialogue choices use the same signal pipeline to mutate the world graph.

3. **Zero-Sum Passion Economy ("Narrative Legos"):** Non-Player Characters (NPCs) use micro-passions, macro-passion bars, and zero-sum relational links to trigger modular narrative sub-graphs dynamically when values cross thresholds.

4. **First-Class Beyond-Branching Storylet Dispatchers (QBN, Salience, and Waypoint):** Rather than relying only on static wire connections, the engine natively dispatches modular content through three dynamic mechanisms defined by Emily Short:

   * **Quality-Based Narrative (QBN):** The system filters a pool of unsnapped storylets against World Qualities (`Currencies`, `Metrics`, `Tools`, `Progress`, `Menaces`) and presents eligible storylets for player selection.

   * **Salience-Based Narrative (SBN):** The system scores candidate storylets or dialogue barks by condition specificity and multidimensional numerical state-vector distance (protected by dramatic-arc phase gates) and automatically executes the best-fitting match.

   * **Waypoint Narrative (Topic-Graph Pathfinding):** NPCs dynamically pathfind across a directed graph of conversational topics or scene beats toward an active narrative goal while the player diverts the current topic node to unlock hidden branches.

5. **Token-Driven Latent Scene Execution:** A fixed-point millisecond clock moves fixed-size tokens through graph nodes and evaluates half-open time windows to play scenes forward or backward.

6. **Call-History State Stack ("Story Defines World"):** The system saves an ordered log of node and storylet invocations instead of heavy entity snapshots. Each mutator node caches its previous values internally to allow 1-frame rollbacks, fast-forward story invokes, and multiplayer timeline synchronization.

### 1.3 Engine Independence

This specification does not depend on a specific game engine. A code agent can implement this specification from scratch in any object-oriented or data-oriented language (such as C++, Rust, or C#) or adapt it as an extension to existing node-graph frameworks.

## 2. DEFINITIONS AND GLOSSARY (ASD-STE100 COMPLIANT)

* **Graph Asset:** A static, immutable data definition of nodes, pins, and edges.

* **Graph Instance:** An active runtime execution of a Graph Asset.

* **Storylet Asset:** A self-contained modular narrative unit with preconditions, a content graph or bark payload, and world-state effects.

* **Quality:** A typed numerical or boolean variable in the World Blackboard classified as a `CURRENCY`, `METRIC`, `TOOL`, `PROGRESS`, or `MENACE`.

* **Salience Score:** A deterministic floating-point value that measures how closely a Storylet Asset matches the current World Blackboard state and dramatic arc phase.

* **Waypoint Graph:** A directed graph of narrative topics or scene beats where edge weights increase after traversal to prevent repetitive NPC pathfinding.

* **Signal Token:** A fixed-size data packet that moves from an output pin to an input pin to trigger node execution.

* **World Blackboard:** The central database that holds the Layered World Graph, all Qualities, and Star NPC Passions.

* **Call History:** An ordered list of completed node and storylet invocations that defines the current state of the story and the world.

* **Story Invoke:** An operation that finds a path from the active nodes (or satisfied quality preconditions) to a target node and completes all steps on that path in one frame.

* **Rollback:** An operation that reads the Call History in reverse order and executes the inverse function of each node.

* **Star NPC:** A major character with individual micro-passions and a calculated macro-passion bar.

* **Drone NPC:** A minor character whose disposition value equals the average macro-passion of the Star NPCs in the same faction or location.

## 3. SYSTEM ARCHITECTURE OVERVIEW

```
+===================================================================================+
|                         CHRONOFLOW-NARRATIVE ENGINE CORE                          |
+===================================================================================+
|                                                                                   |
|  [1. WORLD BLACKBOARD, QUALITIES & LAYERED GRAPH DATABASE]                        |
|   ├── Layer 0: Locations   (Zones, Rooms, Portals)                                |
|   ├── Layer 1: Characters  (Star NPCs, Drone NPCs, Zero-Sum Passion Edges)        |
|   ├── Layer 2: Items/Tools (Currencies, Metrics, Tools, Inventory Edges)          |
|   └── Layer 3: Facts/Arcs  (Progress Qualities, Menaces, Dramatic Arc Phase)      |
|          ▲                                                       │                |
|          │ (Mutates State & Caches Old Value)                    │ (Evaluates     |
|          │                                                       ▼  Preconditions)|
|  [2. FIRST-CLASS BEYOND-BRANCHING DISPATCHER (QBN / SALIENCE / WAYPOINT)]         |
|   ├── QBN Deck Manager     (Filters Storylets by Quality Gates -> Player Choice)  |
|   ├── Salience Matcher     (Specificity + Ideal Vector Distance + Arc Gate)       |
|   └── Waypoint Navigator   (Dynamic Dijkstra Topic Pathfinding + Edge Penalties)  |
|          ▲                                                       │                |
|          │ (Requests Dynamic Link)                               │ (Spawns /      |
|          │                                                       ▼  Routes Token) |
|  [3. TOKEN-DRIVEN FLOW GRAPH & SCRUBBABLE SCENE RUNTIME]                          |
|   ├── Fixed-Point Clock (Integer Milliseconds) & Active Token Ring-Buffer         |
|   ├── Atomic Node Library (Mutators, Waiters, Logic Gates, QBN/Salience Routers)  |
|   └── Scene Composer & Interrupter (Resolves Actor Conflicts & Gameplay Breaks)   |
|          ▲                                                       │                |
|          │ (Replays / Unwinds Node & Storylet Calls)             │ (Appends       |
|          │                                                       ▼  Invocations)  |
|  [4. CHRONO CALL-HISTORY & RECONCILIATION ENGINE]                                 |
|   ├── Global Call History Stack (Records Node GUIDs, Chosen Pins & Storylet IDs)  |
|   ├── Scoped Mission History Stacks (Isolated Sub-Graphs for Local Rewind)        |
|   ├── Hybrid Backward Path-Seeker (1-Frame Story Invoke across Wires & Qualities) |
|   └── Timeline Reconciler (LCA Rollback + Invoke for Branch Jumps & Co-op Sync)   |
+===================================================================================+

```

## 4. MODULE 1: LAYERED WORLD GRAPH, QUALITIES, AND PASSION BLACKBOARD

### 4.1 Requirements

* **REQ-WB-01:** The World Blackboard SHALL store world entities as vertices (`WorldVertex`) and relationships as directed edges (`WorldEdge`) across four layers: `LOCATION`, `CHARACTER`, `ITEM`, and `FACT`.

* **REQ-WB-02:** Every graph transformation rule SHALL define a Left-Hand Side pattern (`LHS_Pattern`) to match and a Right-Hand Side mutation (`RHS_Mutation`) to apply.

* **REQ-WB-03:** When the system applies a graph transformation rule, the executing node SHALL record the removed vertices, removed edges, and overwritten attribute values in a local cache to permit exact reversal.

* **REQ-WB-04:** Each Star NPC SHALL contain 1 to $N$ Micro-Passions. Each Micro-Passion SHALL have a visibility flag (`is_hidden`) and a weight (`weight`).

* **REQ-WB-05:** The system SHALL calculate the Macro-Passion of a Star NPC as the clamped weighted sum of its Micro-Passions plus a trust inertia modifier.

* **REQ-WB-06:** When a player action changes a Micro-Passion, the system SHALL propagate inverse changes across all Zero-Sum Edges connected to that Micro-Passion.

* **REQ-WB-07:** When a Star NPC Macro-Passion crosses a defined Threshold (upward or downward), the system SHALL emit a trigger event to activate or cancel the associated Graph Asset ("Narrative Lego").

* **REQ-WB-08:** The World Blackboard SHALL classify all narrative state variables into five explicit **Quality Categories** to support Quality-Based Narrative (QBN):

  1. `CURRENCY`: Consumable numerical resources spent or gained by storylet branches.

  2. `METRIC`: Character skill or capability levels tested by storylet challenges.

  3. `TOOL`: Persistent items, clues, or unlocked traits that satisfy preconditions without being consumed.

  4. `PROGRESS`: Integer milestones that track advancement along a narrative spine or investigation.

  5. `MENACE`: Threat accumulators that automatically trigger crisis storylets when they cross an upper limit.

* **REQ-WB-09:** Each Quality SHALL support a `is_hidden` property ("Quality Parsimony") to allow internal randomizers and bookkeeping flags to operate without cluttering the player UI.

### 4.2 Abstract Data Structures

```
# --- LAYERED WORLD GRAPH & QUALITY STRUCTURES ---

enum GraphLayer:
    LOCATION = 0
    CHARACTER = 1
    ITEM = 2
    FACT = 3

enum QualityCategory:
    CURRENCY = 0                       # Spent on branches (e.g., Gold, Favors)
    METRIC = 1                         # Tested skills (e.g., Watchful, Dangerous)
    TOOL = 2                           # Non-consumed keys/clues (e.g., Evidence_Motive)
    PROGRESS = 3                       # Story spine counters (e.g., Act_Stage)
    MENACE = 4                         # Danger accumulators (e.g., Suspicion, Wounds)

struct QualityDefinition:
    quality_id: string                 # e.g., "Quality.Clue.MotiveFound"
    category: QualityCategory
    current_value: float64
    min_value: float64
    max_value: float64
    is_hidden: bool                    # True for hidden randomizers or internal flags
    menace_crisis_storylet_id: string  # Auto-triggered storylet if category == MENACE and value >= max_value

struct AttributeValue:
    type: enum { INT, FLOAT, BOOL, STRING, TAG }
    int_val: int64
    float_val: float64
    bool_val: bool
    str_val: string

struct WorldVertex:
    vertex_id: string                  # Unique hierarchical tag, e.g., "Char.VillageOrc.Frank"
    layer: GraphLayer
    attributes: Map<string, AttributeValue>

struct WorldEdge:
    edge_id: string
    source_id: string
    target_id: string
    relation_type: string              # e.g., "LOCATED_IN", "OWNS", "ZERO_SUM_RIVAL"
    weight: float64                    # Used for Zero-Sum multiplier (e.g., -1.0 or -0.5)

struct GraphDiffRecord:
    added_vertex_ids: List<string>
    removed_vertices: List<WorldVertex>
    added_edge_ids: List<string>
    removed_edges: List<WorldEdge>
    previous_attributes: Map<string, Map<string, AttributeValue>> # vertex_id -> (attr_key -> old_val)


# --- PASSION ECONOMY STRUCTURES (NARRATIVE LEGOS) ---

struct MicroPassion:
    passion_id: string                 # e.g., "Passion.HatesElves"
    current_value: float64             # Range: [-100.0, +100.0]
    weight: float64                    # Contribution to Macro-Passion
    is_hidden: bool                    # True until revealed by gossip or threshold

struct MacroThreshold:
    threshold_id: string
    trigger_value: float64             # e.g., +50.0 or -40.0
    direction: enum { POSITIVE, NEGATIVE }
    is_currently_active: bool
    linked_graph_asset_id: string      # Sub-graph ("Lego") spawned when crossed

struct StarNPCComponent:
    character_vertex_id: string
    passion_pool: List<MicroPassion>   # Full authored pool (e.g., 10 passions)
    active_passions: Map<string, MicroPassion> # 3-4 active passions selected for this run
    macro_passion_value: float64       # Range: [-100.0, +100.0]
    trust_inertia: float64             # Hysteresis modifier to prevent threshold rapid toggling
    thresholds: List<MacroThreshold>

```

### 4.3 Algorithms: Zero-Sum Passion and Quality Mutation

```
class WorldBlackboard:
    vertices: Map<string, WorldVertex>
    edges: Map<string, WorldEdge>
    stars: Map<string, StarNPCComponent>
    qualities: Map<string, QualityDefinition>
    current_dramatic_arc_phase: int32 = 1 # e.g., 1=Setup, 2=RisingAction, 3=Climax, 4=Aftermath

    function MutateQuality(quality_id: string, delta: float64) -> float64:
        # Returns previous value for in-node rollback cache
        if not qualities.Contains(quality_id):
            return 0.0
        q = qualities[quality_id]
        old_val = q.current_value
        q.current_value = Clamp(old_val + delta, q.min_value, q.max_value)
        return old_val

    function RestoreQuality(quality_id: string, old_val: float64) -> void:
        if qualities.Contains(quality_id):
            qualities[quality_id].current_value = old_val

    function MutatePassion(star_id: string, passion_id: string, delta: float64, propagate_zero_sum: bool) -> Map<string, Map<string, float64>>:
        # Returns a snapshot of old passion values for all affected Stars so the caller node can cache it.
        old_values_snapshot = Map<string, Map<string, float64>>()
        
        if not stars.Contains(star_id):
            return old_values_snapshot

        # 1. Record and mutate primary Star
        star = stars[star_id]
        if star.active_passions.Contains(passion_id):
            old_val = star.active_passions[passion_id].current_value
            old_values_snapshot[star_id][passion_id] = old_val
            star.active_passions[passion_id].current_value = Clamp(old_val + delta, -100.0, 100.0)
            RecalculateMacroPassion(star)

        # 2. Propagate across Zero-Sum relational edges
        if propagate_zero_sum:
            for each edge in GetOutgoingEdges(star_id, "ZERO_SUM_RIVAL"):
                rival_id = edge.target_id
                rival_passion_id = edge.relation_metadata_passion_id
                rival_star = stars[rival_id]
                
                if rival_star.active_passions.Contains(rival_passion_id):
                    rival_old = rival_star.active_passions[rival_passion_id].current_value
                    old_values_snapshot[rival_id][rival_passion_id] = rival_old
                    rival_delta = delta * edge.weight # e.g., weight = -1.0 inverts the gain
                    rival_star.active_passions[rival_passion_id].current_value = Clamp(rival_old + rival_delta, -100.0, 100.0)
                    RecalculateMacroPassion(rival_star)

        return old_values_snapshot

    function RestorePassionSnapshot(snapshot: Map<string, Map<string, float64>>):
        for each (star_id, passion_map) in snapshot:
            star = stars[star_id]
            for each (passion_id, old_val) in passion_map:
                star.active_passions[passion_id].current_value = old_val
            RecalculateMacroPassion(star)

    function RecalculateMacroPassion(star: StarNPCComponent):
        old_macro = star.macro_passion_value
        weighted_sum = 0.0
        total_weight = 0.0

        for each (id, passion) in star.active_passions:
            weighted_sum += passion.current_value * passion.weight
            total_weight += passion.weight

        if total_weight > 0.0:
            raw_macro = (weighted_sum / total_weight) + star.trust_inertia
            star.macro_passion_value = Clamp(raw_macro, -100.0, 100.0)

        # Evaluate Threshold Crossings (Forward or Backward)
        for each threshold in star.thresholds:
            if threshold.direction == POSITIVE:
                if old_macro < threshold.trigger_value and star.macro_passion_value >= threshold.trigger_value:
                    threshold.is_currently_active = true
                    FlowEngine.SpawnSubGraph(threshold.linked_graph_asset_id)
                else if old_macro >= threshold.trigger_value and star.macro_passion_value < threshold.trigger_value:
                    threshold.is_currently_active = false
                    FlowEngine.RollbackAndCancelSubGraph(threshold.linked_graph_asset_id)

```

## 5. MODULE 2: FIRST-CLASS BEYOND-BRANCHING ARCHITECTURE (QBN, SALIENCE, AND WAYPOINT)

### 5.1 Architectural Purpose

Static branching graphs create exponential authoring overhead when players can complete objectives in any order (for example, gathering three clues: *Means*, *Motive*, and *Opportunity* requires $3! = 6$ duplicate tree paths in a naive branching graph). Furthermore, Pure Quality-Based Narrative systems require tedious manual bookkeeping to enforce linear plot spines ($A \to B \to C$).

The **ChronoFlow-Narrative Engine** solves both problems by making **Flow Graphs** and **Beyond-Branching Storylet Dispatchers** dual first-class citizens:

* **Macro-Spines use Flow Graphs:** Authors wire sequential plot milestones ($A \to B \to C$) directly as nodes in a `FlowGraphAsset` without manual progress-quality bookkeeping.

* **Open Hubs, Reactive Barks, and Dynamic Scenes use QBN, Salience, and Waypoint Nodes:** Any node inside a Flow Graph can open a **QBN Storylet Deck**, query the **Salience Engine**, or run a **Waypoint Topic Navigator**.

### 5.2 Requirements

* **REQ-BB-01 (Quality-Based Narrative Dispatcher):** The system SHALL evaluate `StoryletAsset` preconditions against the `WorldBlackboard` (`Qualities`, `GraphLayer` vertices, and `StarNPCComponent` passions) to assemble an eligible candidate pool for the player to select from.

* **REQ-BB-02 (Order-Independent Clue & Resource Aggregation):** A QBN Hub SHALL allow the player to execute any number of eligible storylets in any order (e.g., *Find Means*, *Find Motive*, *Find Opportunity*) until a gate storylet's preconditions become true (e.g., *Accuse Murderer*).

* **REQ-BB-03 (Storylet Surfacing & Pacing Clusters):** To prevent UI overload when many storylets are eligible, each `StoryletAsset` SHALL declare a `display_priority`, an `urgency_color_tag`, and a `StoryletClusterType` (`LINEAR_PROGRESSION`, `PLAYER_PASSAGE`, `WHEEL_DECK`, or `AFTERMATH_ACCELERATOR`).

* **REQ-BB-04 (Salience-Based Narrative Engine):** When a node requests a Salience match, the system SHALL automatically select the single most applicable `StoryletAsset` without prompting the player. The Salience score $S(c)$ of candidate $c$ SHALL combine two metrics:

  1. **Boolean Rule Specificity** (*Left 4 Dead / Firewatch* pattern): The count of matched precondition predicates multiplied by their specificity weight.

  2. **Continuous State-Vector Proximity** (*The King of Chicago* pattern): The inverse weighted Euclidean distance between the current World Blackboard values and the storylet's `ideal_state_vector`:
     

     $$
     S(c) = \left(\sum_{k=1}^{m} w_k \cdot \text{Match}(p_k)\right) + \left|\{b \in B_c : \text{Present}(b)\}\right| - \alpha \sqrt{\sum_{j=1}^{d} v_j \left(Q_j - I_{c,j}\right)^2}
     $$

     The second term is the `salience_bonus_tags` extension: each authored bonus tag present on the blackboard adds weight 1.0. `v_j` defaults to 1.0 and is authored per quality through `ideal_state_weights`.

* **REQ-BB-05 (Dramatic Arc Vulnerability Protection):** To prevent the *King of Chicago* salience vulnerability (where a player accidentally satisfies numerical preconditions for a premature climax or fatal ending during Act 1), every Salience candidate SHALL include a `required_arc_phase_mask`. The Salience Matcher SHALL reject any candidate whose phase mask does not match `Blackboard.current_dramatic_arc_phase`.

* **REQ-BB-06 (Waypoint Topic-Graph Navigation):** A Waypoint Node SHALL represent a conversation or scene net as a directed graph $G_{topic} = (V_{topic}, E_{topic})$ with two competing agents:

  1. **NPC Goal Pathfinding:** The NPC computes the lowest-cost path from `current_topic` to `npc_goal_topic` using Dijkstra's algorithm and traverses the first edge, speaking its transition line.

  2. **Player Topic Diversion & Trigger Nodes:** The player can inject a topic shift to an adjacent or unlocked topic node. When a transition crosses an edge, the engine SHALL add a `traversal_penalty_weight` to that edge so the NPC routes through alternative topics on subsequent turns (exposing new clues or hidden trigger topics such as *Risk of Revolution*).

### 5.3 Abstract Data Structures

```
enum StoryletClusterType:
    LINEAR_PROGRESSION = 0             # Constrained high-tension chain
    PLAYER_PASSAGE = 1                 # Open hub for investigation or gearing up
    WHEEL_DECK = 2                     # Randomized/weighted opportunity card deck
    AFTERMATH_ACCELERATOR = 3          # Rapid consequence sequence

struct QualityPredicate:
    target_id: string                  # Quality ID, Passion ID, or Vertex Attribute
    operator: enum { EQ, NEQ, GT, GTE, LT, LTE }
    comparison_value: float64
    specificity_weight: float64 = 1.0  # Used for Salience scoring

struct StoryletEffect:
    target_id: string
    delta_or_set_value: float64
    is_incremental: bool               # True for +/- delta, False for absolute assignment

struct StoryletAsset:
    storylet_id: UUID
    title: string
    cluster_type: StoryletClusterType
    display_priority: int32            # Ranks storylet in large QBN lists
    urgency_tag: string                # e.g., "Gold_MustPlay", "Red_Menace", "Standard"
    is_repeatable: bool
    cooldown_ms: int64
    
    # 1. Hard Preconditions (QBN & Salience Eligibility)
    preconditions: List<QualityPredicate>
    required_arc_phases: Set<int32>    # Prevents premature salience endings
    
    # 2. Continuous Salience Target (King of Chicago Vector Match)
    ideal_state_vector: Map<string, float64> # quality_id -> ideal numerical value
    
    # 3. Execution Payload & Effects
    payload_graph_asset_id: string     # Sub-graph or SceneSection executed when selected
    effects_on_complete: List<StoryletEffect>


# --- WAYPOINT TOPIC-GRAPH STRUCTURES ---

struct WaypointTopicNode:
    topic_id: string                   # e.g., "Topic.ThePeople", "Topic.Royalty", "Topic.Revolution"
    is_player_only_trigger: bool       # True if NPCs never pathfind here voluntarily
    triggers_scene_exit: bool          # True if reaching this topic ends the conversation and branches
    exit_output_pin: string            # e.g., "Out_DefaultRoyalty" or "Out_RevolutionTriggered"
    revealed_clue_quality_id: string   # Optional quality unlocked when topic is visited

struct WaypointTopicEdge:
    edge_id: string
    from_topic_id: string
    to_topic_id: string
    dialogue_transition_asset_id: string
    base_cost: float64 = 1.0
    used_penalty_cost: float64 = 100.0 # Added after use so NPC routes around already-used edges
    is_fallback_default: bool = false

```

### 5.4 Algorithms: QBN Dispatcher, Salience Matcher, and Waypoint Navigator

```
class BeyondBranchingDispatcher:
    storylet_database: Map<UUID, StoryletAsset>
    completed_non_repeatable_storylets: Set<UUID>

    # --- 1. QUALITY-BASED NARRATIVE (QBN) QUERY ---
    function GetAvailableQBNStorylets(deck_filter_ids: List<UUID>, blackboard: WorldBlackboard) -> List<StoryletAsset>:
        eligible_list = List<StoryletAsset>()
        
        for each id in deck_filter_ids:
            storylet = storylet_database[id]
            if not storylet.is_repeatable and completed_non_repeatable_storylets.Contains(id):
                continue
            if EvaluatePreconditions(storylet.preconditions, blackboard):
                eligible_list.Add(storylet)
                
        # Sort by author-defined display_priority to solve QBN scale clutter
        SortDescending(eligible_list, by: storylet.display_priority)
        return eligible_list

    # --- 2. SALIENCE-BASED NARRATIVE (SBN) MATCHER ---
    function SelectMostSalientStorylet(
        pool_ids: List<UUID>, 
        blackboard: WorldBlackboard,
        deterministic_seed: uint64
    ) -> StoryletAsset:
        
        best_candidates = List<StoryletAsset>()
        highest_score = -Infinity

        for each id in pool_ids:
            candidate = storylet_database[id]
            
            # Step A: Check Dramatic Arc Gate (Prevents premature King of Chicago endings)
            if not candidate.required_arc_phases.IsEmpty():
                if not candidate.required_arc_phases.Contains(blackboard.current_dramatic_arc_phase):
                    continue

            # Step B: Check Hard Preconditions
            if not EvaluatePreconditions(candidate.preconditions, blackboard):
                continue

            # Step C: Compute Hybrid Salience Score (Specificity - Ideal Vector Distance)
            specificity_score = 0.0
            for each pred in candidate.preconditions:
                if EvaluatePredicate(pred, blackboard):
                    specificity_score += pred.specificity_weight
            # Extension: each present salience bonus tag adds weight 1.0.
            for each tag in candidate.salience_bonus_tags:
                if blackboard.HasTag(tag):
                    specificity_score += 1.0

            vector_distance_sq = 0.0
            for each (q_id, ideal_val) in candidate.ideal_state_vector:
                actual_val = blackboard.GetNumericValue(q_id)
                weight = candidate.ideal_state_weights.Get(q_id, 1.0)
                diff = actual_val - ideal_val
                vector_distance_sq += weight * (diff * diff)

            salience_score = specificity_score - (0.05 * Sqrt(vector_distance_sq))

            if Abs(salience_score - highest_score) < 0.0001:
                best_candidates.Add(candidate)
            else if salience_score > highest_score:
                highest_score = salience_score
                best_candidates.Clear()
                best_candidates.Add(candidate)

        # Deterministic tie-breaking using saved seed for 100% rollback/replay fidelity
        if best_candidates.Count() == 1:
            return best_candidates[0]
        pick_index = DeterministicHash(deterministic_seed) % best_candidates.Count()
        return best_candidates[pick_index]


    # --- 3. WAYPOINT TOPIC-GRAPH PATHFINDER ---
    function ComputeNextNPCTopicStep(
        current_topic: string,
        npc_goal_topic: string,
        nodes: Map<string, WaypointTopicNode>,
        edges: List<WaypointTopicEdge>,
        edge_use_counts: Map<string, int32>
    ) -> WaypointTopicEdge:
        
        # Runs Dijkstra shortest path where dynamic edge weight = base_cost + (use_count * used_penalty_cost)
        # Excludes nodes where is_player_only_trigger == true
        shortest_path_edges = DijkstraShortestPath(
            start: current_topic,
            goal: npc_goal_topic,
            edges: edges,
            cost_fn: (e) => e.base_cost + (edge_use_counts.GetOrDefault(e.edge_id, 0) * e.used_penalty_cost),
            node_filter: (n) => not nodes[n].is_player_only_trigger
        )
        
        if not shortest_path_edges.IsEmpty():
            return shortest_path_edges[0] # Return the immediate next conversational transition
            
        # Fallback if player exhausted all normal paths
        return FindFallbackEdge(current_topic, edges)

```

## 6. MODULE 3: TOKEN-DRIVEN FLOW GRAPH CORE

### 6.1 Requirements

* **REQ-FG-01:** The Flow Graph Runtime SHALL separate static graph topology (`FlowGraphAsset`) from runtime state (`FlowGraphInstance`).

* **REQ-FG-02:** Every node in a `FlowGraphAsset` SHALL have a persistent, globally unique identifier (`node_guid`).

* **REQ-FG-03:** The runtime SHALL drive execution through a flat array of fixed-size `SignalToken` structures to maximize CPU cache locality.

* **REQ-FG-04:** The runtime SHALL classify every node into one of five functional types:

  1. `MUTATOR`: Immediate execution that changes world, passion, or quality state and caches previous values in the node instance.

  2. `WAITER`: Latent execution that halts the signal token until an external gameplay verb or condition occurs.

  3. `LOGIC_GATE`: Routing node (`ALL` / Logical AND, `ANY` / Logical OR, `CHOICE`, `POI`).

  4. `DYNAMIC_DISPATCHER`: First-class Beyond-Branching nodes (`Node_QBNStoryletHub`, `Node_SalienceSelector`, `Node_WaypointConversation`).

  5. `SCOPED_CONTAINER`: A node that encapsulates an isolated sub-graph (`MISSION`) or a time-windowed timeline (`SCENE_SECTION`).

* **REQ-FG-05:** Every `FlowNodeInstance` SHALL implement three execution interfaces:

  1. `ExecuteNormal(input_pin)`: Standard runtime behavior.

  2. `ForceComplete(chosen_output_pin, chosen_storylet_id)`: Instant 1-frame execution for save-loading and Story Invokes.

  3. `CancelAndRevert()`: Instant 1-frame inverse execution that restores the node's internal cached state.

### 6.2 Abstract Data Structures

```
enum NodeCategory:
    MUTATOR = 0
    WAITER = 1
    LOGIC_GATE = 2
    DYNAMIC_DISPATCHER = 3
    SCOPED_CONTAINER = 4

enum NodeExecutionState:
    IDLE = 0
    ACTIVE_LATENT = 1
    COMPLETED = 2
    ABORTED = 3

struct PinDefinition:
    pin_name: string
    is_input: bool

struct EdgeConnection:
    source_node_guid: UUID
    source_pin_name: string
    target_node_guid: UUID
    target_pin_name: string

struct SignalToken:
    token_id: uint64
    graph_instance_id: UUID
    target_node_guid: UUID
    target_pin_name: string
    time_budget_ms: int64
    is_active: bool                    # False if deferred to next frame due to budget

# --- ABSTRACT BASE NODE ---

class FlowNodeInstance (Abstract):
    node_guid: UUID
    node_type_id: string
    category: NodeCategory
    state: NodeExecutionState
    entity_path: string                # Soft string binding to level actor (e.g., "Convoy.Truck.TeddyBear_01")
    input_pins: List<PinDefinition>
    output_pins: List<PinDefinition>

    # Standard gameplay execution
    abstract function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>

    # 1-Frame instant execution for Save Load & Story Invokes
    abstract function ForceComplete(chosen_output_pin: string, chosen_storylet_id: UUID) -> void

    # 1-Frame inverse execution for Rollback
    abstract function CancelAndRevert() -> void

```

### 6.3 Concrete Atomic and Beyond-Branching Node Implementations (Pseudocode)

```
# 1. WORLD MUTATOR EXAMPLE: Set Variable / Quality Node
class Node_SetQuality extends FlowNodeInstance:
    quality_key: string
    delta_value: float64
    
    # In-Node Inverse Cache (LIFO Stack supports cyclic loops in graphs)
    cached_previous_values: Stack<float64>

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        ApplyMutation()
        state = COMPLETED
        return EmitTokenOnPin("Out", token.time_budget_ms)

    function ForceComplete(chosen_output_pin: string, chosen_storylet_id: UUID) -> void:
        ApplyMutation()
        state = COMPLETED

    function ApplyMutation() -> void:
        old_val = Blackboard.MutateQuality(quality_key, delta_value)
        cached_previous_values.Push(old_val)

    function CancelAndRevert() -> void:
        if not cached_previous_values.IsEmpty():
            prev_val = cached_previous_values.Pop()
            Blackboard.RestoreQuality(quality_key, prev_val)
        state = IDLE


# 2. FIRST-CLASS QBN NODE: Unsnapped Storylet Hub (Order-Independent Clues / Hubs)
class Node_QBNStoryletHub extends FlowNodeInstance:
    deck_storylet_ids: List<UUID>
    hub_exit_storylet_id: UUID         # e.g., "Accuse_Murderer" storylet that exits the hub
    
    # Stack of (storylet_id, snapshot_of_previous_qualities) for exact LIFO rollback
    executed_storylet_history: Stack<Pair<UUID, Map<string, float64>>>

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        state = ACTIVE_LATENT
        available = Dispatcher.GetAvailableQBNStorylets(deck_storylet_ids, Blackboard)
        UI.PresentStoryletDeck(this.node_guid, available, OnPlayerSelectedStorylet)
        return EmptyList()

    function OnPlayerSelectedStorylet(chosen_id: UUID) -> void:
        ExecuteStoryletEffects(chosen_id)
        FlowEngine.RecordCallHistory(this.node_guid, "In", "Out", chosen_id)
        
        if chosen_id == hub_exit_storylet_id:
            state = COMPLETED
            FlowEngine.EnqueueToken(this.node_guid, "Out")
        else:
            # Remain in hub and refresh available QBN storylets (e.g., Means/Motive/Opportunity)
            available = Dispatcher.GetAvailableQBNStorylets(deck_storylet_ids, Blackboard)
            UI.PresentStoryletDeck(this.node_guid, available, OnPlayerSelectedStorylet)

    function ForceComplete(chosen_output_pin: string, chosen_storylet_id: UUID) -> void:
        ExecuteStoryletEffects(chosen_storylet_id)
        if chosen_storylet_id == hub_exit_storylet_id:
            state = COMPLETED

    function ExecuteStoryletEffects(storylet_id: UUID) -> void:
        storylet = Dispatcher.storylet_database[storylet_id]
        snapshot = Map<string, float64>()
        for each eff in storylet.effects_on_complete:
            old_val = Blackboard.MutateQuality(eff.target_id, eff.delta_or_set_value)
            snapshot[eff.target_id] = old_val
        if not storylet.is_repeatable:
            Dispatcher.completed_non_repeatable_storylets.Add(storylet_id)
        executed_storylet_history.Push(Pair(storylet_id, snapshot))

    function CancelAndRevert() -> void:
        if not executed_storylet_history.IsEmpty():
            (last_id, snapshot) = executed_storylet_history.Pop()
            for each (q_id, old_val) in snapshot:
                Blackboard.RestoreQuality(q_id, old_val)
            Dispatcher.completed_non_repeatable_storylets.Remove(last_id)
        state = IDLE


# 3. FIRST-CLASS SALIENCE NODE: Automatic Context-Aware Bark / Scene Selector
class Node_SalienceSelector extends FlowNodeInstance:
    candidate_pool_ids: List<UUID>
    executed_storylet_stack: Stack<Pair<UUID, Map<string, float64>>>

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        seed = FlowEngine.GetNextDeterministicSeed()
        winner = Dispatcher.SelectMostSalientStorylet(candidate_pool_ids, Blackboard, seed)
        ApplyStorylet(winner.storylet_id)
        state = COMPLETED
        FlowEngine.RecordCallHistory(this.node_guid, "In", "Out", winner.storylet_id)
        return EmitTokenOnPin("Out", token.time_budget_ms)

    function ForceComplete(chosen_output_pin: string, chosen_storylet_id: UUID) -> void:
        ApplyStorylet(chosen_storylet_id)
        state = COMPLETED

    function CancelAndRevert() -> void:
        if not executed_storylet_stack.IsEmpty():
            (s_id, snapshot) = executed_storylet_stack.Pop()
            for each (q_id, old_val) in snapshot:
                Blackboard.RestoreQuality(q_id, old_val)
            Dispatcher.completed_non_repeatable_storylets.Remove(s_id)
        state = IDLE


# 4. FIRST-CLASS WAYPOINT NODE: Goal-Driven Topic Graph Navigator
class Node_WaypointConversation extends FlowNodeInstance:
    topic_nodes: Map<string, WaypointTopicNode>
    topic_edges: List<WaypointTopicEdge>
    initial_topic_id: string           # e.g., "Topic.ThePeople"
    npc_goal_topic_id: string          # e.g., "Topic.Royalty"
    
    # In-Node History Stack of (previous_topic, traversed_edge_id, unlocked_quality_id)
    step_history_stack: Stack<Tuple<string, string, string>>
    edge_use_counts: Map<string, int32>
    current_topic_id: string

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        current_topic_id = initial_topic_id
        state = ACTIVE_LATENT
        StepNPCTowardsGoal()
        return EmptyList()

    function StepNPCTowardsGoal() -> void:
        chosen_edge = Dispatcher.ComputeNextNPCTopicStep(
            current_topic_id, npc_goal_topic_id, topic_nodes, topic_edges, edge_use_counts
        )
        TraverseTopicEdge(chosen_edge)

    function OnPlayerDivertTopic(player_chosen_edge: WaypointTopicEdge) -> void:
        TraverseTopicEdge(player_chosen_edge)
        if state != COMPLETED:
            StepNPCTowardsGoal() # NPC dynamically re-routes from new topic using penalized edges

    function TraverseTopicEdge(edge: WaypointTopicEdge) -> void:
        prev_topic = current_topic_id
        current_topic_id = edge.to_topic_id
        edge_use_counts[edge.edge_id] = edge_use_counts.GetOrDefault(edge.edge_id, 0) + 1
        
        target_node = topic_nodes[current_topic_id]
        unlocked_q = ""
        if target_node.revealed_clue_quality_id != "":
            Blackboard.MutateQuality(target_node.revealed_clue_quality_id, 1.0)
            unlocked_q = target_node.revealed_clue_quality_id
            
        step_history_stack.Push(Tuple(prev_topic, edge.edge_id, unlocked_q))
        
        if target_node.triggers_scene_exit:
            state = COMPLETED
            FlowEngine.RecordCallHistory(this.node_guid, "In", target_node.exit_output_pin, UUID_EMPTY)
            FlowEngine.EnqueueToken(this.node_guid, target_node.exit_output_pin)

    function ForceComplete(chosen_output_pin: string, chosen_storylet_id: UUID) -> void:
        # Finds the exit topic matching chosen_output_pin and applies its revealed qualities
        for each (t_id, t_node) in topic_nodes:
            if t_node.triggers_scene_exit and t_node.exit_output_pin == chosen_output_pin:
                current_topic_id = t_id
                if t_node.revealed_clue_quality_id != "":
                    Blackboard.MutateQuality(t_node.revealed_clue_quality_id, 1.0)
                step_history_stack.Push(Tuple(initial_topic_id, "DIRECT_INVOKE", t_node.revealed_clue_quality_id))
        state = COMPLETED

    function CancelAndRevert() -> void:
        while not step_history_stack.IsEmpty():
            (prev_topic, edge_id, unlocked_q) = step_history_stack.Pop()
            if edge_id != "DIRECT_INVOKE":
                edge_use_counts[edge_id] -= 1
            if unlocked_q != "":
                Blackboard.MutateQuality(unlocked_q, -1.0)
            current_topic_id = prev_topic
        state = IDLE

```

## 7. MODULE 4: SCRUBBABLE SCENE SECTIONS AND TIMELINE COMPOSER

### 7.1 Requirements

* **REQ-SC-01:** A `Node_SceneSection` SHALL maintain an internal fixed-point clock in integer milliseconds (`section_clock_ms`).

* **REQ-SC-02:** Timeline events inside a section SHALL have a `start_time_ms`, a `duration_ms` (0 for point events), and a specific action payload (Animation Workspot, Camera Cut, Audio/VO, LookAt, or VFX).

* **REQ-SC-03:** During forward playback, the section SHALL evaluate events against the left-inclusive, right-exclusive time window $[t_0, t_1)$ where $t_0 < t_1$.

* **REQ-SC-04:** During backward scrubbing or rollback, the section SHALL evaluate events against the inverted time window $(t_1, t_0]$ where $t_1 < t_0$.

* **REQ-SC-05:** The section SHALL decompose overlapping events into `ActionPart` structures (`START`, `REGULAR`, `END`), sort them by `action_type` priority and `target_entity_id` combiner, and dispatch them to the `SceneComposer`.

* **REQ-SC-06:** The `SceneComposer` SHALL arbitrate conflicts when multiple concurrent scene instances issue commands to the same actor.

### 7.2 Algorithm: Reversible Time-Window Event Processing

```
enum EventOverlapActivity:
    START_ACTIVITY = 0
    REGULAR_ACTIVITY = 1
    END_ACTIVITY = 2

struct TimelineTrackEvent:
    event_id: UUID
    action_type: enum { WORKSPOT_POSE, TRANSITION_ANIM, DIALOGUE_VO, CAMERA_CUT, VFX_POINT }
    target_entity_path: string
    start_time_ms: int64
    duration_ms: int64                 # 0 for point events
    payload_asset_id: string
    cached_previous_pose_id: string    # Allows restoring previous actor workspot on backward scrub

struct ActionPart:
    action_key_priority: int32
    combiner_entity_path: string
    activity: EventOverlapActivity
    local_Cursor_time_ms: int64
    event_ref: TimelineTrackEvent

class Node_SceneSection extends FlowNodeInstance:
    section_clock_ms: int64 = 0
    section_duration_ms: int64
    events: List<TimelineTrackEvent>   # Sorted by start_time_ms

    function ProcessTimeWindow(delta_ms: int64) -> void:
        # delta_ms is positive for forward play, negative for backward scrubbing
        t_start = section_clock_ms
        t_end = Clamp(section_clock_ms + delta_ms, 0, section_duration_ms)
        section_clock_ms = t_end

        action_parts = List<ActionPart>()

        if delta_ms >= 0:
            # Forward Window: [t_start, t_end) -> Inclusive left, Non-inclusive right
            for each ev in events:
                ev_end = ev.start_time_ms + ev.duration_ms
                if ev.duration_ms == 0:
                    if ev.start_time_ms >= t_start and ev.start_time_ms < t_end:
                        action_parts.Add(CreatePart(ev, START_ACTIVITY, 0))
                else:
                    if ev_end > t_start and ev.start_time_ms < t_end:
                        act = DetermineForwardActivity(ev.start_time_ms, ev_end, t_start, t_end)
                        offset = t_end - ev.start_time_ms
                        action_parts.Add(CreatePart(ev, act, offset))
        else:
            # Backward Scrub Window: (t_end, t_start] -> Non-inclusive left, Inclusive right
            for each ev in Reverse(events):
                ev_end = ev.start_time_ms + ev.duration_ms
                if ev.duration_ms == 0:
                    if ev.start_time_ms <= t_start and ev.start_time_ms > t_end:
                        action_parts.Add(CreateInversePart(ev, END_ACTIVITY, 0))
                else:
                    if ev_end >= t_end and ev.start_time_ms <= t_start:
                        act = DetermineBackwardActivity(ev.start_time_ms, ev_end, t_end, t_start)
                        offset = t_end - ev.start_time_ms
                        action_parts.Add(CreateInversePart(ev, act, offset))

        # Sort by Priority Key, then by Target Entity Combiner
        Sort(action_parts, by: (part.action_key_priority, part.combiner_entity_path))
        SceneComposer.ExecuteSortedActionParts(this.node_guid, action_parts, is_reverse=(delta_ms < 0))

```

## 8. MODULE 5: CALL-HISTORY ENGINE, ROLLBACK, AND HYBRID STORY INVOKES

### 8.1 Requirements

* **REQ-CH-01:** The system SHALL record every completed node and storylet invocation as a lightweight `CallHistoryEntry` in an ordered array.

* **REQ-CH-02:** To save the game state, the system SHALL serialize only the `CallHistoryEntry` arrays (Global Call History and active Scoped Mission Call Histories) and random seeds.

* **REQ-CH-03:** To load a saved game state, the system SHALL iterate through the saved `CallHistoryEntry` array from index `0` to `N-1` and invoke `ForceComplete(chosen_output_pin, chosen_storylet_id)` on each corresponding node in one frame.

* **REQ-CH-04:** A `Node_MissionSubGraph` SHALL isolate its internal node invocations in a local `mission_call_history` list. Rolling back a Mission node SHALL unwind only its `mission_call_history` list without changing the Global Call History.

* **REQ-CH-05:** The `InvokeStoryToPOI` algorithm SHALL find a valid execution path by traversing backward from a target `Node_POI` to the currently active nodes across **both** hardwired graph edges and implicit Quality-Based Narrative (`QBN`) precondition links:

  * When backward traversal encounters a `Node_LogicALL` (AND gate), it SHALL include **all** incoming upstream branches in the invoke path.

  * When backward traversal encounters a `Node_LogicANY` (OR gate), `Node_Choice`, or `Node_WaypointConversation`, it SHALL select the upstream pin specified in the `declared_choice_routes` map.

  * When backward traversal encounters a `Node_QBNStoryletHub`, it SHALL resolve the hub's exit storylet preconditions (e.g., needing *Means*, *Motive*, and *Opportunity* qualities) by automatically queuing the prerequisite storylets from the hub's deck in topological order.

* **REQ-CH-06:** The `ReconcileTimelines` algorithm SHALL synchronize a current Call History ($H_{current}$) to a target Call History ($H_{target}$) in one frame by:

  1. Finding the Lowest Common Ancestor (LCA) index where $H_{current}$ and $H_{target}$ diverge.

  2. Calling `CancelAndRevert()` in reverse order from the top of $H_{current}$ down to the LCA index.

  3. Calling `ForceComplete()` in forward order from the LCA index to the end of $H_{target}$.

### 8.2 Abstract Data Structures

```
struct CallHistoryEntry:
    node_guid: UUID
    executed_input_pin: string
    chosen_output_pin: string          # Critical for Choice, ANY, and Waypoint exit pins
    chosen_storylet_id: UUID           # Critical for QBN Storylet Hubs and Salience matches

class Node_MissionSubGraph extends FlowNodeInstance:
    sub_graph_asset_id: string
    sub_graph_instance: FlowGraphInstance
    local_call_history: List<CallHistoryEntry>
    scoped_entity_paths: List<string>

    function RollbackMissionToStart() -> void:
        # Unwinds only the local mission stack (safe for Co-op failure/retry)
        for i from (local_call_history.Count() - 1) down to 0:
            entry = local_call_history[i]
            node = sub_graph_instance.GetNode(entry.node_guid)
            node.CancelAndRevert()
        
        local_call_history.Clear()
        sub_graph_instance.RestartFromStartNode()

```

### 8.3 Algorithm: 1-Frame Hybrid Backward Path-Seeking ("Story Invoke" with QBN Resolution)

```
class ChronoInvokeManager:
    
    function InvokeStoryToPOI(
        graph: FlowGraphInstance, 
        target_poi_guid: UUID, 
        declared_routes: Map<UUID, string> # Maps ANY/Choice/Waypoint node_guid -> desired pin_name
    ) -> void:
        
        # Step 1: Seek backward from target_poi_guid to currently active/completed nodes
        nodes_to_invoke_forward = List<CallHistoryEntry>()
        visited = Set<UUID>()
        
        function SeekBackwardRecursive(current_guid: UUID):
            if visited.Contains(current_guid):
                return
            visited.Add(current_guid)
            
            node = graph.GetNode(current_guid)
            
            # Stop seeking backward on this branch if node is already completed
            if node.state == COMPLETED:
                return
            
            # Determine which incoming edges to traverse backward
            incoming_edges = graph.GetIncomingEdges(current_guid)
            selected_edges = List<EdgeConnection>()
            
            if node is Node_LogicALL:
                # Logical AND: Must complete ALL upstream branches simultaneously
                selected_edges = incoming_edges
            else if node is Node_LogicANY or incoming_edges.Count() > 1:
                # Logical OR / Multiple Entrances: Consult declared route pin
                desired_pin = declared_routes.GetOrDefault(current_guid, incoming_edges[0].target_pin_name)
                selected_edges = Filter(incoming_edges, where: edge.target_pin_name == desired_pin)
            else:
                selected_edges = incoming_edges

            # Recurse upstream first (Post-Order Traversal builds topological forward order)
            for each edge in selected_edges:
                SeekBackwardRecursive(edge.source_node_guid)
                
                upstream_node = graph.GetNode(edge.source_node_guid)
                if upstream_node.state != COMPLETED:
                    if upstream_node is Node_QBNStoryletHub:
                        # Backward QBN Goal Chaining: Synthesize required clue/resource storylets
                        required_storylets = ResolveQBNPrerequisiteChain(upstream_node)
                        for each s_id in required_storylets:
                            nodes_to_invoke_forward.Add(CallHistoryEntry(
                                node_guid: upstream_node.node_guid,
                                executed_input_pin: "In",
                                chosen_output_pin: edge.source_pin_name,
                                chosen_storylet_id: s_id
                            ))
                    else:
                        nodes_to_invoke_forward.Add(CallHistoryEntry(
                            node_guid: edge.source_node_guid,
                            executed_input_pin: "In",
                            chosen_output_pin: edge.source_pin_name,
                            chosen_storylet_id: UUID_EMPTY
                        ))

        SeekBackwardRecursive(target_poi_guid)

        # Step 2: ForceComplete all collected nodes & QBN storylets in forward order in 1 frame
        for each entry in DeduplicatePreserveOrder(nodes_to_invoke_forward):
            node = graph.GetNode(entry.node_guid)
            node.ForceComplete(entry.chosen_output_pin, entry.chosen_storylet_id)
            graph.global_call_history.Add(entry)

        # Step 3: Execute Target POI Node (Teleports player to POI respawn transform)
        poi_node = graph.GetNode(target_poi_guid)
        poi_node.ForceComplete("Out", UUID_EMPTY)
        graph.global_call_history.Add(CallHistoryEntry(target_poi_guid, "In", "Out", UUID_EMPTY))

```

### 8.4 Algorithm: Timeline Reconciliation (Choice Branch Jump & Co-op Sync)

```
    function ReconcileTimelines(
        graph: FlowGraphInstance,
        target_history: List<CallHistoryEntry>
    ) -> void:
        
        current_history = graph.global_call_history
        
        # Step 1: Find Lowest Common Ancestor (LCA) index
        lca_index = 0
        min_len = Min(current_history.Count(), target_history.Count())
        
        while lca_index < min_len:
            if current_history[lca_index] != target_history[lca_index]:
                break
            lca_index += 1

        # Step 2: Rollback unshared calls in current_history from top down to lca_index
        for i from (current_history.Count() - 1) down to lca_index:
            entry_to_cancel = current_history[i]
            node = graph.GetNode(entry_to_cancel.node_guid)
            node.CancelAndRevert()
            current_history.RemoveAt(i)

        # Step 3: Invoke forward along target_history from lca_index to tip
        for j from lca_index to (target_history.Count() - 1):
            entry_to_invoke = target_history[j]
            node = graph.GetNode(entry_to_invoke.node_guid)
            node.ForceComplete(entry_to_invoke.chosen_output_pin, entry_to_invoke.chosen_storylet_id)
            current_history.Add(entry_to_invoke)

```

## 9. MODULE 6: LIVE GRAPH PATCHING (CALL-HISTORY REBASING)

### 9.1 The Patching Problem

When a saved game stores a `CallHistoryEntry` list, modifying the `FlowGraphAsset` or `StoryletAsset` database in a game patch (deleting nodes, inserting new prerequisite nodes, or rewiring pins) makes the saved Call History invalid if replayed blindly.

### 9.2 Requirements

* **REQ-GP-01:** When loading a saved `CallHistoryEntry` array against an updated `FlowGraphAsset`, the engine SHALL execute the `RebaseCallHistoryOnPatch` algorithm before replaying the history.

* **REQ-GP-02:** If a `node_guid` or `chosen_storylet_id` in the saved Call History no longer exists in the updated assets, the Rebaser SHALL remove that entry from the Call History.

* **REQ-GP-03:** If the updated `FlowGraphAsset` inserts new uncompleted nodes between two nodes $N_A$ and $N_B$ that were both completed in the saved Call History, the Rebaser SHALL run `SeekBackwardRecursive` from $N_B$ to $N_A$ and splice the synthesized `CallHistoryEntry` records into the history list.

### 9.3 Algorithm: Call-History Patch Rebaser

```
    function RebaseCallHistoryOnPatch(
        updated_graph_asset: FlowGraphAsset,
        saved_history: List<CallHistoryEntry>
    ) -> List<CallHistoryEntry>:
        
        rebased_history = List<CallHistoryEntry>()
        
        # 1. Strip deleted nodes and retired storylets (Tombstone Garbage Collection)
        valid_saved_entries = List<CallHistoryEntry>()
        for each entry in saved_history:
            if updated_graph_asset.HasNode(entry.node_guid):
                if entry.chosen_storylet_id == UUID_EMPTY or Dispatcher.HasStorylet(entry.chosen_storylet_id):
                    valid_saved_entries.Add(entry)

        # 2. Verify topological continuity and splice newly inserted nodes
        for i from 0 to (valid_saved_entries.Count() - 1):
            current_entry = valid_saved_entries[i]
            rebased_history.Add(current_entry)
            
            if i + 1 < valid_saved_entries.Count():
                next_entry = valid_saved_entries[i + 1]
                
                # Check if a direct edge still exists or if a designer inserted new nodes in between
                if current_entry.node_guid != next_entry.node_guid:
                    if not updated_graph_asset.HasDirectEdge(current_entry.node_guid, next_entry.node_guid):
                        missing_bridge_entries = FindShortestBridgePath(
                            updated_graph_asset, 
                            source: current_entry.node_guid, 
                            target: next_entry.node_guid
                        )
                        rebased_history.AddAll(missing_bridge_entries)

        return rebased_history

```

## 10. REFERENCE MAPPING TO UNREAL ENGINE (`MothCocoon/FlowGraph`)

If the code agent or developer implements this specification inside Unreal Engine using `MothCocoon/FlowGraph`, map the abstract types as follows:

| Abstract Specification Type | `MothCocoon/FlowGraph` / UE5 Equivalent | Implementation Note | 
| ----- | ----- | ----- | 
| `WorldBlackboard` & `QualityDefinition` | `UGameInstanceSubsystem` (`UChronoWorldSubsystem`) | Holds the Layered World Graph, `FGameplayTag` Qualities (`Currency`, `Metric`, `Tool`, `Progress`, `Menace`), and Star Passion components. | 
| `StoryletAsset` | `UDataAsset` (`UChronoStoryletAsset`) | Contains `Preconditions`, `IdealStateVector`, `RequiredArcPhases`, and a soft reference to a payload `UFlowAsset`. | 
| `Node_QBNStoryletHub` | `UFlowNode` subclass (`UFlowNode_QBNHub`) | Queries eligible `UChronoStoryletAsset` items; records `ChosenStoryletGuid` inside `FFlowCallHistoryEntry`. | 
| `Node_SalienceSelector` | `UFlowNode` subclass (`UFlowNode_SalienceMatch`) | Evaluates specificity and Euclidean vector distance against `UChronoWorldSubsystem`; auto-fires best storylet or dialogue bark. | 
| `Node_WaypointConversation` | `UFlowNode` subclass (`UFlowNode_WaypointTopicGraph`) | Runs dynamic Dijkstra topic pathfinding with edge reuse penalties and player topic diversion inputs. | 
| `FlowNodeInstance` | `UFlowNode` subclass (`UFlowNode_ChronoAtomic`) | Override `ExecuteInput()`, `OnLoad()`, and add `ForceComplete()` and `CancelAndRevert()`. | 
| `Node_MissionSubGraph` | `UFlowNode_SubGraph` subclass | Add `TArray<FFlowCallHistoryEntry> LocalMissionHistory` marked with `UPROPERTY(SaveGame)`. | 

## 11. VERIFICATION AND AUTOMATED QA REQUIREMENTS

To verify the implementation, the code agent SHALL implement the following four automated test suites:

1. **Test Suite 1: Bijective Rollback Identity Test (`TEST_ROLLBACK_IDENTITY`)**

   * **Procedure:** Initialize a `WorldBlackboard` and `FlowGraphInstance`. Compute a 64-bit state hash `Hash_0` of all vertices, edges, attributes, Qualities, and Star NPC passions. Execute 500 random nodes (including `Node_SetQuality`, `Node_QBNStoryletHub` clue gathering, `Node_SalienceSelector`, `Node_WaypointConversation`, and `Node_MutatePassion` with Zero-Sum links). Then execute `ReconcileTimelines` with an empty target history `[]`. Compute `Hash_Final`.

   * **Acceptance Criterion:** `Hash_Final` MUST equal `Hash_0` with zero floating-point drift and zero leaked event subscriptions.

2. **Test Suite 2: 1-Frame Hybrid Story Invoke & QBN Chaining Test (`TEST_INVOKE_EQUIVALENCE`)**

   * **Procedure:** Author an investigation graph containing a `Node_QBNStoryletHub` with four storylets (*Find Means*, *Find Motive*, *Find Opportunity*, and the gated exit storylet *Accuse Murderer*) leading to `POI_End`. Play through the hub manually to `POI_End`, recording `Hash_Normal`. Reset the world. Run `InvokeStoryToPOI(POI_End)` in a single frame, recording `Hash_Invoked`.

   * **Acceptance Criterion:** The backward path-seeker MUST automatically resolve and complete all three clue storylets plus *Accuse Murderer*, `Hash_Invoked` MUST equal `Hash_Normal`, and execution time MUST be less than `2.0 ms`.

3. **Test Suite 3: Salience Coverage & Dramatic Arc Safety Test (`TEST_SALIENCE_MONTE_CARLO`)**

   * **Procedure:** Following Emily Short and Doug Sharp's QA methodology, run 5,000 randomized state-space trajectories across a pool of 100 Salience storylets (including premature ending storylets gated by `required_arc_phases = {3}`).

   * **Acceptance Criterion:** Zero premature Act 3 endings SHALL fire during Act 1 or Act 2, fallback defaults SHALL prevent any "No Storylet Found" starvation, and the test runner SHALL output a histogram flagging any orphaned storylets with 0 activations.

4. **Test Suite 4: Waypoint Conversation Redirection & Rollback Test (`TEST_WAYPOINT_DIVERSION`)**

   * **Procedure:** Initialize a `Node_WaypointConversation` with start topic `ThePeople`, NPC goal `Royalty`, and player-only trigger topic `Revolution` (unlocked via `PopularUnrest`). Simulate the NPC stepping `ThePeople -> God`, then simulate the player diverting back to `ThePeople`, and verify the next NPC step.

   * **Acceptance Criterion:** Because the `ThePeople -> God` edge has accrued `used_penalty_cost`, the NPC MUST route through `ThePeople -> Government` (revealing `PopularUnrest`), allowing the player to trigger `Revolution`. Calling `CancelAndRevert()` MUST restore all edge use counts and revoke the `PopularUnrest` quality.