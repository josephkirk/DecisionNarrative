# Play: the studio's playtest surface

**Status:** Design spec for implementation (see `docs/superpowers/plans/2026-11-05-nonlinear-rework.md`, Phases 2, 3, 4 and 5)
**Parent design:** [proposal-human-first-nonlinear-narrative.md](proposal-human-first-nonlinear-narrative.md)
**Visual contract:** [`DESIGN.md`](../../DESIGN.md) is the source of truth for colour, type, spacing and shape.

## 1. What changed and why

The previous ChronoFlow playtest view, deleted on `refactor-backend`, showed the engine's internals to the author:

- a "Tick 16 ms" button
- pending tokens and node GUIDs
- the call history
- a raw "Send signal event" box

It was useful for engine debugging, but it made the author operate the engine before they could read their story.

**Play** reverses this. **You read and choose. The engine runs itself until it needs you.** Engine internals are hidden behind a single *Why?* affordance and an *Advanced* fold.

## 2. Principles

1. **One click to play.** A primary **Play** button sits in the top toolbar on every screen. Storylet editors also offer **Play from here**.
2. **Story first.** The prose column is the centre of the screen, set in EB Garamond at a 65–75 character measure. Everything else is optional context.
3. **Never ask the author to tick.** The run auto-advances through waiters and timeline sections until there is a choice, a move or an ending.
4. **Every "nothing happens" has a why.** When no content is available, Play explains the nearest misses in plain language, with a link to the storylet.
5. **Undo is free; branching is automatic.** Every step is on a timeline. Clicking an earlier step rewinds. Choosing differently from there creates a branch, with no dialog.
6. **Edit and continue.** A title in Play opens its editor. When content changes, a banner offers **Reload run**: the same actions are replayed on the new content, and Play reports where the run diverged.
7. **Small visible surface.** One prose column, one *where & when* strip, one step timeline and a closed-by-default *Inspect* panel. No more.

## 3. Layout

```text
┌─ Studio toolbar ─────────────────────────────────────────────── [▶ Play] ┐
├─ Project tree ─┬─ Play ─────────────────────────────────────┬─ Inspect ───┤
│                │ Weir's Crossing · Run 3 · branch b        │ (closed by   │
│                │ ┌ Where & when ───────────────────────────┐│  default;    │
│                │ │ [Mara] [Teodor]   Harbor · D1 evening ·  ││  [I] opens)  │
│                │ │ 1 action left     Go to: Lighthouse  Inn ││             │
│                │ │                    Customs  [End turn]   ││ Knows │      │
│                │ └──────────────────────────────────────────┘│ World │      │
│                │                                              │ Story │      │
│                │   The Lantern on the Water        ✎ edit     │ so far│      │
│                │                                              │             │
│                │   The shingle is black glass under the rain. │             │
│                │   A lantern bobs where no boat should be…    │             │
│                │                                              │             │
│                │   1  Wade out toward it                      │             │
│                │   2  Call out to the boat                    │             │
│                │   3  Turn back                (needs: lamp)  │             │
│                │                                              │             │
│                │ ── Also here ─────────────────────────────── │             │
│                │   Harbor master's tide table                 │             │
│                ├──────────────────────────────────────────────┤             │
│                │ Steps ●─●─●─●─◉     ↶ Undo    branch: a | b  │             │
└────────────────┴──────────────────────────────────────────────┴─────────────┘
```

### 3.1 Start screen (`/play`, no run selected)

- **Start**: *From the beginning* (default), or *From a storylet…* (a searchable picker).
- **Scenario** dropdown. Defaults to the project's first scenario; hidden when there is only one.
- **Seed**: hidden under *Options*; defaults to random and is shown on the run header.
- **Recent runs** list: title, branch, step count, ending badge, and a "content changed" dot when the run is stale.
- Secondary action: **Let bots play** (Phase 5). Runs the explorer and shows the coverage report (§6).

### 3.2 Where & when strip

| Element | Shown when | Behaviour |
|---|---|---|
| Participant tabs (avatar + name) | there is more than one human participant | hot-seat: the active participant is highlighted; tabs of participants who have no actions left are dimmed |
| Place · time | always (time only if the project has a clock) | e.g. `Harbor · D1 evening` |
| Actions left | the project has a clock | `1 action left` |
| **Go to:** chips | there are connections | a disabled chip shows its reason on hover and focus (e.g. *Causeway closed until D3 morning*, *Needs: tide table*) |
| **End turn** | the project has a clock | spends the remaining actions |

