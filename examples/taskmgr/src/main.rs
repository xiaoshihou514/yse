//! 跨平台系统任务管理器（Windows / Linux），基于 Yse 构建。
//!
//! 采用 Laminar 风格：所有状态放在 `Var` 中，UI 通过派生信号与绑定驱动，
//! 事件处理器只修改状态、不直接操作控件。系统数据来源按平台条件编译：
//! Linux 读取 `/proc` 与 `/sys`，Windows 使用 `windows-sys`。

mod sys;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use yse::{
    Application, MessageBox, MessageBoxButtons, MessageBoxResult, QtGuiScheduler, StringTableModel,
    Subscription, TreeModel, Var, Window, clone,
};

const HISTORY: usize = 60;
const TITLES: [&str; 5] = ["CPU", "内存", "磁盘 0", "以太网", "GPU 0"];
const PROCESS_COLUMNS: [&str; 10] = [
    "名称",
    "类型",
    "状态",
    "CPU",
    "内存",
    "磁盘",
    "网络",
    "GPU",
    "GPU 引擎",
    "电源使用情况",
];

// Quiet chrome, data-forward theme: no boxes around the table, tree, or
// charts; hierarchy comes from typography and weight rather than borders.
// The same structure drives light and dark palettes.
const LIGHT_QSS: &str = r#"
* { font-family: "Segoe UI", "Noto Sans CJK SC", "Microsoft YaHei", sans-serif; font-size: 13px; }
QMainWindow { background: #f5f6f8; }
QHeaderView::section { background: transparent; border: none; border-bottom: 1px solid rgba(31, 35, 40, 0.08); padding: 6px 10px; font-weight: 500; color: #6b7280; }
QTableView, QTreeView { background: transparent; border: none; gridline-color: transparent; selection-background-color: rgba(0, 120, 212, 0.10); selection-color: #1f2328; show-decoration-selected: 1; outline: none; }
QTableView::item:hover, QTreeView::item:hover { background: rgba(0, 120, 212, 0.06); }
QTreeView::branch { background: transparent; }
QPushButton { padding: 6px 14px; border: none; border-radius: 6px; background: transparent; color: #1f2328; }
QPushButton:hover { background: rgba(31, 35, 40, 0.06); }
QPushButton:pressed { background: rgba(31, 35, 40, 0.10); }
QPushButton:disabled { color: #a6adb5; }
QPushButton[yseClass="accent"] { background: #0078d4; color: #ffffff; }
QPushButton[yseClass="accent"]:hover { background: #0067b8; }
QPushButton[yseClass="quiet"], QPushButton[yseClass="nav"] { background: transparent; text-align: left; padding: 6px 10px; border-radius: 6px; }
QPushButton[yseClass="nav"]:hover { background: rgba(0, 120, 212, 0.06); }
QPushButton[yseClass="navSelected"] { background: rgba(0, 120, 212, 0.12); color: #0067b8; font-weight: 600; text-align: left; border-radius: 6px; padding: 6px 10px; }
QLabel { background: transparent; }
QLabel[yseClass="muted"] { color: #6b7280; }
QLabel[yseClass="title"] { font-size: 15px; font-weight: 600; color: #1f2328; }
QLabel[yseClass="primary"] { font-size: 34px; font-weight: 600; color: #1f2328; }
QLabel[yseClass="secondary"] { font-size: 15px; font-weight: 500; color: #1f2328; }
QLineEdit { border: 1px solid rgba(31, 35, 40, 0.15); border-radius: 6px; padding: 5px 8px; background: #ffffff; }
QLineEdit:focus { border-color: #0078d4; }
QProgressBar { border: none; border-radius: 4px; background: #eceef1; text-align: center; color: transparent; }
QProgressBar::chunk { border-radius: 4px; background: #0078d4; }
QScrollBar:vertical { background: transparent; width: 10px; margin: 0; }
QScrollBar::handle:vertical { background: #d7dbe0; border-radius: 5px; min-height: 30px; }
QScrollBar::handle:vertical:hover { background: #b6bdc7; }
QScrollBar::add-line, QScrollBar::sub-line { height: 0; width: 0; }
QMenu { background: #ffffff; border: 1px solid rgba(31, 35, 40, 0.12); border-radius: 6px; padding: 4px; }
QMenu::item { padding: 6px 22px 6px 12px; border-radius: 4px; }
QMenu::item:selected { background: rgba(0, 120, 212, 0.10); }
QMenu::separator { height: 1px; background: #e5e7eb; margin: 4px 8px; }
QToolTip { background: #ffffff; color: #1f2328; border: 1px solid rgba(31, 35, 40, 0.12); }
"#;

const DARK_QSS: &str = r#"
* { font-family: "Segoe UI", "Noto Sans CJK SC", "Microsoft YaHei", sans-serif; font-size: 13px; }
QMainWindow { background: #1e1f22; }
QHeaderView::section { background: transparent; border: none; border-bottom: 1px solid rgba(230, 230, 230, 0.08); padding: 6px 10px; font-weight: 500; color: #9d9d9d; }
QTableView, QTreeView { background: transparent; border: none; gridline-color: transparent; selection-background-color: rgba(77, 163, 255, 0.16); selection-color: #e6e6e6; show-decoration-selected: 1; outline: none; }
QTableView::item:hover, QTreeView::item:hover { background: rgba(77, 163, 255, 0.08); }
QTreeView::branch { background: transparent; }
QPushButton { padding: 6px 14px; border: none; border-radius: 6px; background: transparent; color: #e6e6e6; }
QPushButton:hover { background: rgba(230, 230, 230, 0.08); }
QPushButton:pressed { background: rgba(230, 230, 230, 0.14); }
QPushButton:disabled { color: #6f7076; }
QPushButton[yseClass="accent"] { background: #4da3ff; color: #101418; }
QPushButton[yseClass="accent"]:hover { background: #6cb2ff; }
QPushButton[yseClass="quiet"], QPushButton[yseClass="nav"] { background: transparent; text-align: left; padding: 6px 10px; border-radius: 6px; }
QPushButton[yseClass="nav"]:hover { background: rgba(77, 163, 255, 0.08); }
QPushButton[yseClass="navSelected"] { background: rgba(77, 163, 255, 0.16); color: #8fc1ff; font-weight: 600; text-align: left; border-radius: 6px; padding: 6px 10px; }
QLabel { background: transparent; }
QLabel[yseClass="muted"] { color: #9d9d9d; }
QLabel[yseClass="title"] { font-size: 15px; font-weight: 600; color: #e6e6e6; }
QLabel[yseClass="primary"] { font-size: 34px; font-weight: 600; color: #e6e6e6; }
QLabel[yseClass="secondary"] { font-size: 15px; font-weight: 500; color: #e6e6e6; }
QLineEdit { border: 1px solid rgba(230, 230, 230, 0.15); border-radius: 6px; padding: 5px 8px; background: #26272a; }
QLineEdit:focus { border-color: #4da3ff; }
QProgressBar { border: none; border-radius: 4px; background: #3a3c42; text-align: center; color: transparent; }
QProgressBar::chunk { border-radius: 4px; background: #4da3ff; }
QScrollBar:vertical { background: transparent; width: 10px; margin: 0; }
QScrollBar::handle:vertical { background: #45464b; border-radius: 5px; min-height: 30px; }
QScrollBar::handle:vertical:hover { background: #55565c; }
QScrollBar::add-line, QScrollBar::sub-line { height: 0; width: 0; }
QMenu { background: #26272a; border: 1px solid rgba(230, 230, 230, 0.12); border-radius: 6px; padding: 4px; }
QMenu::item { padding: 6px 22px 6px 12px; border-radius: 4px; }
QMenu::item:selected { background: rgba(77, 163, 255, 0.16); }
QMenu::separator { height: 1px; background: #3f3f46; margin: 4px 8px; }
QToolTip { background: #26272a; color: #e6e6e6; border: 1px solid rgba(230, 230, 230, 0.12); }
"#;

/// Follow the system color scheme and apply the matching stylesheet.
fn apply_theme(app: &Application) -> bool {
    let dark = app.follow_system_color_scheme();
    app.set_style_sheet(if dark { DARK_QSS } else { LIGHT_QSS });
    dark
}

/// Sortable columns of the process table.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SortState {
    column: usize,
    descending: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: 2, // CPU
            descending: true,
        }
    }
}

fn compare_rows(
    a: &sys::ProcessSample,
    b: &sys::ProcessSample,
    column: usize,
) -> std::cmp::Ordering {
    match column {
        0 => a.name.cmp(&b.name),
        1 => a.group.cmp(&b.group),
        3 => a.mem_bytes.cmp(&b.mem_bytes),
        4 => a.disk_bytes_per_s.cmp(&b.disk_bytes_per_s),
        5 => a.net_bytes_per_s.cmp(&b.net_bytes_per_s),
        9 => a.power.cmp(&b.power),
        _ => a
            .cpu
            .partial_cmp(&b.cpu)
            .unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// A small friendly-name map for well-known processes; the executable name is
/// the fallback.
fn friendly_name(name: &str) -> String {
    const FRIENDLY: &[(&str, &str)] = &[
        ("systemd", "系统服务管理器"),
        ("explorer", "Windows 资源管理器"),
        ("svchost", "Windows 服务主机"),
        ("firefox", "Firefox"),
        ("code", "Visual Studio Code"),
        ("Code", "Visual Studio Code"),
        ("chrome", "Chrome"),
        ("msedgewebview2", "Microsoft Edge WebView2"),
        ("yse-taskmgr", "任务管理器"),
        ("yse-taskmgr.exe", "任务管理器"),
    ];
    for (key, display) in FRIENDLY {
        if name.starts_with(key) {
            return (*display).to_string();
        }
    }
    name.to_string()
}

/// One row of the grouped process tree: group roots have `parent == -1` and
/// no pid; children carry their pid, icon path, and per-column heat values.
#[derive(Clone, PartialEq)]
struct ProcessTreeRow {
    parent: i32,
    cells: Vec<String>,
    icon: String,
    pid: Option<u32>,
    start_time: u64,
    heat: [f64; 4],
}

fn process_cells(p: &sys::ProcessSample) -> Vec<String> {
    vec![
        friendly_name(&p.name),
        p.group.label().to_string(),
        String::from("正在运行"),
        format!("{:.1}%", p.cpu),
        format_mb(p.mem_bytes),
        format_bytes_per_s(p.disk_bytes_per_s),
        format_bytes_per_s(p.net_bytes_per_s),
        String::from("—"),
        String::from("—"),
        p.power.label().to_string(),
    ]
}

fn build_tree(data: &[sys::ProcessSample], sort: SortState) -> Vec<ProcessTreeRow> {
    let mut rows = Vec::new();
    for group in [
        sys::ProcessGroup::App,
        sys::ProcessGroup::Background,
        sys::ProcessGroup::System,
    ] {
        let parent = rows.len() as i32;
        let mut cells = vec![String::new(); 10];
        cells[0] = group.label().to_string();
        cells[1] = group.label().to_string();
        rows.push(ProcessTreeRow {
            parent: -1,
            cells,
            icon: String::new(),
            pid: None,
            start_time: 0,
            heat: [0.0; 4],
        });
        let mut children: Vec<&sys::ProcessSample> =
            data.iter().filter(|p| p.group == group).collect();
        children.sort_by(|a, b| {
            let ordering = compare_rows(a, b, sort.column);
            if sort.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
        let mem_max = children
            .iter()
            .map(|p| p.mem_bytes)
            .max()
            .unwrap_or(1)
            .max(1);
        let disk_max = children
            .iter()
            .map(|p| p.disk_bytes_per_s)
            .max()
            .unwrap_or(1)
            .max(1);
        let net_max = children
            .iter()
            .map(|p| p.net_bytes_per_s)
            .max()
            .unwrap_or(1)
            .max(1);
        for p in children {
            rows.push(ProcessTreeRow {
                parent,
                cells: process_cells(p),
                icon: p.exe.clone(),
                pid: Some(p.pid),
                start_time: p.start_time,
                heat: [
                    (p.cpu / 100.0).clamp(0.0, 1.0),
                    (p.mem_bytes as f64 / mem_max as f64).clamp(0.0, 1.0),
                    (p.disk_bytes_per_s as f64 / disk_max as f64).clamp(0.0, 1.0),
                    (p.net_bytes_per_s as f64 / net_max as f64).clamp(0.0, 1.0),
                ],
            });
        }
    }
    rows
}

fn details_rows(stats: &sys::SystemStats) -> Vec<(Vec<String>, String)> {
    let mut rows: Vec<&sys::ProcessSample> = stats.processes.iter().collect();
    rows.sort_by_key(|p| p.pid);
    rows.iter()
        .map(|p| {
            (
                vec![
                    p.name.clone(),
                    p.pid.to_string(),
                    p.parent_pid.to_string(),
                    String::from("正在运行"),
                    format!("{:.1}%", p.cpu),
                    format_mb(p.mem_bytes),
                    p.threads.to_string(),
                    format!("{:.0} 秒", p.cpu_ticks as f64 / 100.0),
                    p.priority.clone(),
                ],
                p.exe.clone(),
            )
        })
        .collect()
}

fn format_mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

fn format_bytes_per_s(bytes_per_s: u64) -> String {
    if bytes_per_s >= 1_000_000 {
        format!("{:.1} MB/秒", bytes_per_s as f64 / 1_000_000.0)
    } else {
        format!("{:.0} KB/秒", bytes_per_s as f64 / 1_000.0)
    }
}

fn format_kb(kb: u64) -> String {
    if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{kb} KB")
    }
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;
    format!("{days:02}:{hours:02}:{minutes:02}:{seconds:02}")
}

fn push_history(history: &Var<Vec<f64>>, value: f64) {
    let mut values = history.value().as_ref().clone();
    values.push(value);
    if values.len() > HISTORY {
        values.remove(0);
    }
    history.set(values);
}

fn main() {
    let app = Application::init();
    let _initial_dark = apply_theme(&app);
    let window = Window::new();
    window.set_title("任务管理器");
    window.set_size(960, 620);

    // --- 状态：全部放在 Var 中，UI 通过绑定消费这些信号 ---
    let process_data = Var::new(Vec::<sys::ProcessSample>::new());
    let sort = Var::new(SortState::default());
    let visible_columns = Var::new((0..PROCESS_COLUMNS.len()).collect::<Vec<usize>>());
    let selected_pid = Var::new(None::<(u32, u64)>);
    let details_open = Var::new(false);
    let resource = Var::new(0usize);
    let cpu_history = Var::new(Vec::<f64>::new());
    let mem_history = Var::new(Vec::<f64>::new());
    let net_history = Var::new(Vec::<f64>::new());
    let disk_history = Var::new(Vec::<f64>::new());
    let per_core_history = Var::new(Vec::<Vec<f64>>::new());
    let usage_text = Var::new(String::new());
    let speed_text = Var::new(String::new());
    let detail_text = Var::new(String::new());
    let status_text = Var::new(String::new());
    let stats_text = Var::new(String::new());
    let resource_texts = Var::new(vec![String::new(); 5]);
    let mem_percent = Var::new(0i32);
    let mem_label_text = Var::new(String::new());
    let details_rows_var = Var::new(Vec::<(Vec<String>, String)>::new());
    let services_rows = Var::new(Vec::<Vec<String>>::new());
    let startup_rows = Var::new(Vec::<Vec<String>>::new());
    let users_rows = Var::new(Vec::<Vec<String>>::new());

    // --- 表格模型 ---
    let tree_model = TreeModel::new(PROCESS_COLUMNS.len());
    tree_model.set_headers(PROCESS_COLUMNS.iter().map(|h| (*h).to_string()));
    let details_model = StringTableModel::new(
        9,
        vec![
            String::from("名称"),
            String::from("PID"),
            String::from("PPID"),
            String::from("状态"),
            String::from("CPU"),
            String::from("内存"),
            String::from("线程数"),
            String::from("CPU 时间"),
            String::from("优先级"),
        ],
    );
    let services_model = StringTableModel::new(
        3,
        vec![
            String::from("名称"),
            String::from("状态"),
            String::from("描述"),
        ],
    );
    let startup_model = StringTableModel::new(
        3,
        vec![
            String::from("名称"),
            String::from("命令"),
            String::from("状态"),
        ],
    );
    let users_model = StringTableModel::new(
        3,
        vec![
            String::from("用户"),
            String::from("会话"),
            String::from("状态"),
        ],
    );

    // --- 菜单 ---
    let menubar = window.menu_bar();
    let (quit_action, refresh_action) = menubar.menu_with("文件", |m| {
        (
            m.action("退出").shortcut("Ctrl+Q"),
            m.action("立即刷新").shortcut("Ctrl+R"),
        )
    });
    let about_action = menubar.menu_with("选项", |m| m.action("关于"));
    let (high_action, normal_action, low_action, pause_action, column_actions) =
        menubar.menu_with("查看", |m| {
            let speed = m.menu("更新速度");
            let high = speed.action("高（500 毫秒）");
            let normal = speed.action("普通（1 秒）");
            let low = speed.action("低（4 秒）");
            let pause = speed.action("暂停");
            for action in [&high, &normal, &low, &pause] {
                action.set_checkable(true);
            }
            normal.set_checked(true);
            let columns = m.menu("列");
            let mut column_actions = Vec::new();
            for (index, label) in PROCESS_COLUMNS.iter().enumerate() {
                let action = columns.action(*label);
                if index > 0 {
                    // 名称列始终显示，其余列可切换。
                    action.set_checkable(true);
                    action.set_checked(true);
                }
                column_actions.push((index, action));
            }
            (high, normal, low, pause, column_actions)
        });
    let about_box = MessageBox::new(
        &window,
        "关于",
        "Yse 任务管理器\n跨平台系统监控示例（Windows / Linux）",
        MessageBoxButtons::Ok,
    );

    // --- 声明式控件树 ---
    let ui = window.ui();
    let (
        rail,
        stack,
        process_view,
        details_view,
        end_task,
        toggle,
        status_label,
        detail_stats_label,
        sidebar,
        perf,
    ) = ui.column(|column| {
        column.row(|row| {
            // 左侧导航栏：与经典任务管理器一致的图标+文字导轨。
            let rail = row.column(|side| {
                let mut buttons = Vec::new();
                for label in [
                    "应用", "性能", "服务", "启动", "用户", "详细信息", "应用历史记录",
                ] {
                    let button = side.button(label);
                    button.set_style_class("nav");
                    buttons.push(button);
                }
                side.spacer();
                buttons
            });
            rail.first().unwrap().set_style_class("navSelected");

            let stack = row.stacked_widget();

            let process_page = stack.add_page();
            let (view, end_task, toggle, status, detail_stats) = process_page.column(|page| {
                let view = page.tree_view(&tree_model.clone());
                let bottom = page.row(|bar| {
                    let toggle = bar.button("详细信息");
                    let status = bar.label("");
                    bar.spacer();
                    let end = bar.button("结束任务");
                    (toggle, status, end)
                });
                let stats = page.label("");
                (view, bottom.2, bottom.0, bottom.1, stats)
            });

            let perf_page = stack.add_page();
            let (sidebar, perf) = perf_page.row(|row| {
                let sidebar = row.column(|side| {
                    let cpu = side.button("CPU");
                    let memory = side.button("内存");
                    let disk = side.button("磁盘 0");
                    let net = side.button("以太网");
                    let gpu = side.button("GPU 0");
                    (cpu, memory, disk, net, gpu)
                });
                let perf = row.column(|area| {
                    let title = area.label("CPU");
                    title.set_style_class("title");
                    let chart = area.line_chart();
                    let cores_chart = area.line_chart();
                    let usage = area.label("");
                    usage.set_style_class("primary");
                    let speed = area.label("");
                    speed.set_style_class("secondary");
                    let detail = area.label("");
                    detail.set_style_class("muted");
                    let resource_label = area.label("");
                    let memory_row = area.row(|mr| {
                        let progress = mr.progress_bar(0);
                        let label = mr.label("");
                        (progress, label)
                    });
                    let stats = area.label("");
                    let resmon = area.button("打开资源监视器");
                    (
                        title,
                        chart,
                        cores_chart,
                        usage,
                        speed,
                        detail,
                        resource_label,
                        memory_row.0,
                        memory_row.1,
                        stats,
                        resmon,
                    )
                });
                (sidebar, perf)
            });

            let details_page = stack.add_page();
            let details_view = details_page.table_view(&details_model.clone());

            let services_page = stack.add_page();
            services_page.table_view(&services_model.clone());

            let startup_page = stack.add_page();
            startup_page.table_view(&startup_model.clone());

            let users_page = stack.add_page();
            users_page.table_view(&users_model.clone());

            let history_page = stack.add_page();
            history_page.label("该视图需要平台资源使用记录：Windows 上需启用“应用历史记录”数据源，Linux 上无对应数据。");

            (
                rail,
                stack,
                view,
                details_view,
                end_task,
                toggle,
                status,
                detail_stats,
                sidebar,
                perf,
            )
        })
    });
    let (
        title_label,
        chart,
        cores_chart,
        usage,
        speed,
        detail,
        resource_label,
        mem_progress,
        mem_label,
        perf_stats_label,
        resmon,
    ) = perf;
    let (cpu_btn, mem_btn, disk_btn, net_btn, gpu_btn) = sidebar;

    // 视觉润色：主操作按钮用强调色，侧边栏与次要按钮用扁平样式。
    end_task.set_style_class("accent");
    resmon.set_style_class("quiet");
    toggle.set_style_class("quiet");
    status_label.set_style_class("muted");
    detail_stats_label.set_style_class("muted");
    perf_stats_label.set_style_class("muted");

    // 左侧导航：切换页面并高亮当前项。
    for (index, button) in rail.iter().enumerate() {
        button.on_click(clone!(rail, stack => move |_| {
            stack.set_current(index);
            for (i, nav) in rail.iter().enumerate() {
                nav.set_style_class(if i == index { "navSelected" } else { "nav" });
            }
        }));
    }

    // --- 派生信号与绑定（不直接操作控件） ---
    details_model.bind_table(&details_rows_var.signal());
    services_model.bind_rows(&services_rows.signal());
    startup_model.bind_rows(&startup_rows.signal());
    users_model.bind_rows(&users_rows.signal());

    // 分组进程树：三组根行 + 组内按所选列排序的子进程。
    let tree = process_data
        .signal()
        .combine(&sort.signal(), |data, s| build_tree(data, *s));
    // 进程树每帧整树刷新会折叠组、跳回顶部；刷新前后保存并恢复展开、
    // 滚动与选中状态。
    let tree_rows_signal = tree.map(|rows| {
        rows.iter()
            .map(|r| (r.parent, r.cells.clone(), r.icon.clone()))
            .collect()
    });
    let _tree_sync = tree_rows_signal.observe(clone!(
        tree_model, process_view, tree, selected_pid => move |rows: &Vec<(i32, Vec<String>, String)>| {
            let expanded = process_view.expanded_rows();
            let top = process_view.top_row();
            tree_model.reset(rows.clone());
            process_view.expand_rows(&expanded);
            if let Some(top) = top {
                process_view.scroll_to_flat(top);
            }
            if let Some((pid, _)) = *selected_pid.value()
                && let Some(row) = tree.value().iter().position(|r| r.pid == Some(pid))
            {
                process_view.select(row);
            }
        }
    ));
    tree_model.bind_heat(
        3,
        &tree.map(|rows| rows.iter().map(|r| r.heat[0]).collect()),
    );
    tree_model.bind_heat(
        4,
        &tree.map(|rows| rows.iter().map(|r| r.heat[1]).collect()),
    );
    tree_model.bind_heat(
        5,
        &tree.map(|rows| rows.iter().map(|r| r.heat[2]).collect()),
    );
    tree_model.bind_heat(
        6,
        &tree.map(|rows| rows.iter().map(|r| r.heat[3]).collect()),
    );

    status_label.bind_text(&status_text.signal());
    detail_stats_label.bind_text(&stats_text.signal());
    detail_stats_label.bind_visible(&details_open.signal());
    perf_stats_label.bind_text(&stats_text.signal());
    usage.bind_text(&usage_text.signal());
    speed.bind_text(&speed_text.signal());
    detail.bind_text(&detail_text.signal());
    title_label.bind_text(&resource.signal().map(|r| TITLES[*r].to_string()));
    resource_label.bind_text(
        &resource
            .signal()
            .combine(&resource_texts.signal(), |r, texts| texts[*r].clone()),
    );
    resource_label.bind_visible(&resource.signal().map(|r| *r >= 2));

    let histories = cpu_history.signal().combine4(
        &mem_history.signal(),
        &net_history.signal(),
        &disk_history.signal(),
        |cpu, mem, net, disk| [cpu.clone(), mem.clone(), net.clone(), disk.clone()],
    );
    let chart_series = resource
        .signal()
        .combine(&histories, |r, histories| match *r {
            0 => histories[0].clone(),
            1 => histories[0].clone(),
            2 => histories[3].clone(),
            3 => histories[2].clone(),
            _ => Vec::new(),
        });
    chart.bind_series(&chart_series);
    cores_chart.bind_series_multi(&per_core_history.signal());
    cores_chart.bind_visible(&resource.signal().map(|r| *r == 0));
    usage.bind_visible(&resource.signal().map(|r| *r == 0));
    speed.bind_visible(&resource.signal().map(|r| *r == 0));
    detail.bind_visible(&resource.signal().map(|r| *r == 0));
    mem_progress.bind_visible(&resource.signal().map(|r| *r == 1));
    mem_progress.bind_value(&mem_percent.signal());
    mem_label.bind_visible(&resource.signal().map(|r| *r == 1));
    mem_label.bind_text(&mem_label_text.signal());

    end_task.bind_enabled(&selected_pid.signal().map(|pid| pid.is_some()));
    toggle.bind_text(&details_open.signal().map(|open| {
        if *open {
            String::from("简略信息")
        } else {
            String::from("详细信息")
        }
    }));

    // --- 事件处理器：只修改状态 ---
    process_view
        .header_clicked()
        .observe(clone!(sort => move |column| {
            let mut next = *sort.value();
            if next.column == *column {
                next.descending = !next.descending;
            } else {
                next.column = *column;
                next.descending = true;
            }
            sort.set(next);
        }));

    process_view.on_selection(clone!(tree, selected_pid => move |rows| {
        if let Some(&row) = rows.first()
            && let Some(entry) = tree.value().get(row)
            && let Some(pid) = entry.pid
        {
            selected_pid.set(Some((pid, entry.start_time)));
        }
    }));

    toggle.on_click(clone!(details_open => move |_| {
        details_open.set(!*details_open.value());
    }));

    let resource_buttons = [cpu_btn, mem_btn, disk_btn, net_btn, gpu_btn];
    for (index, button) in resource_buttons.iter().enumerate() {
        button.on_click(clone!(resource => move |_| resource.set(index)));
    }
    // 资源导航按钮同样用“选中”样式，随 resource 状态切换。
    let _resource_nav = resource
        .signal()
        .observe(clone!(resource_buttons => move |selected| {
            for (index, button) in resource_buttons.iter().enumerate() {
                button.set_style_class(if index == *selected { "navSelected" } else { "nav" });
            }
        }));

    let kill_subs: Rc<RefCell<Vec<Subscription>>> = Rc::new(RefCell::new(Vec::new()));
    // 结束进程（或整棵进程树）并给出明确反馈。
    let confirm_kill: Rc<dyn Fn(u32, u64, bool)> = Rc::new(
        clone!(process_data, status_text, window, kill_subs => move |pid, start_time, tree| {
            let name = process_data
                .value()
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let pids: Vec<(u32, u64)> = if tree {
                let mut queue = vec![(pid, start_time)];
                let mut seen = std::collections::HashSet::from([(pid, start_time)]);
                while let Some((current, _current_start)) = queue.pop() {
                    for process in process_data.value().iter() {
                        if process.parent_pid == current
                            && seen.insert((process.pid, process.start_time))
                        {
                            queue.push((process.pid, process.start_time));
                        }
                    }
                }
                seen.into_iter().collect()
            } else {
                vec![(pid, start_time)]
            };
            let confirm = MessageBox::new(
                &window,
                if tree { "结束进程树" } else { "结束任务" },
                if tree {
                    format!("确定要结束“{name}”(PID {pid})及其 {} 个子进程吗？", pids.len() - 1)
                } else {
                    format!("确定要结束“{name}”(PID {pid})吗？")
                },
                MessageBoxButtons::OkCancel,
            );
            let sub = confirm.result().observe(clone!(pids, name, status_text => move |result| {
                if *result == MessageBoxResult::Ok {
                    let mut failed = 0;
                    let mut ok = 0;
                    for (pid, start_time) in &pids {
                        if sys::kill_process(*pid, *start_time) {
                            ok += 1;
                        } else {
                            failed += 1;
                        }
                    }
                    status_text.set(if failed == 0 {
                        format!("已结束 {name}（{ok} 个进程）")
                    } else {
                        format!("无法结束 {name}：{ok} 成功，{failed} 失败")
                    });
                }
            }));
            kill_subs.borrow_mut().push(sub);
            confirm.show();
        }),
    );
    end_task.on_click(clone!(selected_pid, confirm_kill => move |_| {
        if let Some((pid, start_time)) = *selected_pid.value() {
            confirm_kill(pid, start_time, false);
        }
    }));

    // 打开进程可执行文件的位置。
    let open_location: Rc<dyn Fn(u32)> = Rc::new(clone!(process_data, status_text => move |pid| {
        let Some(exe) = process_data
            .value()
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| p.exe.clone())
            .filter(|exe| !exe.is_empty())
        else {
            status_text.set(String::from("无法定位该进程的可执行文件"));
            return;
        };
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .args(["/select,", &exe])
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(dir) = std::path::Path::new(&exe).parent() {
                let _ = std::process::Command::new("xdg-open")
                    .arg(dir)
                    .spawn();
            }
        }
    }));

    // 右键菜单：结束任务 / 结束进程树 / 打开文件位置。
    let context_menu_holder: Rc<RefCell<Option<yse::Menu>>> = Rc::new(RefCell::new(None));
    process_view.context_menu().observe(clone!(
        tree, selected_pid, confirm_kill, open_location, window,
        context_menu_holder
    => move |row| {
        if let Some(entry) = tree.value().get(*row)
            && let Some(pid) = entry.pid
        {
            let start_time = entry.start_time;
            selected_pid.set(Some((pid, start_time)));
            let menu = window.menu("");
            let end = menu.action("结束任务");
            end.on_trigger(clone!(confirm_kill => move |_| confirm_kill(pid, start_time, false)));
            let tree = menu.action("结束进程树");
            tree.on_trigger(clone!(confirm_kill => move |_| confirm_kill(pid, start_time, true)));
            menu.separator();
            let location = menu.action("打开文件位置");
            location.on_trigger(clone!(open_location => move |_| open_location(pid)));
            *context_menu_holder.borrow_mut() = Some(menu);
            context_menu_holder.borrow().as_ref().unwrap().popup();
        }
    }));

    resmon.on_click(clone!(window => move |_| {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("resmon.exe").spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let box_ = MessageBox::new(
                &window,
                "资源监视器",
                "Linux 上未提供资源监视器；可尝试 gnome-system-monitor。",
                MessageBoxButtons::Ok,
            );
            box_.show();
        }
    }));

    about_action.on_trigger(clone!(about_box => move |_| about_box.show()));
    quit_action.on_trigger(move |_| std::process::exit(0));

    // --- 刷新循环：采样并更新状态，UI 自动跟随 ---
    // 将一次采样结果应用到全部状态（在 GUI 线程运行）。
    let service_tick = Rc::new(RefCell::new(0u32));
    let apply: Rc<dyn Fn(&sys::SystemStats)> = Rc::new(clone!(
        process_data,
        details_rows_var,
        services_rows,
        startup_rows,
        users_rows,
        usage_text,
        speed_text,
        detail_text,
        status_text,
        stats_text,
        resource_texts,
        cpu_history,
        mem_history,
        net_history,
        disk_history,
        per_core_history,
        mem_percent,
        mem_label_text,
        service_tick
        => move |stats: &sys::SystemStats| {
            process_data.set(stats.processes.clone());
            details_rows_var.set(details_rows(stats));
            let tick = *service_tick.borrow();
            *service_tick.borrow_mut() = tick.wrapping_add(1);
            if tick.is_multiple_of(4) {
                services_rows.set(
                    sys::service_entries()
                        .into_iter()
                        .map(|(name, state, description)| vec![name, state, description])
                        .collect(),
                );
                startup_rows.set(
                    sys::startup_entries()
                        .into_iter()
                        .map(|(name, command, state)| vec![name, command, state])
                        .collect(),
                );
                users_rows.set(
                    sys::user_sessions()
                        .into_iter()
                        .map(|(user, session, state)| vec![user, session, state])
                        .collect(),
                );
            }

            let cpu = &stats.cpu;
            let virtualized = if cpu.virtualization { "已启用" } else { "未启用" };
            usage_text.set(format!("{:.0}%", cpu.usage_pct));
            speed_text.set(format!(
                "当前速度 {:.2} GHz    基准速度 {:.2} GHz",
                cpu.current_mhz / 1000.0,
                cpu.base_mhz / 1000.0,
            ));
            detail_text.set(format!(
                "插槽 {} · 内核 {} · 逻辑处理器 {} · 虚拟化 {virtualized} · L1 {} · L2 {} · L3 {}",
                cpu.sockets,
                cpu.cores,
                cpu.logical,
                format_kb(cpu.l1_kb),
                format_kb(cpu.l2_kb),
                format_kb(cpu.l3_kb),
            ));

            status_text.set(format!(
                "进程：{} 线程：{} 句柄：{} 运行时间：{}",
                stats.process_count,
                stats.thread_count,
                stats.handle_count,
                format_uptime(stats.uptime_secs),
            ));
            stats_text.set(format!(
                "进程：{} 线程：{} 句柄：{}\n正常运行时间：{}",
                stats.process_count,
                stats.thread_count,
                stats.handle_count,
                format_uptime(stats.uptime_secs),
            ));

            let mut disk_text = String::from("磁盘\n");
            if stats.disk_activity.is_empty() {
                for name in &stats.disk_names {
                    disk_text.push_str(&format!("{name}  活动：不可用\n"));
                }
            } else {
                for (name, active) in &stats.disk_activity {
                    disk_text.push_str(&format!("{name}  活动：{active:.0}%\n"));
                }
            }
            let mut net_text = String::from("网络\n");
            for net in &stats.nets {
                net_text.push_str(&format!(
                    "{}  发送：{}  接收：{}\n",
                    net.name,
                    format_bytes_per_s(net.tx_bps),
                    format_bytes_per_s(net.rx_bps),
                ));
            }
            let mem_text = if stats.mem_total > 0 {
                format!(
                    "内存\n总计：{:.1} GB  已用：{:.1} GB ({:.0}%)",
                    stats.mem_total as f64 / (1024.0 * 1024.0 * 1024.0),
                    stats.mem_used as f64 / (1024.0 * 1024.0 * 1024.0),
                    stats.mem_used as f64 / stats.mem_total as f64 * 100.0,
                )
            } else {
                String::from("内存\n—")
            };
            let gpu_text = format!("GPU 0\n{}\n使用率：—", stats.gpu_name);
            resource_texts.set(vec![
                String::new(),
                mem_text,
                disk_text,
                net_text,
                gpu_text,
            ]);

            push_history(&cpu_history, cpu.usage_pct.clamp(0.0, 100.0));
            let mem_pct = if stats.mem_total > 0 {
                stats.mem_used as f64 / stats.mem_total as f64 * 100.0
            } else {
                0.0
            };
            push_history(&mem_history, mem_pct.clamp(0.0, 100.0));
            let net_mbps = stats
                .nets
                .iter()
                .map(|net| net.rx_bps + net.tx_bps)
                .sum::<u64>() as f64
                / 1_000_000.0;
            push_history(&net_history, net_mbps);
            let disk_active = stats
                .disk_activity
                .iter()
                .map(|(_, active)| *active)
                .fold(0.0f64, f64::max);
            push_history(&disk_history, disk_active.clamp(0.0, 100.0));
            let mut cores = per_core_history.value().as_ref().clone();
            if cores.len() != stats.cpu.per_core_pct.len() {
                cores = vec![Vec::new(); stats.cpu.per_core_pct.len()];
            }
            for (index, pct) in stats.cpu.per_core_pct.iter().enumerate() {
                cores[index].push((*pct).clamp(0.0, 100.0));
                if cores[index].len() > HISTORY {
                    cores[index].remove(0);
                }
            }
            per_core_history.set(cores);
            mem_percent.set(mem_pct as i32);
            mem_label_text.set(format!("内存使用率：{mem_pct:.0}%"));
        }
    ));

    // 采样在后台线程进行（500ms 一次），GUI 按所选更新速度应用，且采样
    // 永不堆积（spawn_interval 自带合并）。
    let last_stats = Rc::new(RefCell::new(None::<sys::SystemStats>));
    let last_applied = Rc::new(RefCell::new(Instant::now()));
    let mode = Rc::new(RefCell::new((Duration::from_millis(1000), false)));

    // 启动时立即应用一次初始采样，避免空白窗口。
    let mut initial_sampler = sys::Sampler::new();
    let initial = initial_sampler.sample();
    apply(&initial);
    *last_stats.borrow_mut() = Some(initial);

    let interval = yse::spawn_interval(
        Arc::new(QtGuiScheduler),
        Duration::from_millis(500),
        move |_token| initial_sampler.sample(),
    );
    let _sample_sub = interval.results().observe(clone!(
        apply, mode, last_applied, last_stats, app => move |stats| {
            let (interval_ms, paused) = *mode.borrow();
            if paused {
                return;
            }
            if last_applied.borrow().elapsed() >= interval_ms {
                *last_applied.borrow_mut() = Instant::now();
                apply_theme(&app);
                apply(stats);
                *last_stats.borrow_mut() = Some((*stats).clone());
            }
        }
    ));

    refresh_action.on_trigger(clone!(apply, last_stats, app => move |_| {
        if let Some(stats) = last_stats.borrow().as_ref() {
            apply_theme(&app);
            apply(stats);
        }
    }));

    // 更新速度 / 暂停。
    let set_mode: Rc<dyn Fn(u64, bool)> = Rc::new(clone!(
        mode, high_action, normal_action, low_action, pause_action => move |ms, paused| {
            *mode.borrow_mut() = (Duration::from_millis(ms), paused);
            high_action.set_checked(ms == 500 && !paused);
            normal_action.set_checked(ms == 1000 && !paused);
            low_action.set_checked(ms == 4000 && !paused);
            pause_action.set_checked(paused);
        }
    ));
    high_action.on_trigger(clone!(set_mode => move |_| set_mode(500, false)));
    normal_action.on_trigger(clone!(set_mode => move |_| set_mode(1000, false)));
    low_action.on_trigger(clone!(set_mode => move |_| set_mode(4000, false)));
    pause_action.on_trigger(clone!(set_mode => move |_| set_mode(1000, true)));
    for (index, action) in &column_actions {
        let index = *index;
        let handle = action.clone();
        action.on_trigger(clone!(visible_columns => move |_| {
            let mut visible = visible_columns.value().as_ref().clone();
            let now_visible = if let Some(position) =
                visible.iter().position(|&column| column == index)
            {
                visible.remove(position);
                false
            } else {
                visible.push(index);
                visible.sort_unstable();
                true
            };
            visible_columns.set(visible);
            handle.set_checked(now_visible);
        }));
    }

    // 列选择器通过隐藏表头列生效，模型布局与热力列位置保持稳定。
    let _column_visibility = visible_columns.signal().observe(clone!(
        process_view => move |visible| {
            for column in 0..PROCESS_COLUMNS.len() {
                process_view.set_column_hidden(column, !visible.contains(&column));
            }
        }
    ));

    // 表格布局与外观（一次性配置）。
    process_view.select_rows(true);
    process_view.set_alternating_row_colors(true);
    process_view.enable_heat();
    details_view.select_rows(true);
    details_view.set_alternating_row_colors(true);
    process_view.set_column_width(0, 230);
    process_view.set_column_width(1, 90);
    process_view.set_column_width(3, 70);
    process_view.set_column_width(4, 110);
    process_view.set_column_width(5, 90);
    process_view.set_column_width(6, 90);
    process_view.set_column_width(7, 70);
    process_view.set_column_width(8, 90);
    process_view.set_column_width(9, 110);
    process_view.stretch_last_section(true);

    // Headless smoke run: drive sorting, selection, the details toggle, and
    // the performance tab, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
        app.quit_after(4500);
        app.after(
            700,
            clone!(process_view, toggle, stack => move || {
                process_view.click_header(3); // sort by CPU
                process_view.click_header(3); // toggle direction
                process_view.select(0);
                toggle.click();
                stack.set_current(1);
                stack.set_current(0);
            }),
        );
    }

    window.show();
    let code = app.exec();

    let rows = process_data.value();
    println!(
        "[taskmgr] processes={} cpu={:.1}% top={:?}",
        rows.len(),
        cpu_history.value().last().copied().unwrap_or(0.0),
        rows.first().map(|p| (p.name.clone(), p.cpu)),
    );
    println!(
        "[diag] pages={} stack_visible={} stack_size={}x{} \
         table_rows={} table_visible={} table_size={}x{} \
         details_rows={} services={} startup={} users={} icons={} first_entries={:?}",
        stack.count(),
        stack.is_visible(),
        stack.width(),
        stack.height(),
        tree_model.row_count(),
        process_view.is_visible(),
        process_view.width(),
        process_view.height(),
        details_model.row_count(),
        services_model.row_count(),
        startup_model.row_count(),
        users_model.row_count(),
        tree_model.row_count(),
        (0..3)
            .map(|row| {
                tree.value()
                    .get(row)
                    .map(|entry| entry.cells[0].clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>(),
    );
    println!(
        "[diag] exe_count={}",
        rows.iter().filter(|p| !p.exe.is_empty()).count(),
    );
    assert!(!rows.is_empty(), "process table must be populated");
    assert!(tree_model.row_count() > 3, "process tree must have groups");
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    //! Bounded soak: repeatedly sample the system the way the app does at
    //! run time, asserting the sampler stays healthy and keeps producing
    //! data. The roadmap's 24-hour soak runs this loop for the session
    //! duration on real machines.
    #[test]
    fn sampler_survives_repeated_ticking() {
        let mut sampler = crate::sys::Sampler::new();
        let started = std::time::Instant::now();
        for tick in 0..50 {
            let stats = sampler.sample();
            assert!(
                !stats.processes.is_empty(),
                "tick {tick}: process list must be populated"
            );
            assert!(
                stats.cpu.logical > 0,
                "tick {tick}: CPU topology must exist"
            );
            assert!(stats.mem_total > 0, "tick {tick}: memory must be readable");
        }
        eprintln!(
            "[soak] 50 samples in {:.1}s ({:.0} ms/sample)",
            started.elapsed().as_secs_f64(),
            started.elapsed().as_secs_f64() * 1000.0 / 50.0,
        );
    }
}
