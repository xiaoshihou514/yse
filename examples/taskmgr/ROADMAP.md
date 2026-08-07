# Yse Task Manager — Roadmap & parity status

Adapted from `/tmp/TASKMGR_ROADMAP.md` for the Yse implementation. The Qt
specifics (QAbstractItemModel, delegates, QTreeView, performance counters)
map onto Yse's reactive model + C++ shim; the product goals, data-correctness
rules, and parity contract carry over unchanged. Progress is tracked per phase
below; the app doubles as a UX test of the Yse framework, so library gaps
found while implementing are fixed in `crates/yse-ui` and recorded here.

## Parity contract

Six classic sections are present: **进程 / 性能 / 服务 / 启动 / 用户 /
详细信息** (应用历史记录 was removed per product decision — no reliable
cross-platform data source), plus menus, context-menu actions, update-speed
and pause behavior, sorting/grouping, icons, resource columns, heat maps,
live graphs, and the process-management actions.

## Status matrix

| Phase | Scope | Status |
|---|---|---|
| 0 | Parity baseline, capability matrix, keyboard/menu inventory | In progress |
| 1 | Provider/snapshot separation, background sampling, update speed, settings, benchmarks | Partially done (worker-thread sampling via `spawn_interval`, High/Normal/Low/Paused, coalescing) |
| 2 | Theme tokens, responsive shell, shared table/graph widgets, DPI | Done (left icon+label rail + stacked pages, quiet data-forward theme, shared heat delegate) |
| 3 | Processes parity: grouping, friendly names, icons, columns, heat maps, actions | Done (expandable tree, friendly names, icons, heat maps, column chooser, end task/tree guarded by PID+start-time pairing, open location) |
| 4 | Performance parity: CPU/memory/disk/network/GPU detail pages | CPU (incl. per-core via PDH), memory, network, disk (Linux + Windows PDH) done; GPU explicitly unavailable |
| 5 | Details + Services | Details partial (PID/PPID/threads/CPU time/priority); Services implemented (systemd / `sc`) |
| 6 | Startup + Users | Startup implemented (XDG autostart / registry Run); Users implemented (who / query user) |
| 7 | App history | Removed per product decision (no reliable cross-platform source) |
| 8 | Menus/commands/keyboard parity | Partially done (update speed, pause, group toggle, submenus, checkable actions) |
| 9 | Accessibility, reliability, polish | Bounded soak + 28x sampling speedup; keyboard nav/focus via Qt defaults, Ctrl+Q/R shortcuts, no color-only info |
| 10 | Release hardening | Known-limitations matrix below; signing/installer pending |

## Done so far

- Processes, Performance (CPU/memory), Details sections.
- System light/dark following, Fusion + stylesheet theme, per-process icons.
- Table header sorting, row selection, alternating rows, column widths,
  right-click context menu (end task), background-thread sampling with
  High/Normal/Low/Paused update speeds and coalescing, process grouping by
  Apps/Background/System, friendly names, group-aware sorting, process-tree
  termination, open-file-location, extended Details columns (PPID, threads,
  CPU time, priority), composable popup context menus, and checkable/submenu
  menu actions, resource heat-map cells, Services (systemd/SCM), Startup
  (XDG/registry Run), and Users (session) sections.

## Next

- Expandable app groups (tree view) and column chooser.
- Heat-map resource cells.
- Performance detail pages for disk/network/GPU.
- GPU usage (vendor APIs), Windows per-disk counters (PDH), elevation
  helper, 24-hour soak on real machines, signing/installer.
- Keyboard parity, accessibility, 24-hour soak, release hardening.

## Data-correctness rules (from the original roadmap)

1. Never identify a process solely by PID after an async delay; pair PID with
   creation identity where possible.
2. Never sort on formatted strings.
3. Never derive a rate from a single sample.
4. Never display stale data as live without indicating paused state.
5. Never turn an unavailable counter into zero unless zero is actually known.
6. Never let one failing provider block unrelated pages.
7. Never make the GUI wait synchronously for a privileged operation.
8. Keep units consistent and centralized.
9. Keep raw values in models; formatting belongs to presentation.
10. Sampling frequency must not depend on how many views are open.

## Known limitations & parity deviations

Explicitly documented instead of fabricating values, per the roadmap rules:

| Capability | Linux | Windows |
|---|---|---|
| Process icons | PATH lookup when `/proc/<pid>/exe` is unreadable | Shell icons |
| Per-disk active time | `/proc/diskstats` deltas | PDH `% Disk Time` |
| GPU usage | Unavailable (no vendor API) | Unavailable (no NVML/ADL/DXGI) |
| Per-core CPU | `/proc/stat` deltas | PDH `Processor(<n>) % Processor Time` |
| App history | Removed (no reliable source) | Removed (requires Windows resource-usage records) |
| Users | `who` | `query user` (exit code 1 from non-console contexts; output still parsed) |
| Privilege elevation | Not isolated (demo) | Not isolated (demo) |
| 24-hour soak | Bounded soak in tests; run the sampler loop for full duration on real machines | Same |

## UI acceptance checklist

- Distinct focus/hover/selected/disabled states: Qt Fusion + stylesheet. Done.
- Works without a mouse: Qt keyboard navigation; Ctrl+Q quit, Ctrl+R refresh. Done.
- Long text overflow: table/tree cells ellipsize with tooltips. Done.
- No information by color alone: heat maps pair with text labels. Done.
- Units consistent and no layout jumps: formatting helpers centralized; refresh is coalesced. Done.
- Light/dark both intentional: palette + per-scheme stylesheets. Done.
- DPI: Qt Fusion scaling via device-pixel-ratio; explicit 150/200% audit pending on
  real displays.
- Inaccessible metrics show "不可用", not misleading zeroes. Done.
- Provider errors never modal-spam: sampling failures degrade to empty/unavailable states. Done.

+## Visual direction (working note)
+
+The UI follows "quiet chrome, data-forward" principles: no boxes around the
+table, tree, or charts; grouping comes from alignment, whitespace, and a
+single accent color. Hierarchy is typographic (primary metric → secondary →
+metadata → labels). Navigation is a left icon+label rail with a stacked
+content area, matching the original roadmap's responsive-shell guidance.
+
