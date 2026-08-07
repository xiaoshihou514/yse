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
    Subscription, Var, Window, clone,
};

const HISTORY: usize = 60;
const TITLES: [&str; 5] = ["CPU", "内存", "磁盘 0", "以太网", "GPU 0"];

const LIGHT_QSS: &str = r#"
QTabWidget::pane { border: 1px solid #d0d0d0; border-radius: 4px; }
QTabBar::tab { padding: 6px 14px; border: 1px solid transparent; border-top-left-radius: 4px; border-top-right-radius: 4px; background: transparent; }
QTabBar::tab:selected { background: #ffffff; border-color: #d0d0d0; }
QTabBar::tab:hover:!selected { background: #f0f0f0; }
QHeaderView::section { background: #f5f5f5; border: none; border-right: 1px solid #e0e0e0; border-bottom: 1px solid #d0d0d0; padding: 5px 8px; font-weight: 600; }
QTableView { gridline-color: #ececec; selection-background-color: #3daee9; selection-color: #ffffff; }
QPushButton { padding: 6px 14px; border: 1px solid #c8c8c8; border-radius: 4px; background: #fafafa; }
QPushButton:hover { background: #f0f0f0; }
QPushButton:pressed { background: #e4e4e4; }
QPushButton:disabled { color: #a0a0a0; }
QPushButton[yseClass="accent"] { background: #3daee9; color: #ffffff; border-color: #2e9bd6; }
QPushButton[yseClass="quiet"] { background: transparent; border-color: transparent; }
QLabel[yseClass="muted"] { color: #888888; }
"#;

const DARK_QSS: &str = r#"
QTabWidget::pane { border: 1px solid #4a4a4a; border-radius: 4px; }
QTabBar::tab { padding: 6px 14px; border: 1px solid transparent; border-top-left-radius: 4px; border-top-right-radius: 4px; background: transparent; color: #dcdcdc; }
QTabBar::tab:selected { background: #353535; border-color: #4a4a4a; }
QTabBar::tab:hover:!selected { background: #2f2f2f; }
QHeaderView::section { background: #3a3a3a; border: none; border-right: 1px solid #464646; border-bottom: 1px solid #4a4a4a; padding: 5px 8px; font-weight: 600; color: #dcdcdc; }
QTableView { gridline-color: #3a3a3a; selection-background-color: #2a82da; selection-color: #ffffff; }
QPushButton { padding: 6px 14px; border: 1px solid #4e4e4e; border-radius: 4px; background: #3f3f3f; color: #dcdcdc; }
QPushButton:hover { background: #4a4a4a; }
QPushButton:pressed { background: #555555; }
QPushButton:disabled { color: #808080; }
QPushButton[yseClass="accent"] { background: #2a82da; color: #ffffff; border-color: #2171b8; }
QPushButton[yseClass="quiet"] { background: transparent; border-color: transparent; }
QLabel[yseClass="muted"] { color: #9a9a9a; }
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
    /// Sort by group (应用/后台进程/系统进程) first, then by `column`.
    group_primary: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: 2, // CPU
            descending: true,
            group_primary: true,
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

fn sorted_table(data: &[sys::ProcessSample], sort: SortState) -> Vec<(Vec<String>, String)> {
    let mut rows: Vec<&sys::ProcessSample> = data.iter().collect();
    rows.sort_by(|a, b| {
        let ordering = if sort.group_primary && sort.column != 1 {
            a.group
                .cmp(&b.group)
                .then_with(|| compare_rows(a, b, sort.column))
        } else {
            compare_rows(a, b, sort.column)
        };
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    rows.iter()
        .map(|p| {
            (
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
                ],
                p.exe.clone(),
            )
        })
        .collect()
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
    let selected_pid = Var::new(None::<u32>);
    let details_open = Var::new(false);
    let resource = Var::new(0usize);
    let cpu_history = Var::new(Vec::<f64>::new());
    let mem_history = Var::new(Vec::<f64>::new());
    let per_core_history = Var::new(Vec::<Vec<f64>>::new());
    let metrics_text = Var::new(String::new());
    let status_text = Var::new(String::new());
    let stats_text = Var::new(String::new());
    let resource_texts = Var::new(vec![String::new(); 5]);
    let mem_percent = Var::new(0i32);
    let mem_label_text = Var::new(String::new());
    let details_rows_var = Var::new(Vec::<(Vec<String>, String)>::new());

    // --- 表格模型 ---
    let process_model = StringTableModel::new(
        10,
        vec![
            String::from("名称"),
            String::from("类型"),
            String::from("状态"),
            String::from("CPU"),
            String::from("内存"),
            String::from("磁盘"),
            String::from("网络"),
            String::from("GPU"),
            String::from("GPU 引擎"),
            String::from("电源使用情况"),
        ],
    );
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

    // --- 菜单 ---
    let menubar = window.menu_bar();
    let (quit_action, refresh_action) = menubar.menu_with("文件", |m| {
        (m.action("退出").shortcut("Ctrl+Q"), m.action("立即刷新"))
    });
    let about_action = menubar.menu_with("选项", |m| m.action("关于"));
    let (high_action, normal_action, low_action, pause_action, group_action) =
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
            let group = m.action("按类型分组");
            group.set_checkable(true);
            group.set_checked(true);
            (high, normal, low, pause, group)
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
        tabs,
        process_view,
        details_view,
        end_task,
        toggle,
        status_label,
        detail_stats_label,
        sidebar,
        perf,
    ) = ui.column(|column| {
        let tabs = column.tab_widget();

        let process_page = tabs.add_tab("进程");
        let (view, end_task, toggle, status, detail_stats) = process_page.column(|page| {
            let view = page.table_view(&process_model.clone());
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

        let perf_page = tabs.add_tab("性能");
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
                let chart = area.line_chart();
                let cores_chart = area.line_chart();
                let metrics = area.label("");
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
                    metrics,
                    resource_label,
                    memory_row.0,
                    memory_row.1,
                    stats,
                    resmon,
                )
            });
            (sidebar, perf)
        });

        let details_page = tabs.add_tab("详细信息");
        let details_view = details_page.table_view(&details_model.clone());

        (
            tabs,
            view,
            details_view,
            end_task,
            toggle,
            status,
            detail_stats,
            sidebar,
            perf,
        )
    });
    let (
        title_label,
        chart,
        cores_chart,
        metrics,
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
    for button in [&cpu_btn, &mem_btn, &disk_btn, &net_btn, &gpu_btn] {
        button.set_style_class("quiet");
    }
    status_label.set_style_class("muted");
    detail_stats_label.set_style_class("muted");
    perf_stats_label.set_style_class("muted");

    // --- 派生信号与绑定（不直接操作控件） ---
    let sorted = process_data
        .signal()
        .combine(&sort.signal(), |data, s| sorted_table(data, *s));
    process_model.bind_table(&sorted);
    details_model.bind_table(&details_rows_var.signal());

    status_label.bind_text(&status_text.signal());
    detail_stats_label.bind_text(&stats_text.signal());
    detail_stats_label.bind_visible(&details_open.signal());
    perf_stats_label.bind_text(&stats_text.signal());
    metrics.bind_text(&metrics_text.signal());
    title_label.bind_text(&resource.signal().map(|r| TITLES[*r].to_string()));
    resource_label.bind_text(
        &resource
            .signal()
            .combine(&resource_texts.signal(), |r, texts| texts[*r].clone()),
    );
    resource_label.bind_visible(&resource.signal().map(|r| *r >= 2));

    let chart_series = resource.signal().combine3(
        &cpu_history.signal(),
        &mem_history.signal(),
        |r, cpu, mem| {
            if *r == 0 {
                cpu.clone()
            } else if *r == 1 {
                mem.clone()
            } else {
                Vec::new()
            }
        },
    );
    chart.bind_series(&chart_series);
    cores_chart.bind_series_multi(&per_core_history.signal());
    cores_chart.bind_visible(&resource.signal().map(|r| *r == 0));
    metrics.bind_visible(&resource.signal().map(|r| *r == 0));
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

    process_view.on_selection(clone!(process_data, sort, selected_pid => move |rows| {
        let Some(&row) = rows.first() else { return };
        let data = process_data.value();
        let s = *sort.value();
        let mut ordered: Vec<&sys::ProcessSample> = data.iter().collect();
        ordered.sort_by(|a, b| {
            let ordering = compare_rows(a, b, s.column);
            if s.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
        if let Some(process) = ordered.get(row) {
            selected_pid.set(Some(process.pid));
        }
    }));

    toggle.on_click(clone!(details_open => move |_| {
        details_open.set(!*details_open.value());
    }));

    for (index, button) in [cpu_btn, mem_btn, disk_btn, net_btn, gpu_btn]
        .into_iter()
        .enumerate()
    {
        button.on_click(clone!(resource => move |_| resource.set(index)));
    }

    let kill_subs: Rc<RefCell<Vec<Subscription>>> = Rc::new(RefCell::new(Vec::new()));
    // 结束进程（或整棵进程树）并给出明确反馈。
    let confirm_kill: Rc<dyn Fn(u32, bool)> = Rc::new(
        clone!(process_data, status_text, window, kill_subs => move |pid, tree| {
            let name = process_data
                .value()
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let pids: Vec<u32> = if tree {
                let mut queue = vec![pid];
                let mut seen = std::collections::HashSet::from([pid]);
                while let Some(current) = queue.pop() {
                    for process in process_data.value().iter() {
                        if process.parent_pid == current && seen.insert(process.pid) {
                            queue.push(process.pid);
                        }
                    }
                }
                seen.into_iter().collect()
            } else {
                vec![pid]
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
                    for pid in &pids {
                        if sys::kill_process(*pid) {
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
        if let Some(pid) = *selected_pid.value() {
            confirm_kill(pid, false);
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
        process_data, sort, selected_pid, confirm_kill, open_location, window,
        context_menu_holder
    => move |row| {
        let data = process_data.value();
        let s = *sort.value();
        let mut ordered: Vec<&sys::ProcessSample> = data.iter().collect();
        ordered.sort_by(|a, b| {
            let ordering = if s.group_primary && s.column != 1 {
                a.group
                    .cmp(&b.group)
                    .then_with(|| compare_rows(a, b, s.column))
            } else {
                compare_rows(a, b, s.column)
            };
            if s.descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
        if let Some(process) = ordered.get(*row) {
            selected_pid.set(Some(process.pid));
            let pid = process.pid;
            let menu = window.menu("");
            let end = menu.action("结束任务");
            end.on_trigger(clone!(confirm_kill => move |_| confirm_kill(pid, false)));
            let tree = menu.action("结束进程树");
            tree.on_trigger(clone!(confirm_kill => move |_| confirm_kill(pid, true)));
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
    let apply: Rc<dyn Fn(&sys::SystemStats)> = Rc::new(clone!(
        process_data,
        details_rows_var,
        metrics_text,
        status_text,
        stats_text,
        resource_texts,
        cpu_history,
        mem_history,
        per_core_history,
        mem_percent,
        mem_label_text
        => move |stats: &sys::SystemStats| {
            process_data.set(stats.processes.clone());
            details_rows_var.set(details_rows(stats));

            let cpu = &stats.cpu;
            let virtualized = if cpu.virtualization { "已启用" } else { "未启用" };
            metrics_text.set(format!(
                "当前利用率：{:.0}%\n当前速度：{:.2} GHz\n基准速度：{:.2} GHz\n插槽：{}\n内核数：{}\n逻辑处理器：{}\n虚拟化：{}\nL1 缓存：{}\nL2 缓存：{}\nL3 缓存：{}",
                cpu.usage_pct,
                cpu.current_mhz / 1000.0,
                cpu.base_mhz / 1000.0,
                cpu.sockets,
                cpu.cores,
                cpu.logical,
                virtualized,
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
            for name in &stats.disk_names {
                disk_text.push_str(&format!("{name}  活动：—\n"));
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

            push_history(&cpu_history, cpu.usage_pct);
            let mem_pct = if stats.mem_total > 0 {
                stats.mem_used as f64 / stats.mem_total as f64 * 100.0
            } else {
                0.0
            };
            push_history(&mem_history, mem_pct);
            let mut cores = per_core_history.value().as_ref().clone();
            if cores.len() != stats.cpu.per_core_pct.len() {
                cores = vec![Vec::new(); stats.cpu.per_core_pct.len()];
            }
            for (index, pct) in stats.cpu.per_core_pct.iter().enumerate() {
                cores[index].push(*pct);
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
    group_action.on_trigger(clone!(sort => move |_| {
        let mut next = *sort.value();
        next.group_primary = !next.group_primary;
        sort.set(next);
    }));

    // 表格布局与外观（一次性配置）。
    process_view.select_rows(true);
    process_view.set_alternating_row_colors(true);
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
            clone!(process_view, toggle, tabs => move || {
                process_view.click_header(3); // sort by CPU
                process_view.click_header(3); // toggle direction
                process_view.select(0);
                toggle.click();
                tabs.set_current(1);
                tabs.set_current(0);
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
        "[diag] tabs={} tab_visible={} tab_size={}x{} \
         table_rows={} table_visible={} table_size={}x{} \
         details_rows={} icons={} first_entries={:?}",
        tabs.count(),
        tabs.is_visible(),
        tabs.width(),
        tabs.height(),
        process_model.row_count(),
        process_view.is_visible(),
        process_view.width(),
        process_view.height(),
        details_model.row_count(),
        process_model.row_icon_count(),
        (0..3)
            .map(|row| process_model.cell(row, 0))
            .collect::<Vec<_>>(),
    );
    println!(
        "[diag] exe_count={}",
        rows.iter().filter(|p| !p.exe.is_empty()).count(),
    );
    assert!(!rows.is_empty(), "process table must be populated");
    assert!(
        process_model.row_icon_count() > 0,
        "process list must show icons"
    );
    std::process::exit(code);
}
