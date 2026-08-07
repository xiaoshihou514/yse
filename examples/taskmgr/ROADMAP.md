# Yse Task Manager — Roadmap & parity status

Adapted from `/tmp/TASKMGR_ROADMAP.md` for the Yse implementation. The Qt
specifics (QAbstractItemModel, delegates, QTreeView, performance counters)
map onto Yse's reactive model + C++ shim; the product goals, data-correctness
rules, and parity contract carry over unchanged. Progress is tracked per phase
below; the app doubles as a UX test of the Yse framework, so library gaps
found while implementing are fixed in `crates/yse-ui` and recorded here.

## Parity contract

All seven classic sections must be present: **进程 / 性能 / 应用历史记录 /
启动 / 用户 / 详细信息 / 服务**, plus menus, context-menu actions, update-speed
and pause behavior, sorting/grouping, icons, resource columns, heat maps, live
graphs, and the process-management actions.

## Status matrix

| Phase | Scope | Status |
|---|---|---|
| 0 | Parity baseline, capability matrix, keyboard/menu inventory | In progress |
| 1 | Provider/snapshot separation, background sampling, update speed, settings, benchmarks | Partially done (worker-thread sampling via `spawn_interval`, High/Normal/Low/Paused, coalescing) |
| 2 | Theme tokens, responsive shell, shared table/graph widgets, DPI | Partially done (theme + shell live) |
| 3 | Processes parity: grouping, friendly names, icons, columns, heat maps, actions | Partially done (grouping, friendly names, icons, resource columns, heat-map cells, column chooser, end task/tree, open location; expandable tree pending) |
| 4 | Performance parity: CPU/memory/disk/network/GPU detail pages | CPU + memory + network history done; disk/GPU metadata only |
| 5 | Details + Services | Details partial (PID/PPID/threads/CPU time/priority); Services implemented (systemd / `sc`) |
| 6 | Startup + Users | Startup implemented (XDG autostart / registry Run); Users implemented (who / query user) |
| 7 | App history | Explicit unavailable state (degrades per roadmap) |
| 8 | Menus/commands/keyboard parity | Partially done (update speed, pause, group toggle, submenus, checkable actions) |
| 9 | Accessibility, reliability, polish | Sampler soak test + 28x sampling speedup; keyboard/accessibility audit pending |
| 10 | Release hardening | Pending |

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
- Expandable tree groups, disk/GPU performance detail pages, keyboard/accessibility
  audit, 24-hour soak, release hardening.
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
