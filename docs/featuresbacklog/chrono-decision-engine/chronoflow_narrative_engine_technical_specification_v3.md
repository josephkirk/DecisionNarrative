# TECHNICAL DESIGN DOCUMENT: CHRONOFLOW-NARRATIVE ENGINE
**Document ID:** CFN-SPEC-001  
**Version:** 3.0 (Engine-Agnostic Code Agent Specification)  
**Writing Standard:** ASD-STE100 (Simplified Technical English)  
**Target Audience:** Autonomous Code Agents, Systems Architects, Narrative Engineers  

---

## 1. PURPOSE AND SCOPE

### 1.1 System Purpose
This document specifies the architecture, data structures, and algorithms for the **ChronoFlow-Narrative Engine**. The system executes non-linear interactive narratives, systemic character relationships, and time-synchronized scenes. The system guarantees 100% deterministic world-state reversal and choice rollback.

### 1.2 Architectural Pillars
The system integrates five core engineering principles into one unified runtime engine:
1. **Layered Attributed World Graphs:** The system stores the game world state as a formal graph with four distinct semantic layers and mutates this state through reversible subgraph transformation rules.
2. **Unified Ludic and Narrative Agency:** Player gameplay actions (combat, movement, item interaction) and dialogue choices use the same signal pipeline to mutate the world graph.
3. **Zero-Sum Passion Economy ("Narrative Legos"):** Non-Player Characters (NPCs) use micro-passions, macro-passion bars, and zero-sum relational links to trigger modular narrative sub-graphs dynamically when values cross thresholds.
4. **Token-Driven Latent Scene Execution:** A fixed-point millisecond clock moves fixed-size tokens through graph nodes and evaluates half-open time windows to play scenes forward or backward.
5. **Call-History State Stack ("Story Defines World"):** The system saves an ordered log of node invocations instead of heavy entity snapshots. Each mutator node caches its previous values internally to allow 1-frame rollbacks, fast-forward story invokes, and multiplayer timeline synchronization.

