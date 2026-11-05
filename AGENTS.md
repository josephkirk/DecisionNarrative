# When research
conduct deep research and verify all source validity , build prototype code to check if a research topic ( algorithm, methods) is viable

# When coding
use graphify to query
call agent with small model to build project knowledge
use uv for python
use bun for typescript
use pnpm for javascript
use rust for anything lowlevel and highperformance
use C or C++ only if nessary like Unreal Code
use unsloth for lora trainning

#architecture
use mvvm
use data oriented design architecture
local first

# UX Skill
For frontend work, use the installed ux-skill tooling where relevant. If a project `DESIGN.md` exists, treat it as the visual-system source of truth.
Before generating frontend code:
1. Run `ux discover` to capture the project brief.
2. If given the user's own brand URL, inspect the rendered site and capture actual brand signals; pass those signals to `ux recommend`.
3. Run `ux recommend` and use its applicable style, palette, type, motion, and component guidance.
4. Run `ux design-md` only when creating or intentionally refreshing the visual contract; preserve an existing `DESIGN.md` unless the task requires a change.
5. Run `ux lint` after frontend generation and address high-severity findings.

See https://github.com/Laith0003/ux-skill for full documentation.
