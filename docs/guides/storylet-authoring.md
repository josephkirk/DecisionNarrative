# Storylet authoring guide

Use this checklist for any nonlinear narrative project created with the studio.

## Before generation

- Define only the qualities, characters, locations, events, and relationships this project needs.
- Give each character a role, goals, boundaries, knowledge, relationships, and a distinct voice.
- Describe each location through observable details and facts relevant to play; assign tags that storylets can use as reusable location constraints.
- State a scene's purpose, role requirements, trigger conditions, and intended outcomes.
- Keep trigger predicates and choice effects explicit, typed, and reviewable. Avoid rules that depend on hidden or omniscient facts.

## Open the graph editor

Run `bun run play`, open **Storylets**, then choose **New Storylet** or open an existing storylet to edit it. The Storylets page is a card list; the graph canvas is inside the storylet editor. The starter graph is Start → Dialog → End.

Use the shared inline tag editor: type inside the tag container to filter existing tags, use Up/Down and Enter to choose a suggestion, or press Enter to create a tag. Remove a selected tag with its × control; no separate Add button is needed. Open **Tag Manager** to review character roles, location tags, emotion tags, and action tags by distinct entity count; **Open** navigates to the Codex record or the storylet graph item that uses the tag. Tag names are trimmed and deduplicated case-insensitively within each tag category.


## Review each generated storylet

- Confirm the bound roles can plausibly be present and act in the scene.
- Check that each choice is understandable and its conditions and effects match the designer-authored outcome contract.
- Check who can observe each event and which characters can learn from it.
- Review dialogue, action, character voice, continuity, and prose audit findings. The audit reports findings without changing text.
- Ensure every storylet graph has exactly one Start and one End, with every node reachable from Start and able to reach End; validate before approval.
- Set storylet location tags instead of binding it to a location ID. A storylet with matching tags is eligible at any location sharing at least one tag.
- Approve the exact content revision before adding it to a playable scenario.

## Playtest

- Start from the intended scenario state and seed.
- Inspect eligible and rejected storylets, including the director's reasons.
- Try different player actions and locations; observe NPC actions, witnesses, and downstream world changes.
- Use the event trace and butterfly graph to follow causal links. Fork a run when comparing alternate choices.
- Restart tests from a fresh playtest session; project authoring data remains separate from disposable session state.

The studio is project-neutral. Example settings, characters, and storylets are optional project content, never platform defaults.