### 3.3 Story column

- **Scene:** title (with an edit link), speaker lines, and prose (EB Garamond, `body-lg`).
- **Choices:** numbered 1–9 with keyboard shortcuts. Choices beyond 9 are clickable only. A disabled choice shows its requirement in plain language.
- **Also here:** storylets eligible at this place that the participant may start, for QBN-style hubs.
- **Nothing here:** the text *Nothing happens here right now.* followed by **Why?**, which expands up to 5 near misses: *Customs Ledger — needs Benedek to be away (he is here until evening)*.
- **Witnessed / told** notes appear inline, in small muted Geist: *Teodor saw this.* Rosa *heard you.*
- **Ending:** ending title, a one-line *path summary* (step count, branch), and the buttons **Play again**, **Rewind** and **Back to start**.

### 3.4 Step timeline

- One dot per step. Hover or focus shows `participant · action · time`.
- Click a dot to rewind to it. The steps after it stay visible as a faded *ghost* branch until you act; acting turns them into a separate branch.
- **Undo** (Ctrl+Z) means "rewind one step".
- A branch selector appears only once a second branch exists.

### 3.5 Inspect panel (closed by default; **I** toggles it)

| Tab | Content |
|---|---|
| **Knows** | the active participant's facts, each with a kind badge (evidence, witnessed, told, rumour, recollection, testimony, inference) and its source step as a link to the timeline dot. Testimony and rumour are visibly marked as *not confirmed*. |
| **World** | the properties and tags that changed in the last step (before → after), then *Show all*. |
| **Story so far** | the canon chronicle in story-time order. Entries this participant has learned show their telling number (①②③); untold entries are grey bars. |
| **Advanced** (a fold inside World) | read-only engine details: run ID, seed, content revision, current node, the pending interaction kind. No controls. |

## 4. Keyboard

| Key | Action |
|---|---|
| 1–9 | choose |
| G then 1–9 | go to the n-th connection |
| E | end turn |
| Ctrl+Z | undo |
| I | toggle Inspect |
| Tab / Shift+Tab | next / previous participant (hot-seat) |
| Esc | close Inspect, or collapse Why? |

## 5. States

| State | What the user sees |
|---|---|
| Loading | structural skeleton of the strip, the column and the timeline |
| Stale content | banner: *Content changed since this run started.* **Reload run**. After a reload: *Replayed 12 of 14 steps; diverged at step 13 (choice no longer exists)*. |
| Stalled (engine safety cap) | *The story stopped advancing at "‹storylet›".* **Open storylet**. Not a crash and not silent. |
| Error (validation/revision) | inline under the strip, with role="alert", and the run left unchanged |
| No project content | empty state with one action: **Create a storylet** |

## 6. Coverage report (Phase 5)

The report opens from **Let bots play**.

- **Runs and seed:** default 2,000 runs; editable up to 50,000.
- **Endings:** a horizontal bar per ending, with its percentage.
- **Milestones:** reachability for each declared participant configuration (solo, co-op), e.g. ✔ 74% · ✖ 0%.
- **Holes:** each declared intent probe with its count and an *Open storylet* link for the probe's expected routes.
- **Invariants:** red when the count is above 0. Each violation has a **Replay seed** button, which opens Play on that seed with the bot's actions as steps, stopped at the violating step.
- **Dead air:** the share of actions with only fallback content, listed per location.

## 7. Accessibility and the visual contract

- Interactive targets are at least 44×44 px; the 2026-10-24 UI review lists this as P1.
- Every form control has a visible label.
- Status changes are announced in an `aria-live="polite"` region; errors use `role="alert"`.
- Use DESIGN.md tokens only:
  - 0 px radius
  - hairline borders
  - no glow and no `backdrop-filter`
  - amber for choices, cyan for focus and current step, rose for endings and invariant failures, violet for knowledge kinds
- Motion: 120–200 ms ease-out fades only; honour `prefers-reduced-motion`.
- Run `ux lint` after implementation and fix high-severity findings.

## 8. Out of scope

- Networked multiplayer. Co-op is hot-seat on one screen.
- Runtime LLM features in Play.
- Editing content inside Play.
- Scenario authoring UI. Scenario CRUD stays in HTTP/MCP; Play only picks a scenario.
