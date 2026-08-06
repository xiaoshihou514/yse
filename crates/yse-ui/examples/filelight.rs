//! A Filelight-inspired disk inspector with cancellable background scanning.

#[path = "support/example_style.rs"]
mod example_style;

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use yse_model::{Subscription, Task, Var, spawn_task};
use yse_ui::{Application, FileDialog, QtGuiScheduler, StringTableModel, Window, clone};

type ScanTask = Task<Result<Vec<Vec<String>>, String>>;

fn directory_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| {
            let path = entry.path();
            entry
                .metadata()
                .map(|metadata| {
                    if metadata.is_file() {
                        metadata.len()
                    } else {
                        0
                    }
                })
                .unwrap_or(0)
                + if path.is_dir() {
                    directory_size(&path)
                } else {
                    0
                }
        })
        .sum()
}

fn scan(path: String) -> Result<Vec<Vec<String>>, String> {
    let root = Path::new(&path);
    let mut entries: Vec<_> = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let bytes = directory_size(&entry.path());
            (name, bytes)
        })
        .collect();
    entries.sort_by_key(|(_, bytes)| std::cmp::Reverse(*bytes));
    let total: u64 = entries.iter().map(|(_, bytes)| bytes).sum();
    Ok(entries
        .into_iter()
        .take(24)
        .map(|(name, bytes)| {
            let percent = if total == 0 {
                0.0
            } else {
                bytes as f64 * 100.0 / total as f64
            };
            vec![
                name,
                format!("{:.1} MiB", bytes as f64 / 1_048_576.0),
                format!("{percent:5.1}%"),
            ]
        })
        .collect())
}

fn main() {
    let app = Application::init();
    example_style::apply(&app);
    let window = Window::new();
    window.set_title("Filelight — Yse edition");
    window.set_size(860, 580);
    let menubar = window.menu_bar();
    let scan_action = menubar.menu_with("Scan", |menu| {
        menu.action("Scan location")
            .icon("view-refresh")
            .shortcut("F5")
    });
    let open_action = menubar.menu_with("Location", |menu| {
        menu.action("Choose folder…").icon("folder-open")
    });
    let toolbar = window.toolbar();
    toolbar.add(&open_action);
    toolbar.add(&scan_action);
    let path = Var::new(String::from("."));
    let status = Var::new(String::from(
        "Choose a folder and scan its largest children.",
    ));
    let table = StringTableModel::new(
        3,
        [
            String::from("Folder"),
            String::from("Size"),
            String::from("Share"),
        ],
    );
    let task = Rc::new(RefCell::new(None::<ScanTask>));
    let subscriptions = Rc::new(RefCell::new(Vec::<Subscription>::new()));
    let (
        _title,
        path_edit,
        browse_button,
        scan_button,
        cancel_button,
        disk_map,
        table_view,
        status_label,
    ) = window.ui().column(|ui| {
        let title = ui.label("Disk usage");
        let helper = ui.label("Choose a folder to inspect its largest children.");
        helper.set_style_class("muted");
        let (path_edit, browse, scan, cancel) = ui.row(|ui| {
            (
                ui.line_edit(""),
                ui.button("Browse…"),
                ui.button("Scan"),
                ui.button("Cancel"),
            )
        });
        let disk_map = ui.disk_map();
        let table_view = ui.table_view(&table);
        let status_label = ui.label("");
        (
            title,
            path_edit,
            browse,
            scan,
            cancel,
            disk_map,
            table_view,
            status_label,
        )
    });
    browse_button.set_icon("folder-open");
    browse_button.set_style_class("quiet");
    scan_button.set_style_class("accent");
    cancel_button.set_style_class("quiet");
    status_label.set_style_class("muted");
    table_view.set_visible(false);
    path_edit.bind_text_two_way(&path);
    status_label.bind_text(&status.signal());
    scan_button.on_click(clone!(path, status, table, table_view, task, subscriptions, disk_map => move |_| {
        status.set(format!("Scanning {}…", path.value()));
        let work = spawn_task(Arc::new(QtGuiScheduler), { let path = path.value().to_string(); move |_| scan(path) });
        subscriptions.borrow_mut().push(work.results().observe(clone!(status, table, table_view, disk_map => move |result| match result {
            Ok(rows) => { let count = rows.len(); let labels = rows.iter().map(|row| row[0].clone()).collect::<Vec<_>>(); let shares = rows.iter().filter_map(|row| row[2].trim().trim_end_matches('%').parse::<f64>().ok()).collect::<Vec<_>>(); table.replace_all(rows.to_vec()); table_view.set_visible(true); disk_map.set_segments(labels, shares); status.set(format!("Scan complete · {count} top-level entries")); }
            Err(error) => status.set(format!("Scan failed: {error}")),
        })));
        *task.borrow_mut() = Some(work);
    }));
    scan_action.on_trigger(clone!(path, status, table, table_view, task, subscriptions, disk_map => move |_| {
        status.set(format!("Scanning {}…", path.value()));
        let work = spawn_task(Arc::new(QtGuiScheduler), { let path = path.value().to_string(); move |_| scan(path) });
        subscriptions.borrow_mut().push(work.results().observe(clone!(status, table, table_view, disk_map => move |result| match result {
            Ok(rows) => { let count = rows.len(); let labels = rows.iter().map(|row| row[0].clone()).collect::<Vec<_>>(); let shares = rows.iter().filter_map(|row| row[2].trim().trim_end_matches('%').parse::<f64>().ok()).collect::<Vec<_>>(); table.replace_all(rows.to_vec()); table_view.set_visible(true); disk_map.set_segments(labels, shares); status.set(format!("Scan complete · {count} top-level entries")); }
            Err(error) => status.set(format!("Scan failed: {error}")),
        })));
        *task.borrow_mut() = Some(work);
    }));
    open_action.on_trigger(clone!(window, path, status, subscriptions => move |_| {
        let dialog = FileDialog::directory(&window, "Choose a folder to scan");
        subscriptions.borrow_mut().push(dialog.result().observe(clone!(path, status => move |selected| {
            if let Some(folder) = selected {
                path.set(folder.clone());
                status.set(format!("{} selected — press Scan or F5.", folder));
            }
        })));
        dialog.show();
    }));
    browse_button.on_click(clone!(window, path, status, subscriptions => move |_| {
        let dialog = FileDialog::directory(&window, "Choose a folder to scan");
        subscriptions.borrow_mut().push(dialog.result().observe(clone!(path, status => move |selected| {
            if let Some(folder) = selected {
                path.set(folder.clone());
                status.set(format!("{} selected — press Scan or F5.", folder));
            }
        })));
        dialog.show();
    }));
    cancel_button.on_click(clone!(task, status => move |_| {
        if let Some(task) = task.borrow().as_ref() { task.cancel(); }
        status.set(String::from("Scan cancelled"));
    }));
    if std::env::var("YSE_SMOKE").is_ok() {
        path.set(String::from("/tmp"));
        scan_button.click();
        app.quit_after(400);
    } else if std::env::var("YSE_SCREENSHOT").is_ok() {
        path.set(String::from("."));
        scan_button.click();
    }
    window.show();
    std::process::exit(app.exec());
}
