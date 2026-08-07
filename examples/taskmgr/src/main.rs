//! 跨平台系统任务管理器（Windows / Linux），基于 Yse 构建。
//!
//! 采用 Laminar 风格：所有状态放在 `Var` 中，UI 通过派生信号与绑定驱动，
//! 事件处理器只修改状态、不直接操作控件。系统数据来源按平台条件编译：
//! Linux 读取 `/proc` 与 `/sys`，Windows 使用 `windows-sys`。

mod sys;

use std::cell::RefCell;
use std::rc::Rc;
use yse::{
    Application, MessageBox, MessageBoxButtons, MessageBoxResult, StringTableModel, Subscription,
    Timer, Var, Window, clone,
};

const HISTORY: usize = 60;
const TITLES: [&str; 5] = ["CPU", "内存", "磁盘 0", "以太网", "GPU 0"];

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
        3 => a.mem_bytes.cmp(&b.mem_bytes),
        4 => a.disk_bytes_per_s.cmp(&b.disk_bytes_per_s),
        5 => a.net_bytes_per_s.cmp(&b.net_bytes_per_s),
        8 => a.power.cmp(&b.power),
        _ => a
            .cpu
            .partial_cmp(&b.cpu)
            .unwrap_or(std::cmp::Ordering::Equal),
    }
}

fn sorted_rows(data: &[sys::ProcessSample], sort: SortState) -> Vec<Vec<String>> {
    let mut rows: Vec<&sys::ProcessSample> = data.iter().collect();
    rows.sort_by(|a, b| {
        let ordering = compare_rows(a, b, sort.column);
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
    rows.iter()
        .map(|p| {
            vec![
                p.name.clone(),
                String::from("正在运行"),
                format!("{:.1}%", p.cpu),
                format_mb(p.mem_bytes),
                format_bytes_per_s(p.disk_bytes_per_s),
                format_bytes_per_s(p.net_bytes_per_s),
                String::from("—"),
                String::from("—"),
                p.power.label().to_string(),
            ]
        })
        .collect()
}

fn details_rows(stats: &sys::SystemStats) -> Vec<Vec<String>> {
    let mut rows: Vec<&sys::ProcessSample> = stats.processes.iter().collect();
    rows.sort_by_key(|p| p.pid);
    rows.iter()
        .map(|p| {
            vec![
                p.name.clone(),
                p.pid.to_string(),
                String::from("正在运行"),
                format!("{:.1}%", p.cpu),
                format_mb(p.mem_bytes),
            ]
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
    let details_rows_var = Var::new(Vec::<Vec<String>>::new());

    // --- 表格模型 ---
    let process_model = StringTableModel::new(
        9,
        vec![
            String::from("名称"),
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
        5,
        vec![
            String::from("名称"),
            String::from("PID"),
            String::from("状态"),
            String::from("CPU"),
            String::from("内存"),
        ],
    );

    // --- 菜单 ---
    let menubar = window.menu_bar();
    let (quit_action, refresh_action) = menubar.menu_with("文件", |m| {
        (m.action("退出").shortcut("Ctrl+Q"), m.action("立即刷新"))
    });
    let about_action = menubar.menu_with("选项", |m| m.action("关于"));
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
        _details_view,
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

        let history_page = tabs.add_tab("应用历史记录");
        history_page.label("该视图尚未实现");
        let startup_page = tabs.add_tab("启动");
        startup_page.label("该视图尚未实现");
        let users_page = tabs.add_tab("用户");
        users_page.label("该视图尚未实现");

        let details_page = tabs.add_tab("详细信息");
        let details_view = details_page.table_view(&details_model.clone());

        let services_page = tabs.add_tab("服务");
        services_page.label("该视图尚未实现");

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

    // --- 派生信号与绑定（不直接操作控件） ---
    let sorted = process_data
        .signal()
        .combine(&sort.signal(), |data, s| sorted_rows(data, *s));
    process_model.bind_rows(&sorted);
    details_model.bind_rows(&details_rows_var.signal());

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
    let confirm_kill: Rc<dyn Fn(u32)> = Rc::new(
        clone!(process_data, status_text, window, kill_subs => move |pid| {
            let name = process_data
                .value()
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let confirm = MessageBox::new(
                &window,
                "结束任务",
                format!("确定要结束“{name}”(PID {pid})吗？"),
                MessageBoxButtons::OkCancel,
            );
            let sub = confirm.result().observe(clone!(pid, name, status_text => move |result| {
                if *result == MessageBoxResult::Ok {
                    let ok = sys::kill_process(pid);
                    status_text.set(if ok {
                        format!("已结束 {name} (PID {pid})")
                    } else {
                        format!("无法结束 {name} (PID {pid})")
                    });
                }
            }));
            kill_subs.borrow_mut().push(sub);
            confirm.show();
        }),
    );
    end_task.on_click(clone!(selected_pid, confirm_kill => move |_| {
        if let Some(pid) = *selected_pid.value() {
            confirm_kill(pid);
        }
    }));
    // 右键菜单“结束任务”：与底部按钮走同一条确认流程。
    process_view.context_menu().observe(
        clone!(process_data, sort, selected_pid, confirm_kill => move |row| {
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
            if let Some(process) = ordered.get(*row) {
                selected_pid.set(Some(process.pid));
                confirm_kill(process.pid);
            }
        }),
    );

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
    let sampler = Rc::new(RefCell::new(sys::Sampler::new()));
    let refresh: Rc<dyn Fn()> = Rc::new(clone!(
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
        mem_label_text,
        sampler
        => move || {
            let stats = sampler.borrow_mut().sample();

            process_data.set(stats.processes.clone());
            details_rows_var.set(details_rows(&stats));

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

    refresh_action.on_trigger(clone!(refresh => move |_| refresh()));

    // 立即刷新一次，然后每秒刷新。
    refresh();
    struct Tick {
        refresh: Rc<dyn Fn()>,
        timer: Rc<yse::QtTimer>,
    }
    impl Tick {
        fn run(self: &Rc<Self>) {
            (self.refresh)();
            let this = self.clone();
            self.timer
                .schedule_after(std::time::Duration::from_secs(1), {
                    Box::new(move || this.run())
                });
        }
    }
    let tick = Rc::new(Tick {
        refresh,
        timer: Rc::new(yse::QtTimer),
    });
    tick.run();

    // 表格列宽（一次性布局配置）。
    process_view.set_column_width(0, 220);
    process_view.set_column_width(2, 80);
    process_view.set_column_width(3, 100);
    process_view.stretch_last_section(true);

    // Headless smoke run: drive sorting, selection, the details toggle, and
    // the performance tab, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
        app.quit_after(4500);
        app.after(
            700,
            clone!(process_view, toggle, tabs => move || {
                process_view.click_header(2); // sort by CPU
                process_view.click_header(2); // toggle direction
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
    assert!(!rows.is_empty(), "process table must be populated");
    std::process::exit(code);
}
