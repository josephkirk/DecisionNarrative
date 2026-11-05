---
name: DecisionNarrative Studio
colors:
  surface: '#111317'
  surface-dim: '#111317'
  surface-bright: '#37393e'
  surface-container-lowest: '#0c0e12'
  surface-container-low: '#1a1c20'
  surface-container: '#1e2024'
  surface-container-high: '#282a2e'
  surface-container-highest: '#333539'
  on-surface: '#e2e2e8'
  on-surface-variant: '#bcc9cd'
  inverse-surface: '#e2e2e8'
  inverse-on-surface: '#2f3035'
  outline: '#869397'
  outline-variant: '#3d494c'
  surface-tint: '#4cd7f6'
  primary: '#4cd7f6'
  on-primary: '#003640'
  primary-container: '#06b6d4'
  on-primary-container: '#00424f'
  inverse-primary: '#00687a'
  secondary: '#bdc2ff'
  on-secondary: '#131e8c'
  secondary-container: '#2f3aa3'
  on-secondary-container: '#a8afff'
  tertiary: '#ffb95f'
  on-tertiary: '#472a00'
  tertiary-container: '#e79400'
  on-tertiary-container: '#563400'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#acedff'
  primary-fixed-dim: '#4cd7f6'
  on-primary-fixed: '#001f26'
  on-primary-fixed-variant: '#004e5c'
  secondary-fixed: '#e0e0ff'
  secondary-fixed-dim: '#bdc2ff'
  on-secondary-fixed: '#000767'
  on-secondary-fixed-variant: '#2f3aa3'
  tertiary-fixed: '#ffddb8'
  tertiary-fixed-dim: '#ffb95f'
  on-tertiary-fixed: '#2a1700'
  on-tertiary-fixed-variant: '#653e00'
  background: '#111317'
  on-background: '#e2e2e8'
  surface-variant: '#333539'
typography:
  headline-xl:
    fontFamily: EB Garamond
    fontSize: 32px
    fontWeight: '400'
    lineHeight: 40px
    letterSpacing: -0.01em
  headline-lg:
    fontFamily: EB Garamond
    fontSize: 24px
    fontWeight: '500'
    lineHeight: 32px
    letterSpacing: -0.005em
  headline-md:
    fontFamily: EB Garamond
    fontSize: 20px
    fontWeight: '500'
    lineHeight: 28px
  headline-sm:
    fontFamily: EB Garamond
    fontSize: 16px
    fontWeight: '600'
    lineHeight: 24px
  body-lg:
    fontFamily: EB Garamond
    fontSize: 18px
    fontWeight: '400'
    lineHeight: 28px
  body-md:
    fontFamily: Geist
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 22px
  body-sm:
    fontFamily: Geist
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 18px
  label-lg:
    fontFamily: Geist
    fontSize: 13px
    fontWeight: '500'
    lineHeight: 18px
    letterSpacing: 0.02em
  label-md:
    fontFamily: Geist
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: 0.04em
  label-sm:
    fontFamily: Geist
    fontSize: 10px
    fontWeight: '600'
    lineHeight: 14px
    letterSpacing: 0.06em
  code-sm:
    fontFamily: Geist
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 16px
spacing:
  gutter: 1rem
  gutter-sm: 0.5rem
  gutter-lg: 1.5rem
  margin: 1rem
  margin-panel: 0.75rem
  space-2xs: 0.125rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 0.75rem
  space-lg: 1rem
  space-xl: 1.5rem
  space-2xl: 2rem
---

## Brand & Style
This design system pairs the classical typographic grace of historical typesetting with the surgical rigor of an IDE. Created for narrative directors, game writers, and interactive fiction architects, the aesthetic avoids playful consumer SaaS tropes in favor of an austere, monkish workstation.

The design philosophy reconciles two opposing modalities: the lyrical prose of long-form narrative craft and the discrete, deterministic logic of node-based state machines. Surfaces are obsidian-dark, structured through hairline grid boundaries, zero-radius geometry, and precise typographic cadence. The interface remains quiet and low-chroma during focused composition, allowing color to function exclusively as semantic logic indicators (branching choices, speaker roles, logic switches, variable telemetry).

## Colors
The palette balances ultra-dark slate foundations with calibrated semantic accents that categorize narrative topology at a glance:

- **Base Canvas & Surfaces**:
  - Root Canvas: `#0f1115` (Deep warm obsidian)
  - Surface Container Low: `#141720` (Node canvas floor, docked panel wells)
  - Surface Container: `#181c26` (Node bodies, inspector panels, dialog modals)
  - Surface Container High: `#212735` (Header bars, active row states, tab strips)
  - Surface Border: `#283042` (1px structural rule lines, muted dividing axes)

- **Semantic Narrative Accents**:
  - **Cyan (`#06b6d4`)**: Primary system anchor; teleport jumps, memory flags, variable pointers, focus rings.
  - **Iris / Indigo (`#818cf8`)**: Secondary anchor; dialogue passages, speaker roles, voice manifests.
  - **Amber / Gold (`#f59e0b`)**: Tertiary anchor; branch forks, player choice nodes, prompt selections.
  - **Emerald (`#10b981`)**: Logic gates, boolean conditions, successful compilation pass states.
  - **Rose / Coral (`#f43f5e`)**: Terminal narrative endpoints, orphaned node warnings, syntax errors.
  - **Violet (`#a855f7`)**: Codex entities, lore taxonomies, world state definitions.

Text uses strict perceptual steps: `#f1f3f9` for high-contrast literary reading, `#9ba3b8` for property keys and secondary descriptors, and `#64748b` for inactive metadata, hotkeys, and connector port labels.

