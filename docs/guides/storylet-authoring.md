# Storylet authoring guide

Use this checklist for any nonlinear narrative project created with the studio. The studio does not generate content through a model service. Designers may author in the UI or ask an MCP agent to edit project data directly.

## Prepare a project

- Define only the qualities, characters, locations, events, and relationships the project needs.
- Give each character a role, goals, boundaries, knowledge, relationships, and a distinct voice.
- Describe locations through observable details and assign reusable location tags.
- State a scene's purpose, role requirements, trigger conditions, and intended outcomes.
- Keep trigger predicates and choice effects explicit, typed, and reviewable.

## Open the graph editor

Run `bun run play`, open **Storylets**, then choose **New Storylet** or open an existing storylet to edit it. The starter graph is Start → Dialog → End.

Use the shared inline tag editor: type inside the tag container to filter existing tags, use Up/Down and Enter to choose a suggestion, or press Enter to create a tag. Remove a selected tag with its × control; no separate Add button is needed. Open **Tag Manager** to review character roles, location tags, emotion tags, and action tags by distinct entity count; **Open** navigates to the Codex record or storylet graph item that uses the tag. Tag names are trimmed and deduplicated case-insensitively within each category.

## Review each storylet

- Confirm the bound roles can plausibly be present and act in the scene.
- Check that each choice is understandable and its conditions and effects match the intended outcome.
- Check who can observe each event and which characters can learn from it.
- Ensure every graph has exactly one Start and one End, with every node reachable from Start and able to reach End. Save validates the graph before writing; approval validates the saved revision.
- Use storylet location tags instead of binding a storylet to one location ID. A storylet with matching tags is eligible at any location sharing at least one tag.
- A Go To Location node must be targeted directly by a Choice and name an existing project location.
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
