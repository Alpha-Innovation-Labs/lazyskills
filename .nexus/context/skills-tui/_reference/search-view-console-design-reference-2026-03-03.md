# Search View Console Design Reference (2026-03-03)

## Objective

Capture an implementation-ready visual direction for the `Search` tab inspired by dense terminal package-manager UIs, with clear information hierarchy and keyboard-first ergonomics.

## Design Intent

- Maximize scannability for large result sets.
- Keep most-used decision fields in stable columns.
- Promote selected item context in a dedicated details rail.
- Keep actions/status visible without obscuring list flow.

## Layout Blueprint

```text
+------------------------------------------------------------------------------------------------------+
| Search (skills.sh)                                              Total: N | Filtered: M             |
| Query: <text>   Scope: All   Sort: Downloads                                                      |
+------------------------------------------------------------------------------------------------------+
| Type | Name | Version | Description | Downloads | Fav                                                |
+------------------------------------------------------------------------------------------------------+
| [G]  | ratkit | 0.2.12 | Rust TUI component library | 13692 | *                                  |
| [G]  | ...                                                                                          |
| [L]  | ...                                                                                          |
+---------------------------------------------------------------+--------------------------------------+
| Left result table                                             | Details                              |
|                                                               | - Type / Source / Skill              |
|                                                               | - Version / Status / Favorite        |
|                                                               | - Description                         |
|                                                               | - Analytics (rank/downloads)         |
+---------------------------------------------------------------+--------------------------------------+
| Output / Actions: i Install  u Update  r Remove  f Favorite  Enter Preview  / Query               |
+------------------------------------------------------------------------------------------------------+
```

## Color System

- Base background: `#0A0E14`
- Primary text: `#D9E1EA`
- Muted text: `#7A8694`
- Border/divider: `#2A3340`
- Accent cyan (selection/action): `#35C2FF`
- Success green: `#5FD38D`
- Warning amber: `#F6C177`
- Error red: `#F7768E`
- Favorite star yellow: `#FFD166`

## Color Rules

### Result Table

- Header labels: accent cyan.
- Selected row: foreground near-black with accent cyan background.
- Non-selected rows: primary text.
- Favorite marker `*`: yellow.
- Type badge:
  - `[G]` (remote/global source): cyan.
  - `[L]` (local/project): muted gray.
- Downloads column:
  - High percentile: green.
  - Mid percentile: primary text.
  - Low percentile: muted gray.

### Details Rail

- Section titles (Description/Analytics): amber.
- Labels: cyan.
- Values: primary text.
- Status values:
  - Installed: green.
  - Not installed: red.
  - Update available: amber.

### Output / Actions Strip

- Action hotkeys: cyan.
- Success feedback lines: green.
- Warning feedback lines: amber.
- Error feedback lines: red.
- Command/meta text: muted gray.

## Interaction Mapping

- `Up/Down`: move selected result row.
- `Tab`: cycle focus between table and details/actions interaction context.
- `f`: toggle favorite from search list selection.
- `i`: install selected result.
- `u`: update selected installed skill.
- `r` or `Delete`: remove selected installed skill.
- `Enter`: open preview if installed; otherwise trigger install path.

## Integration Notes for skills-tui

- Keep existing split-pane architecture.
- Keep current search data model and enrich rendering semantics first.
- Use stable column widths to prevent horizontal jitter while navigating.
- Prefer concise, non-wrapping table rows; move long text to details panel.

## Acceptance Criteria

- List remains readable with 1000+ items.
- Selection state is always obvious.
- Favorite status is visible in every row.
- Installation status is visible in details panel with semantic color.
- Action bar provides immediate next-step discoverability.