### 1.3 Engine Independence
This specification does not depend on a specific game engine. A code agent can implement this specification from scratch in any object-oriented or data-oriented language (such as C++, Rust, or C#) or adapt it as an extension to existing node-graph frameworks.

---

## 2. DEFINITIONS AND GLOSSARY (ASD-STE100 COMPLIANT)

* **Graph Asset:** A static, immutable data definition of nodes, pins, and edges.
* **Graph Instance:** An active runtime execution of a Graph Asset.
* **Signal Token:** A fixed-size data packet that moves from an output pin to an input pin to trigger node execution.
* **World Blackboard:** The central database that holds the Layered World Graph and all variable qualities.
* **Call History:** An ordered list of completed node invocations that defines the current state of the story and the world.
* **Story Invoke:** An operation that finds a path from the active nodes to a target node and completes all nodes on that path in one frame.
* **Rollback:** An operation that reads the Call History in reverse order and executes the inverse function of each node.
* **Star NPC:** A major character with individual micro-passions and a calculated macro-passion bar.
* **Drone NPC:** A minor character whose disposition value equals the average macro-passion of the Star NPCs in the same faction or location.

---

## 3. SYSTEM ARCHITECTURE OVERVIEW

```
+===================================================================================+
|                         CHRONOFLOW-NARRATIVE ENGINE CORE                          |
+===================================================================================+
|                                                                                   |
|  [1. WORLD BLACKBOARD & LAYERED GRAPH DATABASE]                                   |
|   ├── Layer 0: Locations   (Zones, Rooms, Portals)                                |
|   ├── Layer 1: Characters  (Star NPCs, Drone NPCs, Zero-Sum Passion Edges)        |
|   ├── Layer 2: Items       (Currencies, Tools, Inventory Ownership Edges)         |
|   └── Layer 3: Facts       (Progress Counters, Menaces, World Flags)              |
|          ▲                                                       │                |
|          │ (Mutates State & Caches Old Value)                    │ (Evaluates     |
|          │                                                       ▼  Thresholds)   |
|  [2. TOKEN-DRIVEN FLOW GRAPH RUNTIME]                                             |
|   ├── Fixed-Point Clock (Integer Milliseconds)                                    |
|   ├── Active Token Ring-Buffer (Signal, Execution, and State Tokens)              |
|   ├── Atomic Node Library (Mutators, Waiters, Logic Gates, Scene Sections)        |
|   └── Scene Composer & Interrupter (Resolves Actor Conflicts & Gameplay Breaks)   |
|          ▲                                                       │                |
|          │ (Replays / Unwinds Node Calls)                        │ (Appends       |
|          │                                                       ▼  Invocations)  |
|  [3. CHRONO CALL-HISTORY & RECONCILIATION ENGINE]                                 |
|   ├── Global Call History Stack (Main Story Spine)                                |
|   ├── Scoped Mission History Stacks (Isolated Sub-Graphs for Local Rewind)        |
|   ├── Backward Path-Seeker (1-Frame Story Invoke to Point-of-Interest)            |
|   └── Timeline Reconciler (LCA Rollback + Invoke for Branch Jumps & Co-op Sync)   |
+===================================================================================+
```

---

## 4. MODULE 1: LAYERED WORLD GRAPH AND PASSION BLACKBOARD

### 4.1 Requirements
* **REQ-WB-01:** The World Blackboard SHALL store world entities as vertices (`WorldVertex`) and relationships as directed edges (`WorldEdge`) across four layers: `LOCATION`, `CHARACTER`, `ITEM`, and `FACT`.
* **REQ-WB-02:** Every graph transformation rule SHALL define a Left-Hand Side pattern (`LHS_Pattern`) to match and a Right-Hand Side mutation (`RHS_Mutation`) to apply.
* **REQ-WB-03:** When the system applies a graph transformation rule, the executing node SHALL record the removed vertices, removed edges, and overwritten attribute values in a local cache to permit exact reversal.
* **REQ-WB-04:** Each Star NPC SHALL contain 1 to $N$ Micro-Passions. Each Micro-Passion SHALL have a visibility flag (`is_hidden`) and a weight (`weight`).
* **REQ-WB-05:** The system SHALL calculate the Macro-Passion of a Star NPC as the clamped weighted sum of its Micro-Passions plus a trust inertia modifier.
* **REQ-WB-06:** When a player action changes a Micro-Passion, the system SHALL propagate inverse changes across all Zero-Sum Edges connected to that Micro-Passion.
* **REQ-WB-07:** When a Star NPC Macro-Passion crosses a defined Threshold (upward or downward), the system SHALL emit a trigger event to activate or cancel the associated Graph Asset ("Narrative Lego").

### 4.2 Abstract Data Structures

```python
# --- LAYERED WORLD GRAPH STRUCTURES ---

enum GraphLayer:
    LOCATION = 0
    CHARACTER = 1
    ITEM = 2
    FACT = 3

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

### 4.3 Algorithms: Zero-Sum Passion Mutation and Reversal

```python
class WorldBlackboard:
    vertices: Map<string, WorldVertex>
    edges: Map<string, WorldEdge>
    stars: Map<string, StarNPCComponent>

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

---

## 5. MODULE 2: TOKEN-DRIVEN FLOW GRAPH CORE

### 5.1 Requirements
* **REQ-FG-01:** The Flow Graph Runtime SHALL separate static graph topology (`FlowGraphAsset`) from runtime state (`FlowGraphInstance`).
* **REQ-FG-02:** Every node in a `FlowGraphAsset` SHALL have a persistent, globally unique identifier (`node_guid`).
* **REQ-FG-03:** The runtime SHALL drive execution through a flat array of fixed-size `SignalToken` structures to maximize CPU cache locality.
* **REQ-FG-04:** The runtime SHALL classify every node into one of four functional types:
  1. `MUTATOR`: Immediate execution that changes world or blackboard state and caches previous values in the node instance.
  2. `WAITER`: Latent execution that halts the signal token until an external gameplay verb or condition occurs.
  3. `LOGIC_GATE`: Routing node (`ALL` / Logical AND, `ANY` / Logical OR, `CHOICE`, `POI`).
  4. `SCOPED_CONTAINER`: A node that encapsulates an isolated sub-graph (`MISSION`) or a time-windowed timeline (`SCENE_SECTION`).
* **REQ-FG-05:** Every `FlowNodeInstance` SHALL implement three execution interfaces:
  1. `ExecuteNormal(input_pin)`: Standard runtime behavior.
  2. `ForceComplete(chosen_output_pin)`: Instant 1-frame execution for save-loading and Story Invokes.
  3. `CancelAndRevert()`: Instant 1-frame inverse execution that restores the node's internal cached state.

### 5.2 Abstract Data Structures

```python
enum NodeCategory:
    MUTATOR = 0
    WAITER = 1
    LOGIC_GATE = 2
    SCOPED_CONTAINER = 3

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
    abstract function ForceComplete(chosen_output_pin: string) -> void

    # 1-Frame inverse execution for Rollback
    abstract function CancelAndRevert() -> void
```

### 5.3 Concrete Atomic Node Implementations (Pseudocode)

```python
# 1. WORLD MUTATOR EXAMPLE: Set Variable / Passion Node
class Node_SetVariable extends FlowNodeInstance:
    variable_key: string
    target_value: AttributeValue
    
    # In-Node Inverse Cache (LIFO Stack supports cyclic loops in graphs)
    cached_previous_values: Stack<AttributeValue>

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        ApplyMutation()
        state = COMPLETED
        return EmitTokenOnPin("Out", token.time_budget_ms)

    function ForceComplete(chosen_output_pin: string) -> void:
        ApplyMutation()
        state = COMPLETED

    function ApplyMutation() -> void:
        old_val = Blackboard.GetFact(variable_key)
        cached_previous_values.Push(old_val)
        Blackboard.SetFact(variable_key, target_value)

    function CancelAndRevert() -> void:
        if not cached_previous_values.IsEmpty():
            prev_val = cached_previous_values.Pop()
            Blackboard.SetFact(variable_key, prev_val)
        state = IDLE


# 2. WORLD MUTATOR EXAMPLE: Show / Hide World Entity
class Node_SetEntityVisibility extends FlowNodeInstance:
    make_visible: bool
    cached_previous_visibility: Stack<bool>

    function ApplyMutation() -> void:
        actor = WorldAdapter.FindActorByPath(entity_path)
        cached_previous_visibility.Push(actor.IsVisible())
        actor.SetVisible(make_visible)
        actor.SetCollisionAndAIActive(make_visible)

    function ForceComplete(chosen_output_pin: string) -> void:
        ApplyMutation()
        state = COMPLETED

    function CancelAndRevert() -> void:
        if not cached_previous_visibility.IsEmpty():
            was_visible = cached_previous_visibility.Pop()
            actor = WorldAdapter.FindActorByPath(entity_path)
            actor.SetVisible(was_visible)
            actor.SetCollisionAndAIActive(was_visible)
        state = IDLE


# 3. CONDITION WAITER EXAMPLE: Take Item / Ludic Verb Listener
class Node_WaitTakeItem extends FlowNodeInstance:
    player_id: string

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        state = ACTIVE_LATENT
        EventBus.Subscribe("VERB_ITEM_PICKED_UP", entity_path, OnItemPickedUpCallback)
        return EmptyList() # Halts token until event fires

    function OnItemPickedUpCallback() -> void:
        EventBus.Unsubscribe("VERB_ITEM_PICKED_UP", entity_path)
        state = COMPLETED
        FlowEngine.RecordCallHistory(this.node_guid, "In", "Out")
        FlowEngine.EnqueueToken(this.node_guid, "Out")

    function ForceComplete(chosen_output_pin: string) -> void:
        # Synthesizes the player action automatically in 1 frame
        EventBus.Unsubscribe("VERB_ITEM_PICKED_UP", entity_path)
        WorldAdapter.TransferItemToInventory(entity_path, player_id)
        state = COMPLETED

    function CancelAndRevert() -> void:
        EventBus.Unsubscribe("VERB_ITEM_PICKED_UP", entity_path)
        if state == COMPLETED:
            # Puts the item back into the 3D world at its original transform
            WorldAdapter.ReturnItemFromInventoryToWorld(entity_path, player_id)
        state = IDLE


# 4. LOGIC GATES: ALL (AND) & ANY (OR)
class Node_LogicALL extends FlowNodeInstance:
    triggered_inputs_stack: Stack<Set<string>>
    current_triggered_inputs: Set<string>

    function ExecuteNormal(pin_name: string, token: SignalToken) -> List<SignalToken>:
        current_triggered_inputs.Add(pin_name)
        if current_triggered_inputs.Count() == input_pins.Count():
            triggered_inputs_stack.Push(current_triggered_inputs.Copy())
            current_triggered_inputs.Clear()
            state = COMPLETED
            return EmitTokenOnPin("Out", token.time_budget_ms)
        state = ACTIVE_LATENT
        return EmptyList()

    function ForceComplete(chosen_output_pin: string) -> void:
        triggered_inputs_stack.Push(AllInputPinNames())
        current_triggered_inputs.Clear()
        state = COMPLETED

    function CancelAndRevert() -> void:
        if not triggered_inputs_stack.IsEmpty():
            triggered_inputs_stack.Pop()
        current_triggered_inputs.Clear()
        state = IDLE
```

---

## 6. MODULE 3: SCRUBBABLE SCENE SECTIONS AND TIMELINE COMPOSER

### 6.1 Requirements
* **REQ-SC-01:** A `Node_SceneSection` SHALL maintain an internal fixed-point clock in integer milliseconds (`section_clock_ms`).
* **REQ-SC-02:** Timeline events inside a section SHALL have a `start_time_ms`, a `duration_ms` (0 for point events), and a specific action payload (Animation Workspot, Camera Cut, Audio/VO, LookAt, or VFX).
* **REQ-SC-03:** During forward playback, the section SHALL evaluate events against the left-inclusive, right-exclusive time window $[t_0, t_1)$ where $t_0 < t_1$.
* **REQ-SC-04:** During backward scrubbing or rollback, the section SHALL evaluate events against the inverted time window $(t_1, t_0]$ where $t_1 < t_0$.
* **REQ-SC-05:** The section SHALL decompose overlapping events into `ActionPart` structures (`START`, `REGULAR`, `END`), sort them by `action_type` priority and `target_entity_id` combiner, and dispatch them to the `SceneComposer`.
* **REQ-SC-06:** The `SceneComposer` SHALL arbitrate conflicts when multiple concurrent scene instances issue commands to the same actor.

### 6.2 Algorithm: Reversible Time-Window Event Processing

```python
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

---

## 7. MODULE 4: CALL-HISTORY ENGINE, ROLLBACK, AND STORY INVOKES

### 7.1 Requirements
* **REQ-CH-01:** The system SHALL record every completed node invocation as a lightweight `CallHistoryEntry` in an ordered array.
* **REQ-CH-02:** To save the game state, the system SHALL serialize only the `CallHistoryEntry` arrays (Global Call History and active Scoped Mission Call Histories) and random seeds.
* **REQ-CH-03:** To load a saved game state, the system SHALL iterate through the saved `CallHistoryEntry` array from index `0` to `N-1` and invoke `ForceComplete(chosen_output_pin)` on each corresponding node in one frame.
* **REQ-CH-04:** A `Node_MissionSubGraph` SHALL isolate its internal node invocations in a local `mission_call_history` list. Rolling back a Mission node SHALL unwind only its `mission_call_history` list without changing the Global Call History.
* **REQ-CH-05:** The `InvokeStoryToPOI` algorithm SHALL find a valid execution path by traversing backward from a target `Node_POI` to the currently active nodes.
  * When backward traversal encounters a `Node_LogicALL` (AND gate), it SHALL include **all** incoming upstream branches in the invoke path.
  * When backward traversal encounters a `Node_LogicANY` (OR gate) or `Node_Choice`, it SHALL select the upstream pin specified in the `declared_choice_routes` map.
* **REQ-CH-06:** The `ReconcileTimelines` algorithm SHALL synchronize a current Call History ($H_{current}$) to a target Call History ($H_{target}$) in one frame by:
  1. Finding the Lowest Common Ancestor (LCA) index where $H_{current}$ and $H_{target}$ diverge.
  2. Calling `CancelAndRevert()` in reverse order from the top of $H_{current}$ down to the LCA index.
  3. Calling `ForceComplete()` in forward order from the LCA index to the end of $H_{target}$.

### 7.2 Abstract Data Structures

```python
struct CallHistoryEntry:
    node_guid: UUID
    executed_input_pin: string
    chosen_output_pin: string          # Critical for Choice nodes and ANY gates

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

### 7.3 Algorithm: 1-Frame Backward Path-Seeking ("Story Invoke")

```python
class ChronoInvokeManager:
    
    function InvokeStoryToPOI(
        graph: FlowGraphInstance, 
        target_poi_guid: UUID, 
        declared_routes: Map<UUID, string> # Maps ANY/Choice node_guid -> desired upstream pin_name
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
                
                # Record the required upstream choice output pin
                upstream_node = graph.GetNode(edge.source_node_guid)
                if upstream_node.state != COMPLETED:
                    nodes_to_invoke_forward.Add(CallHistoryEntry(
                        node_guid: edge.source_node_guid,
                        executed_input_pin: "In",
                        chosen_output_pin: edge.source_pin_name
                    ))

        SeekBackwardRecursive(target_poi_guid)

        # Step 2: ForceComplete all collected nodes in topological forward order in 1 frame
        for each entry in DeduplicatePreserveOrder(nodes_to_invoke_forward):
            node = graph.GetNode(entry.node_guid)
            node.ForceComplete(entry.chosen_output_pin)
            graph.global_call_history.Add(entry)

        # Step 3: Execute Target POI Node (Teleports player to POI respawn transform)
        poi_node = graph.GetNode(target_poi_guid)
        poi_node.ForceComplete("Out")
        graph.global_call_history.Add(CallHistoryEntry(target_poi_guid, "In", "Out"))
```

### 7.4 Algorithm: Timeline Reconciliation (Choice Branch Jump & Co-op Sync)

```python
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
            node.ForceComplete(entry_to_invoke.chosen_output_pin)
            current_history.Add(entry_to_invoke)
```

---

## 8. MODULE 5: LIVE GRAPH PATCHING (CALL-HISTORY REBASING)

### 8.1 The Patching Problem
When a saved game stores a `CallHistoryEntry` list, modifying the `FlowGraphAsset` in a game patch (deleting nodes, inserting new prerequisite nodes, or rewiring pins) makes the saved Call History invalid if replayed blindly.

### 8.2 Requirements
* **REQ-GP-01:** When loading a saved `CallHistoryEntry` array against an updated `FlowGraphAsset`, the engine SHALL execute the `RebaseCallHistoryOnPatch` algorithm before replaying the history.
* **REQ-GP-02:** If a `node_guid` in the saved Call History no longer exists in the updated `FlowGraphAsset`, the Rebaser SHALL remove that entry from the Call History.
* **REQ-GP-03:** If the updated `FlowGraphAsset` inserts new uncompleted nodes between two nodes `$N_A$` and `$N_B$` that were both completed in the saved Call History, the Rebaser SHALL run `SeekBackwardRecursive` from `$N_B$` to `$N_A$` and splice the synthesized `CallHistoryEntry` records into the history list.

### 8.3 Algorithm: Call-History Patch Rebaser

```python
    function RebaseCallHistoryOnPatch(
        updated_graph_asset: FlowGraphAsset,
        saved_history: List<CallHistoryEntry>
    ) -> List<CallHistoryEntry>:
        
        rebased_history = List<CallHistoryEntry>()
        
        # 1. Strip deleted nodes (Tombstone Garbage Collection)
        valid_saved_entries = List<CallHistoryEntry>()
        for each entry in saved_history:
            if updated_graph_asset.HasNode(entry.node_guid):
                valid_saved_entries.Add(entry)

        # 2. Verify topological continuity and splice newly inserted nodes
        for i from 0 to (valid_saved_entries.Count() - 1):
            current_entry = valid_saved_entries[i]
            rebased_history.Add(current_entry)
            
            if i + 1 < valid_saved_entries.Count():
                next_entry = valid_saved_entries[i + 1]
                
                # Check if a direct edge still exists or if a designer inserted new nodes in between
                if not updated_graph_asset.HasDirectEdge(current_entry.node_guid, next_entry.node_guid):
                    missing_bridge_entries = FindShortestBridgePath(
                        updated_graph_asset, 
                        source: current_entry.node_guid, 
                        target: next_entry.node_guid
                    )
                    rebased_history.AddAll(missing_bridge_entries)

        return rebased_history
```

---

## 9. REFERENCE MAPPING TO UNREAL ENGINE (`MothCocoon/FlowGraph`)

If the code agent or developer implements this specification inside Unreal Engine using `MothCocoon/FlowGraph`, map the abstract types as follows:

| Abstract Specification Type | `MothCocoon/FlowGraph` / UE5 Equivalent | Implementation Note |
| :--- | :--- | :--- |
| `WorldBlackboard` | `UGameInstanceSubsystem` (`UChronoWorldSubsystem`) | Holds the Layered World Graph, `FGameplayTag` attributes, and Star Passion components. |
| `FlowGraphAsset` | `UFlowAsset` | Static graph topology containing `FGuid` node identifiers. |
| `FlowNodeInstance` | `UFlowNode` subclass (`UFlowNode_ChronoAtomic`) | Override `ExecuteInput()`, `OnLoad()`, and add `ForceComplete()` and `CancelAndRevert()`. |
| `Node_MissionSubGraph` | `UFlowNode_SubGraph` subclass | Add `TArray<FFlowCallHistoryEntry> LocalMissionHistory` marked with `UPROPERTY(SaveGame)`. |
| `entity_path` (String Binding) | `FSoftObjectPath` or `FGameplayTag` via `UFlowComponent` | Decouples the graph asset from hard level references; resolves actors at runtime through `UFlowSubsystem`. |
| `Node_SceneSection` | `UFlowNode` wrapping `ULevelSequence` / Smart Objects | Drives `ULevelSequencePlayer` playhead via fixed-point `section_clock_ms` forward or in reverse. |

---

## 10. VERIFICATION AND AUTOMATED QA REQUIREMENTS

To verify the implementation, the code agent SHALL implement the following three automated test suites:

1. **Test Suite 1: Bijective Rollback Identity Test (`TEST_ROLLBACK_IDENTITY`)**
   * **Procedure:** Initialize a `WorldBlackboard` and `FlowGraphInstance`. Compute a 64-bit state hash `Hash_0` of all vertices, edges, attributes, and Star NPC passions. Execute 500 random nodes (including `Node_SetVariable`, `Node_MutatePassion` with Zero-Sum links, and `Node_LogicALL`). Then execute `ReconcileTimelines` with an empty target history `[]`. Compute `Hash_Final`.
   * **Acceptance Criterion:** `Hash_Final` MUST equal `Hash_0` with zero floating-point drift and zero leaked event subscriptions.
2. **Test Suite 2: 1-Frame Story Invoke Equivalence Test (`TEST_INVOKE_EQUIVALENCE`)**
   * **Procedure:** Play through a branching graph of 50 nodes normally over multiple frames to reach `POI_End`, recording `Hash_Normal`. Reset the world. Run `InvokeStoryToPOI(POI_End)` in a single frame using the same choice route declarations, recording `Hash_Invoked`.
   * **Acceptance Criterion:** `Hash_Invoked` MUST equal `Hash_Normal`, and execution time MUST be less than `2.0 ms`.
3. **Test Suite 3: Headless Monte Carlo Bot & "Teddy Bear" Stress Test (`TEST_BOT_TRAVERSAL`)**
   * **Procedure:** Spawn a test environment containing mock interactable entities ("Teddy Bears") bound via `entity_path`. Run a headless bot for 10,000 steps that randomly completes Waiter nodes, triggers `RollbackMissionToStart()` on 20% of Mission nodes, and reconciles divergent timelines between two simulated Co-op clients.
   * **Acceptance Criterion:** Zero unhandled exceptions, zero orphaned latent tokens, and 100% state parity between synchronized Co-op clients.