//! A Filelight-inspired disk inspector with cancellable background scanning.

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use yse_model::{Subscription, Task, Var, spawn_task};
use yse_ui::{Application, QtGuiScheduler, StringTableModel, Window, clone};

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
    let window = Window::new();
    window.set_title("Filelight — Yse edition");
    window.set_size(760, 500);
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
    let (path_edit, scan_button, cancel_button, status_label) = window.ui().column(|ui| {
        ui.label("Filelight");
        ui.label("Largest folders are ordered like a sunburst legend; the table remains useful without a custom canvas.");
        let (path_edit, scan, cancel) = ui.row(|ui| (ui.line_edit(""), ui.button("Scan"), ui.button("Cancel")));
        ui.table_view(&table);
        let status_label = ui.label("");
        (path_edit, scan, cancel, status_label)
    });
    path_edit.bind_text_two_way(&path);
    status_label.bind_text(&status.signal());
    scan_button.on_click(clone!(path, status, table, task, subscriptions => move |_| {
        status.set(format!("Scanning {}…", path.value()));
        let work = spawn_task(Arc::new(QtGuiScheduler), { let path = path.value().to_string(); move |_| scan(path) });
        subscriptions.borrow_mut().push(work.results().observe(clone!(status, table => move |result| match result {
            Ok(rows) => { let count = rows.len(); table.replace_all(rows.to_vec()); status.set(format!("Scanned {count} top-level entries")); }
            Err(error) => status.set(format!("Scan failed: {error}")),
        })));
        *task.borrow_mut() = Some(work);
    }));
    cancel_button.on_click(clone!(task, status => move |_| {
        if let Some(task) = task.borrow().as_ref() { task.cancel(); }
        status.set(String::from("Scan cancelled"));
    }));
    if std::env::var("YSE_SMOKE").is_ok() {
        path.set(String::from("/tmp"));
        scan_button.click();
        app.quit_after(400);
    }
    window.show();
    std::process::exit(app.exec());
}
