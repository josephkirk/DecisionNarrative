---
name: storylet-content-deck
description: Use when an external MCP agent is asked to author or revise storylet content directly in a DecisionNarrative project, grounded in the user's active studio context.
---

# Storylet Content Authoring

The studio has no bundled prose generator or model provider. When the user asks an MCP agent to author or revise content, work directly with the current project through the local `narrative_platform` MCP tool; do not automate the browser UI.

## Context-first workflow

1. Call `workspace.context` first. Use its active view, focused entity, selection, project revision, and playtest revision to determine what the user is looking at.
2. Call `workspace.inspect` to verify project canon and the target record. If the context is absent or stale, do not guess which entity the user means; ask for the target.
3. Make only the requested change using `content.save`, `content.delete`, scenario operations, or playtest operations. Preserve revisions and unrelated data. Do not replace the whole project bundle for ordinary edits.
4. Run `workspace.validate` on material project changes and inspect the resulting record. Use the normal storylet approval operation only when the user explicitly asks for approval.
5. Report the changed IDs, validation result, and any remaining issue. Do not claim a change is visible in the browser until the user refreshes or the live UI confirms it.

## Storylet correctness

- Storylet prose and graph rules must agree. Use existing project entities and supported conditions/effects; do not invent canon as fact.
- Require one Start and one End, and complete Start-to-End paths for all nodes.
- A Go To Location node must be a direct Choice target and select an existing location.
- Keep role, location, emotion, and action tags consistent with existing categories; tag uniqueness is case-insensitive.
- Review every choice for clear intent and deterministic consequences.

## Boundaries

This workflow uses the agent's normal authoring ability and direct MCP operations; it does not invoke model providers, embedding/ranking services, or generation-job APIs. The current interaction snapshot guides target selection; it is not permission for unrelated changes.
