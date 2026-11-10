---
name: Erzählstudio
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
**Erzählstudio** ("narration studio"). The mark is an ink tile holding an "E": its spine and outer arms in paper white, its middle arm a gilt thread that forks into two outcomes, one filled and one open, a told story at its moment of choice. Source: `src/web/assets/erzaehlstudio-logo.svg` (favicon); the toolbar uses `BrandMark.vue`, the same geometry painted with theme tokens.

Inspired by the spatial, atmospheric aesthetic of modern creative worldbuilding studios (such as vvd.world). Designed for narrative designers and game writers, this aesthetic combines a serene, low-distraction dark backdrop with tactile floating glass panels, rounded geometry, and precise typographic hierarchy.

Surfaces float above an atmospheric canvas with subtle gradient depth. Floating panels feature soft rounded corners (12-16px), fine hairline borders with subtle rim luminescence, and backdrop blur. High-frequency interactions remain snappy and grounded, with color serving primarily as semantic logic indicators.

## Colors & Surfaces
The palette balances an ultra-dark cosmic base with translucent slate containers and purposeful semantic narrative accents:

- **Atmospheric Canvas & Backdrops**:
  - Root Canvas: `#0b0d13` with subtle radial glows of indigo (`rgba(79, 70, 229, 0.08)`) and teal (`rgba(6, 182, 212, 0.05)`)
  - Floating Pane Surface: `rgba(18, 22, 34, 0.85)` with `backdrop-filter: blur(16px)`
  - Panel Header / Card Well: `rgba(26, 32, 48, 0.7)`
  - Elevated Popover / Menu: `rgba(28, 35, 54, 0.95)` with `box-shadow: 0 12px 36px rgba(0, 0, 0, 0.5)`
  - Glass Border Rule: `rgba(255, 255, 255, 0.08)` (hairline rim light)
  - Focused Border: `#06b6d4` with `0 0 0 1px #06b6d4`

- **Semantic Narrative Accents**:
  - **Cyan (`#06b6d4` / `#38bdf8`)**: Primary system anchor; action triggers, selections, teleport jumps.
  - **Iris / Indigo (`#818cf8`)**: Secondary anchor; dialogue passages, speaker roles, entity types.
  - **Amber / Gold (`#f59e0b`)**: Choices, player forks, interactive states.
  - **Emerald (`#10b981`)**: Conditions passed, active states, locations.
  - **Rose / Coral (`#f43f5e`)**: Danger, delete, terminal failure states.

Text maintains high legibility: `#f1f3f9` for primary reading, `#9ba3b8` for secondary descriptions, and `#64748b` for subtle metadata.

## Typography
- **Sora** (self-hosted variable font): System UI, sidebar navigators, headings, table cells, form labels and control buttons.
- **EB Garamond**: Narrative prose fields, beat titles, scene headings, and story card excerpts.
- **Geist Mono**: Monospace identifiers and numeric readouts.
- **Long text**: names that identify something wrap (two lines on tiles); secondary metadata and ids truncate with an ellipsis and a `title` tooltip; unbroken paths, URLs and emails use `overflow-wrap: anywhere`; numbers are never truncated, and counts use `countLabel` for the right noun form and digit grouping. `bun run test:visual:stress` checks this with worst-case data.

## Themes
The toolbar Appearance panel swaps whole-app themes; each is one token set in `src/web/studio/theme.ts`, applied as CSS variables on `<html>`:
- **Tokens a theme owns**: app backdrop and glow, card/input/popover/pane surfaces, three border strengths, three text levels, accent (fill, hover, ink, subtle), a secondary brand color, a radius scale (sharp, crisp, soft, round), and the heading voice (display font, weight, tracking, case).
- **Semantic colors stay fixed** across themes (status, node types, dialog iris) so logic signals keep their meaning.
- **Presets**: Kojima Productions (default), Mass Effect, Cyberpunk, RPG, Death Stranding, Anime, Modern Warfare. Each must keep main text ≥ 7:1, muted text ≥ 4.5:1 and dim text ≥ 5:1 on cards, and accent ink ≥ 4.5:1 on the accent (`tests/web/theme.test.ts`).
- **Background image**: optional, downscaled to 1920px and stored locally; dimmed by the theme backdrop, optionally blurred, and panes turn translucent over it.
- **Popovers are opaque**: menus, pickers and the graph toolbar use the solid `--bg-elevated` surface so content underneath never shows through.

## Layout & Workspace
- **Top Shell Bar**: Sleek 44px glass strip with brand identity, breadcrumbs/tabs, play action, and workspace status.
- **Floating Document Navigator**: Translucent collapsible floating panel on the left with rounded corners and search.
- **Split-Pane Workspace**: Multi-pane subdivision system allowing any entity, storylet, or browse view to be opened side-by-side or split horizontally/vertically via drag-and-drop or pane controls. A pane's title bar is its drag handle (never selectable text): drop it on another pane's edge to move it there, or on its middle to swap the two.
- **Record editors**: Entity, Trait, and Asset use the same centered 960px
  surface, padded identity card, underline section navigation, and spaced
  bordered content cards. In narrow panes the padding contracts and controls
  wrap or scroll rather than clipping.
- **Floating Controls**: Pill-style toolbars with grouped actions, 14px padding, and 44px touch targets.

## Record thumbnails and selection

Thumbnails are part of a record's visual identity. When an entity, entity template, trait, asset, or storylet has a thumbnail, carry it through to every UI representation of that record, including picker results, selected references, cards, lists, tree items, graph nodes, and compact badges. Resolve stored references with `thumbnailUrl` and keep the name visible; a thumbnail does not replace the accessible text label. When there is no thumbnail, use the surface's existing initial or icon fallback. Preserve the record's own thumbnail rather than substituting an unrelated image.

Native `<select>` options cannot reliably display thumbnails. When choosing records with thumbnails, provide an accessible visual picker for the options, and show the selected record's thumbnail next to the control. Preserve keyboard operation, search where appropriate, and a clear empty selection. In split workspaces, center the picker over the pane that opened it, constrain it to the visible pane, and keep it usable in narrow panes.

### Thumbnail review checklist

- Trace thumbnail data from each record through view models and picker item mappings to the rendered image; do not assume a shared picker receives it.
- Check each record type with and without an image, including list and grid results, selected values, arrays, cards, references, navigation, graph nodes, and compact representations. Check long names and missing or unavailable images.
- Inspect selection and display at desktop, narrow, and split-pane sizes. Open the picker from more than one pane and verify its position, sizing, scrolling, focus, and closing behavior.
- Use fixtures with actual thumbnails in visual checks. A fixture containing only records without images cannot prove that thumbnails render.
- Check native selectors and graph labels explicitly for missing images. Existing native menus and some graph labels still need visual thumbnail support; treat these as known gaps rather than evidence that the contract is already met everywhere.
