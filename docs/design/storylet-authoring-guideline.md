# Storylet Content Authoring Guidelines

These guidelines apply to human-authored storylet content and to external MCP agents acting on a user's explicit request. The studio does not call a text, image, embedding, or ranking model. It does not generate storylet drafts.

## Keep prose and executable rules aligned

A storylet is both authored prose and a deterministic operator on project state. Its graph, preconditions, role requirements, and effects are explicit data. The Rust engine owns eligibility, validation, state changes, and replay; prose never overrides those rules.

- Reference only project-owned characters, locations, tags, qualities, events, choices, and effects.
- State prerequisites as typed conditions. Do not rely on hidden facts or prose-only assumptions.
- Describe consequences that follow from the authored effects. Do not imply untracked items, location changes, or permanent character outcomes.
- Preserve unknown canon as unknown; do not silently add project facts.

## Storylet graph requirements

- Exactly one Start and one End node.
- Every node lies on a complete path from Start to End.
- Every Choice target and effect refers to a valid project record or supported operation.
- A Go To Location node must be targeted directly by a Choice and select an existing project location.
- Conditions and variables use the correct global or storylet-local scope.
- Review each branch for meaningful player intent and an observable consequence.

## Writing and revision

Use concrete, character-specific actions. Let tactics, objects, and physical positioning carry subtext. Distinguish speakers through attention, rhythm, and choice of words. Keep sensory details direct and relevant to the action. Remove filler openings, explanatory codas, repeated phrasing, and stock gestures when they weaken the scene.


## MCP agent workflow

When an external MCP agent is asked to work on the project, it first reads `workspace.context`. It uses the active view, focused draft, selection, project revision, and playtest revision to choose the target. If canonical persisted content is needed, it fetches only the focused record with `content.get`; `workspace.inspect` is reserved for cross-entity or whole-project checks. Make requested changes through ordinary content, scenario, or playtest operations; do not automate the UI or invoke a separate model-generation endpoint. Preserve revisions, validate changes, and do not approve content unless the user requested approval.

The active interaction context is local UI state provided to MCP so the agent can work on the item the user is viewing. It is not a prompt-generation service, model provider, or authorization to make unrelated project changes.
