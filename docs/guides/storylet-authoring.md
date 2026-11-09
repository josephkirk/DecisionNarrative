# Storylet authoring guide

Use this checklist for any nonlinear narrative project created with the studio. The studio does not generate content through a model service. Designers may author in the UI or ask an MCP agent to edit project data directly.

## Prepare a project

- The global **World** is an intrinsic Entity under **Entities**, not an Entity Template. Create authored runtime objects as **Entity Templates**. Characters, locations, and facts are identified by applied `system.character`, `system.location`, and `system.fact` trait instances; an entity may carry more than one of these traits. Items use the inventory plugin's Item trait. The intrinsic World is the entity whose ID is `world`.
- Keep traits small and composable: Character Settings holds role and location references; Location Settings holds zone connections; passion data uses focused repeatable traits. Add project-specific traits instead of growing a mandatory all-purpose schema.
- Give each character a registered `character.*` role tag, goals, boundaries, knowledge, relationships, and a distinct voice. Describe locations through observable details and registered `location.*` tags.
- State a scene's purpose, role requirements, trigger conditions, and intended outcomes.
- Keep trigger predicates and choice effects explicit, typed, and reviewable.

## Open the graph editor

Run `bun run play`, open **Storylets**, then choose **New Storylet** or open an existing storylet to edit it. The starter graph is Start → Dialog → End.
Each storylet has separate **Preconditions** and **Story Graph** tabs. The Preconditions canvas replaces the old list editor: use `Get Entity -> Get Entity Property -> Compare` for typed property checks; boolean values feed `And`/`Or` or the Boolean input on `Condition Node`. Connect a flow edge from Start into the Condition Node; its `true`/`false` outputs route to End nodes. `Has Entity Tag` produces a boolean data value and checks a registered tag on the selected entity (for example, `location.market.visited` or `character.guide.met`). The Story Graph uses the same boolean predicates, `And`/`Or` combinators, and Condition Node pattern. Add typed custom properties to Entity Templates, then connect `Get Entity` to `Get Entity Property` for reads or `Apply Command` for changes. `Apply Command` takes an Entity data input and a typed literal; `Set` is available for every property type, `Add`/`Subtract` for Integer/Float, and `Toggle` for Boolean. Changing the selected property updates the available operations. These nodes execute through Chrono.

Property controls are selected from the declared property schema of the connected Entity. For a dynamic EntityRef source, the controls offer only non-array properties common to every possible Entity, with matching types. `Get Entity` selects a runtime Entity; `Get Entity Property` and `Apply Command` use the selected declared property key, and the runtime stores custom properties under `prop.<key>`. Dotted notation is for gameplay tags. `Compare` and `Apply Command` literals must match the declared value type. Undeclared properties, mismatched property or literal types, and unsupported operations fail validation before save, bundle import, approval, or playtest.

Execution-flow sockets and wires are white; value pins and wires use a color for their value type (for example, Boolean, Integer, Float, String, or EntityRef). Connect `Get Entity` to the optional `target` input on `Add Entity Tag`, `Remove Entity Tag`, or `Has Entity Tag` to choose a runtime target; without that connection, each node keeps its configured target. `Apply Command` operations follow the selected Entity property type: Boolean offers Set/Toggle, Integer and Float offer Set/Add/Subtract, and other types offer Set. Changing the selected property resets an operation that is no longer valid to Set.

For TreeGraph entities, connect Get Entity to **Set Tree Position** or **Is Tree Position** in the Story Graph. Choose a TreeItem in the read-only preview. If you change the connected tree, choose a position in the new tree before saving. Set changes that tree's playtest pointer; Is provides a boolean to a Condition Node. The pointer starts unset and can move directly to any member of the tree.
The tree preview's **Fit** control shows the full graph; use the zoom controls to inspect individual items.


The Story Graph toolbar and canvas context menu expose the Chrono `QBN Storylet Hub`, `Salience Selector`, and `Waypoint Conversation` nodes.

Gameplay Tags are canonical, hierarchical identifiers with independently registered namespaces. Their roots and child tags describe gameplay state or categories; they do not classify entities. Entity classification comes from trait instances and traits may coexist. The intrinsic World is the entity with ID `world`. Use **Tag Manager** to add, rename, or remove tags. Referenced or protected tags cannot be changed, and parents with children cannot be renamed or removed. The **Open** action follows a tag reference to its Entity, Entity Template, or storylet graph item.

## Review each storylet

- Confirm each bound role uses a registered `character.*` tag and fits the scene. Character Settings supplies the role and location references; represent goals, needs, schedules, or other project-specific data with separate reusable traits.
- Check that each choice is understandable and its conditions and effects match the intended outcome.
- Check who can observe each event and which characters can learn from it.
- Ensure the Story Graph has exactly one Start and one or more End nodes, with every node reachable from Start and able to reach an End. An End marked terminal closes the run; an End with a jump transitions to another storylet and the jump takes precedence. A storylet is a dedicated ending only when every End is terminal with no jump. The separate Preconditions graph must contain only read-only predicate/value nodes, typed property comparisons, And/Or expressions, and Condition Node switches, plus at least one valid-candidate End. Save validates both graphs before writing; approval validates the saved revision.
- Use optional storylet location tags instead of binding a storylet to one location ID. A storylet with matching tags is eligible at any location sharing at least one tag; a storylet with no location tags is not location-gated.
- A Go To Location node must be targeted directly by a Choice and name an existing location Entity Template.
- Approve the exact content revision only when it is ready for playtest.

## Work through MCP

When an MCP agent is asked to act on the current project, it reads `workspace.context` first. It uses `content.get` for the focused persisted entity when needed, and reserves `workspace.inspect` for cross-entity or whole-project checks. It uses entity-level content/scenario/playtest operations directly; it does not automate the UI. Context guides target selection but is not permission for unrelated edits. Project replacement requires explicit confirmation, and approval remains a separate user decision.

## Playtest

- Start from the intended scenario state and seed.
- Inspect eligible and rejected storylets, including deterministic reasons.
- Try different player actions and locations; observe NPC actions, witnesses, and downstream world changes.
- Use the event trace and butterfly graph to follow causal links. Fork a run when comparing alternate choices.
- Restart tests from a fresh playtest session; project authoring data remains separate from disposable session state.

The studio is project-neutral. Example settings, characters, and storylets are optional project content, never platform defaults.
