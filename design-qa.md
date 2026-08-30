# ForgeCAD Canvas-first Viewer Design QA

> **Status: reference-only historical document (2026-08-29).** 本文保留研究或审计发生时的事实，不再定义当前产品范围或任务顺序。可复用结论必须经过 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md`、ADR-0029 和当前真实证据重新验证后，才能进入穿越火线武器主线。

- source visual truth path: `/Users/liuchongjiang/Downloads/UI.png`
- implementation screenshot path: `/Users/liuchongjiang/Documents/武神/design-implementation.png`
- compact-panel screenshot path: `/Users/liuchongjiang/Documents/武神/design-compact-panel.png`
- command-menu screenshot path: `/Users/liuchongjiang/Documents/武神/design-command-menu.png`
- combined comparison path: `/Users/liuchongjiang/Documents/武神/design-comparison.png`
- current-source native screenshot path: `/Users/liuchongjiang/Documents/武神/design-tauri-current.png`
- viewport: 1672 × 941 CSS px, device scale factor 1
- source pixels: 1672 × 941 RGB PNG
- implementation pixels: 1672 × 941 RGB browser capture
- state: source is Runtime-populated modeling mode; implementation is the honest empty-Runtime modeling state. App chrome, layout, typography, tokens and empty-state behavior were compared; the source's model/reference imagery and populated Runtime facts were not treated as frontend assets or copied into the empty state.

## Full-view comparison evidence

The source and implementation were opened independently and together in `design-comparison.png`. Both use a 58px global toolbar, three centered workspace modes, 248px/320px side rails, a dominant central canvas, a compact viewport command surface, blue-only selection emphasis, and a collapsed bottom rail. The implementation preserves the current empty Runtime state instead of inventing a candidate, quality result or reference image.

## Focused comparison evidence

1. Header and copy: `ForgeCAD`, project switcher, `建模 / 对比 / 审查`, Codex connection, undo/redo, `确认版本`, and `导出` match the reference hierarchy. The implementation adds no unrelated above-the-fold copy.
2. Layout rhythm: measured implementation tracks are 248px / 1104px / 320px at 1672px width. The bottom drawer is 42px collapsed and 238px expanded. The canvas remains the dominant region.
3. Viewport toolbar: version, `1视图 / 2视图 / 4视图`, camera presets, shading modes, reference/grid/Gizmo toggles, and reset are separated into the same control families as the source.
4. Side panels: the left rail exposes `结构 / 参考图`; the right rail exposes `属性 / 几何 / 材质 / 检查`. Tabs, borders, icon weight, typography and selected-state blue were checked at native size.
5. Bottom drawer: versions, Codex tasks, quality issues and activity are available from a single collapsed rail rather than two permanently open panels.
6. Palette and assets: both use a cool blue-black canvas, restrained borders, white/gray typography, blue selection and amber warning. Phosphor icons are used for UI symbols; the Runtime GLB and reference bytes remain the only model/reference asset sources.
7. Responsive behavior: at 900 × 900 the side rails remain available as top-bar `结构 / 对象` drawers. Both drawers open over the workspace, preserve readable internal layout, report `aria-expanded`, and close through Esc or the backdrop instead of disappearing below the desktop breakpoint.
8. Recovery and gating: the disconnected state uses one amber recovery banner, a matching `连接问题` drawer, working retry/diagnostic actions, disabled model controls, and an honestly disabled Export action. Compare mode lists candidate, authorized-reference and fixed-render prerequisites rather than presenting an undifferentiated blank area.
9. Language: the primary workbench path is Simplified Chinese. Product names and technical acronyms such as Runtime, Codex, GLB, AOV and PBR are intentionally retained; raw machine statuses are not used as primary copy.
10. Reduced density: with no Runtime model, the workbench no longer shows inactive scene search/filter controls, inspector detail tabs, three-step cards, or a full row of disabled viewport tools. Those controls appear only when their underlying data is available; secondary view controls live under `视图选项`.
11. Keyboard-first without permanent clutter: `⌘K` opens a compact searchable command menu; Arrow keys and Enter execute commands. Direct shortcuts cover modes, structure search, object information and the bottom drawer, while all primary mouse paths remain available.

Focused crop comparison was not needed because the native-size full views kept toolbar, tab and rail labels readable. Exact model imagery was intentionally excluded from browser fidelity scoring because the browser capture cannot receive authenticated Tauri Runtime bytes. A separate current-source Tauri dev launch then verified the same shell with the real Runtime read model (8 parts, 1636 triangles) without changing the fidelity source.

## Comparison history

- Iteration 0 finding: the prior UI kept candidate metadata, a second camera toolbar, reference comparison, version cards and Codex activity permanently visible, reducing canvas priority. Severity: P1. Fix: consolidated the command bar, added the three workspace modes and moved history/activity into a collapsed drawer.
- Iteration 1 finding: the initial compare mode left a large undifferentiated empty region and hid split/overlay/flicker inside advanced details. Severity: P2. Fix: promoted comparison-mode controls and made the compare stage consume the remaining center column.
- Iteration 2 finding: the command bar touched the center-column edges, while the source used a bounded floating surface. Severity: P2. Fix: added 14px insets, an outer border, 8px radius and a restrained shadow.
- Iteration 3 finding: connection failures were spread across unrelated surfaces, Export looked actionable without a valid version, and compare mode did not explain its prerequisites. Severity: P1. Fix: unified the recovery state, added real retry/diagnostic actions, fail-closed Export gating, and prerequisite-aware comparison recovery.
- Iteration 4 finding: at 900px the first inspector drawer implementation inherited a row flex direction, squeezing its header and tabs into vertical text. Severity: P1. Fix: made the compact inspector a column flex container and repeated the screenshot/DOM check.
- Iteration 5 finding: desktop panel resizing was pointer-only. Severity: P2. Fix: replaced the resize affordances with accessible separators supporting Arrow keys, Home and End; an `ArrowRight` interaction changed the left rail from 248px to 256px.
- Iteration 6 finding: the first polished state still carried too much inactive information when Runtime was unavailable. Severity: P1. Fix: removed inactive search/filter/detail groups and the three-step empty-state cards, made model tools progressive, moved secondary view controls into a popover, and added a keyboard command menu rather than more permanent toolbar controls.
- Iteration 7 finding: narrow-window text needed explicit proof after reducing density. Severity: P1. Fix: repeated 900×900 and 720×800 browser captures; labels remained horizontal, clipped text used bounded containers, and the compact Object panel showed one centered recovery state without overlap.
- Post-fix evidence: `/Users/liuchongjiang/Documents/武神/design-implementation.png` and `/Users/liuchongjiang/Documents/武神/design-compact-panel.png`; no actionable P0/P1/P2 layout, recovery, localization or responsive mismatch remains in the tested empty Runtime state.

## Findings

- No actionable P0/P1/P2 frontend mismatch remains for the tested app-owned workbench chrome, empty Runtime recovery path, comparison prerequisites or 900px compact drawers.
- P3: the installed `/Users/liuchongjiang/Applications/ForgeCAD Runtime Dev.app` remains an older packaged cohort. The current source was verified in a Tauri dev WebView and captured as `design-tauri-current.png`; rebuilding and replacing the installed package remains a separate gate.
- Intentional state deviation: no model, reference thumbnails, version cards or green quality rows are fabricated when Runtime is unavailable.

## Primary interactions tested

- Modeling, compare and review mode switching.
- Structure/reference and all four inspector tabs.
- Version/task/issue/activity drawer open, switch and collapse.
- Retry/diagnostic recovery actions and fail-closed Export gating.
- Compare-mode prerequisite state with missing candidate/reference/render evidence.
- `⌘K` open, ArrowDown navigation and Enter execution in the command menu.
- Drawer persistence and bounded side-panel width state; keyboard separator changed 248px → 256px.
- 900px compact Structure/Object drawers, 720px text layout, and Esc close.
- Browser console errors: none.
- Current-source Tauri dev WebView: launched and visually verified against real Runtime read-model data.

## Implementation checklist

- [x] Canvas-first desktop shell
- [x] Mode-specific center content
- [x] Read-only Runtime boundaries preserved
- [x] Collapsed bottom drawer
- [x] Bounded, persisted UI-only panel sizing
- [x] Native-size browser and responsive checks
- [x] Current-source native Tauri dev check

final result: passed