## Typography
Typography is treated as an editorial artifact inside a mechanical housing. 

- **EB Garamond** powers narrative prose inputs, node beat titles, scene act declarations, and long-form story dialogues. Its classical Roman proportion offers comfort during extended writing sessions and visually isolates story content from machine instructions.
- **Geist** serves as the system engine for the canvas shell, data tables, node headers, inspector fields, and tree navigators. Crisp, neutral, and monolinear, it delivers high legibility at 10px to 13px without geometric aberration.
- Monospaced needs (variables, mathematical operators, raw branch expressions) utilize `Geist` with tabular numbers enabled (`tnum`, `zero`).

## Layout & Spacing
The layout adheres to a multi-pane IDE viewport model designed for high-density spatial multitasking:

1. **Top Shell Bar**: Fixed 40px height housing breadcrumbs, global branch selectors, and compilation status.
2. **Left Dock (Navigator & Codex)**: Collapsible sidebar fixed at 280px width, utilizing `space-xs` and `space-sm` for high vertical density.
3. **Infinite Graph Canvas**: Fluid center plane patterned with a subtle 24px dot-matrix grid (`#1d2332` on `#0f1115`).
4. **Right Dock (Inspector & Logic Evaluator)**: Fixed 360px panel with vertical sections spaced by hairline borders and internal `space-md` paddings.

All internal gaps follow an absolute 4px baseline rhythm (`space-xs` = 4px, `space-sm` = 8px, `space-md` = 12px, `space-lg` = 16px).

## Elevation & Depth
Elevation eschews blurry drop shadows in favor of crisp, layered structural boundaries and localized luminescence:

- **Level 0 (Canvas Base)**: `#0f1115` background with an etched dot grid.
- **Level 1 (Docked Sidebars & Rails)**: `#141720` with a 1px solid border (`#283042`) separating canvas from docked utilities.
- **Level 2 (Nodes & Graph Cards)**: `#181c26` solid fill, framed by a 1px border of `#283042`. When selected or focused, the border transitions to `#06b6d4` with an ultra-subtle, non-diffuse 0 0 0 1px outer ring.
- **Level 3 (Search Palettes & Port Menus)**: `#212735` layered directly over the canvas with a 1px border (`#3a465f`) and zero ambient blur.
- **Execution & Flow Glow**: Active running logic paths apply an intentional, sharp 1px hairline stroke in the semantic accent color without fuzzy illumination.

## Shapes
Every component in the design system has a strict `0px` border radius (`roundedness: 0`). 

Rectilinear purity reinforces precision, maximizes dense screen estate, and emphasizes the structural nature of narrative architecture. Cards, input fields, ports, badges, context flyouts, and modal windows terminate in sharp 90-degree right angles.

## Components

### Node Graph Cards
- **Structure**: Rectangular cards constructed on `#181c26` with a mandatory 1px border (`#283042`).
- **Header**: Height 28px, background `#212735`, displaying an uppercase `label-sm` semantic indicator on the left, beat title in `headline-sm`, and run status dot on the right.
- **Accent Strip**: Top border features a 2px horizontal rule colored by node type (e.g., Amber for Choice, Iris for Dialogue).
- **Body**: Embedded prose area set in `body-md` (EB Garamond) with zero inner borders.
- **Connector Ports**: 8x8px square tabs positioned on card left (inputs) and right (outputs) edges, colored `#64748b`, turning solid Cyan `#06b6d4` on connection hover.

### Collapsible Inspector
- **Accordion Sections**: Headers feature an uppercase `label-md` accompanied by an inline toggle icon.
- **Property Rows**: 28px height, two-column split with label (`#9ba3b8`, 40% width) and input widget (60% width).
- **Dividers**: Flat 1px border (`#283042`) separating semantic sections.

### Buttons
- **Primary**: Solid `#06b6d4` fill, `#0f1115` bold text, sharp edges, 28px height for compact toolbar usage.
- **Secondary / Ghost**: Transparent fill, 1px `#283042` border, `#f1f3f9` text; hover transitions border to `#06b6d4`.
- **Destructive**: 1px `#f43f5e` outline with transparent fill, text colored `#f43f5e`.

### Input Fields & Monospace Editors
- **Text Inputs**: Flat background `#141720`, 1px border `#283042`, zero radius. Focus yields 1px `#06b6d4` perimeter.
- **Prose Area**: EB Garamond font, minimal background tint, auto-expanding height.
- **Logic Condition Editor**: Geist mono font, line-numbered left gutter, colored tokens for boolean expressions and variable calls.

### Tag & Badges
- **Tag Pills**: Flat rectangles (0px radius), height 20px, uppercase `label-sm`.
- **Badges by Category**:
  - `CHOICE`: Background `rgba(245, 158, 11, 0.12)`, text `#f59e0b`, border `1px solid #f59e0b`.
  - `SPEAKER`: Background `rgba(129, 140, 248, 0.12)`, text `#818cf8`, border `1px solid #818cf8`.
  - `LOGIC`: Background `rgba(16, 185, 129, 0.12)`, text `#10b981`, border `1px solid #10b981`.
  - `ERROR`: Background `rgba(244, 63, 94, 0.12)`, text `#f43f5e`, border `1px solid #f43f5e`.

### Search & Command Palette
- **Trigger Overlay**: Fullscreen backdrop with slight tint (`rgba(15, 17, 21, 0.8)`).
- **Palette Window**: 560px width, sharp edges, top input rendered with `headline-sm`, scrollable autocomplete list with selected row highlighted in `#212735` and keyed with a Cyan left border indicator.