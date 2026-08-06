//! Phase 3 demonstration: a small data browser with a declarative UI tree.
//!
//! Asynchronously loads records into an incremental table, filters and sorts
//! them, edits a row through a form with undo/redo, shows progress/error
//! states, cancels loads, persists settings, and exposes menu shortcuts.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use yse_model::{CancellationToken, Command, ListModel, Task, UndoStack, Var, spawn_task};
use yse_ui::{
    Application, MessageBox, MessageBoxButtons, QtGuiScheduler, Settings, StringTableModel, Window,
    button, clone, column, label, line_edit, table_view,
};

type LoadResult = Result<Vec<Vec<String>>, String>;
type LoadTask = Task<LoadResult>;

fn sample_records() -> Vec<Vec<String>> {
    vec![
        vec![String::from("Ada Lovelace"), String::from("92")],
        vec![String::from("Grace Hopper"), String::from("98")],
        vec![String::from("Linus Torvalds"), String::from("87")],
        vec![String::from("Margaret Hamilton"), String::from("95")],
        vec![String::from("Alan Turing"), String::from("99")],
        vec![String::from("Edsger Dijkstra"), String::from("90")],
        vec![String::from("Barbara Liskov"), String::from("93")],
        vec![String::from("Ken Thompson"), String::from("85")],
    ]
}

fn load_records(token: &CancellationToken) -> LoadResult {
    for _ in 0..4 {
        if token.is_cancelled() {
            return Err(String::from("load cancelled"));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(sample_records())
}

fn fail_load(_token: &CancellationToken) -> LoadResult {
    Err(String::from("simulated failure"))
}

fn attach_result(
    task: &LoadTask,
    status: &Var<String>,
    records: &Rc<ListModel<Vec<String>>>,
    subs: &Rc<RefCell<Vec<yse_model::Subscription>>>,
) {
    let subscription = task
        .results()
        .observe(clone!(status, records => move |result| {
            match result {
                Ok(rows) => {
                    let count = rows.len();
                    records.replace_all(rows.to_vec());
                    status.set(format!("loaded {count} rows"));
                }
                Err(error) => {
                    status.set(format!("error: {error}"));
                }
            }
        }));
    subs.borrow_mut().push(subscription);
}

fn refresh_view(
    records: &ListModel<Vec<String>>,
    filter: &str,
    sort: usize,
    table: &StringTableModel,
    visible: &RefCell<Vec<usize>>,
) {
    let filter = filter.to_lowercase();
    let mut rows: Vec<(usize, Vec<String>)> = records
        .snapshot()
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row[0].to_lowercase().contains(&filter))
        .collect();
    rows.sort_by(|a, b| a.1[sort].cmp(&b.1[sort]));
    let mut visible_rows = Vec::with_capacity(rows.len());
    let mut indices = Vec::with_capacity(rows.len());
    for (index, row) in rows {
        indices.push(index);
        visible_rows.push(row);
    }
    *visible.borrow_mut() = indices;
    table.replace_all(visible_rows);
}

struct SetRow {
    records: Rc<ListModel<Vec<String>>>,
    index: usize,
    old: Vec<String>,
    new: Vec<String>,
}

impl Command for SetRow {
    fn execute(&self) {
        self.records.set(self.index, self.new.clone());
    }

    fn undo(&self) {
        self.records.set(self.index, self.old.clone());
    }

    fn text(&self) -> String {
        String::from("edit row")
    }
}

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("Yse data browser");
    window.set_size(680, 460);

    // Menus and shortcuts: each menu builds inline, returning its action
    // handles.
    let menubar = window.menu_bar();
    let (load_action, cancel_action, fail_action, quit_action) = menubar.menu_with("File", |m| {
        (
            m.action("Load records").shortcut("Ctrl+O"),
            m.action("Cancel load"),
            m.action("Simulate failure"),
            m.separator().action("Quit").shortcut("Ctrl+Q"),
        )
    });
    let (undo_action, redo_action) = menubar.menu_with("Edit", |m| {
        (
            m.action("Undo").shortcut("Ctrl+Z"),
            m.action("Redo").shortcut("Ctrl+Y"),
        )
    });
    let (sort_name, sort_score) = menubar.menu_with("View", |m| {
        (m.action("Sort by name"), m.action("Sort by score"))
    });
    let about_action = menubar.menu_with("Help", |m| m.action("About"));

    // State and persistence.
    let settings = Settings::new("Yse", "DataBrowser");
    let records: Rc<ListModel<Vec<String>>> = Rc::new(ListModel::new());
    let filter_var = Var::new(String::new());
    let sort_col = Var::new(0usize);
    let status_var = Var::new(String::from("idle"));
    let visible_indices: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let undo_stack = Rc::new(UndoStack::new());
    let table = StringTableModel::new(2, vec![String::from("Name"), String::from("Score")]);

    // Declarative widget tree: selection and apply handlers are registered on
    // the widgets themselves and are released with the tree.
    let (view, name_edit, apply, status_label) = window.ui(|| {
        column(|| {
            let view = table_view(&table);

            let filter_edit = line_edit("Filter");
            filter_edit.bind_text_two_way(&filter_var);

            let name_edit = line_edit("Name");
            let score_edit = line_edit("Score");

            view.on_selection(clone!(records, visible_indices, name_edit, score_edit => move |rows| {
                if let Some(&row) = rows.first()
                    && let Some(&index) = visible_indices.borrow().get(row)
                    && let Some(record) = records.get(index)
                {
                    name_edit.set_text(record[0].clone());
                    score_edit.set_text(record[1].clone());
                }
            }));

            let apply = button("Apply edit");
            apply.on_click(clone!(records, visible_indices, view, name_edit, score_edit, undo_stack => move |_| {
                if let Some(&row) = view.selected_rows().first()
                    && let Some(&index) = visible_indices.borrow().get(row)
                    && let Some(old) = records.get(index)
                {
                    let new = vec![name_edit.text(), score_edit.text()];
                    if new != old {
                        undo_stack.push(Box::new(SetRow {
                            records: records.clone(),
                            index,
                            old,
                            new,
                        }));
                    }
                }
            }));

            let status_label = label("idle");
            status_label.bind_text(&status_var.signal());

            (view, name_edit, apply, status_label)
        })
    });

    // Reactive refresh of the visible (filtered + sorted) rows.
    let _records_sub = records.changes().observe(clone!(records, filter_var, sort_col, table, visible_indices => move |_| {
        refresh_view(&records, &filter_var.value(), *sort_col.value(), &table, &visible_indices);
    }));
    let _filter_sub = filter_var.signal().changes().observe(clone!(records, filter_var, sort_col, table, visible_indices, settings => move |_| {
        settings.set("filter", &filter_var.value());
        refresh_view(&records, &filter_var.value(), *sort_col.value(), &table, &visible_indices);
    }));

    // Undo/redo actions.
    undo_action.on_trigger(clone!(undo_stack => move |_| {
        undo_stack.undo();
    }));
    redo_action.on_trigger(clone!(undo_stack => move |_| {
        undo_stack.redo();
    }));
    undo_action.bind_enabled(&undo_stack.can_undo());
    redo_action.bind_enabled(&undo_stack.can_redo());

    // Sorting actions.
    sort_name.on_trigger(
        clone!(sort_col, settings, records, filter_var, table, visible_indices => move |_| {
            sort_col.set(0);
            settings.set("sort", "0");
            refresh_view(&records, &filter_var.value(), 0, &table, &visible_indices);
        }),
    );
    sort_score.on_trigger(
        clone!(sort_col, settings, records, filter_var, table, visible_indices => move |_| {
            sort_col.set(1);
            settings.set("sort", "1");
            refresh_view(&records, &filter_var.value(), 1, &table, &visible_indices);
        }),
    );

    // About dialog and quit (flush settings).
    let about_box = MessageBox::new(
        &window,
        "About",
        "Yse data browser (Phase 3)",
        MessageBoxButtons::Ok,
    );
    about_action.on_trigger(clone!(about_box => move |_| about_box.show()));
    quit_action.on_trigger(clone!(settings => move |_| settings.sync()));

    // Async loading with cancellation and error states.
    let load_cell: Rc<RefCell<Option<LoadTask>>> = Rc::new(RefCell::new(None));
    let result_subs: Rc<RefCell<Vec<yse_model::Subscription>>> = Rc::new(RefCell::new(Vec::new()));
    let start_load = Rc::new(
        clone!(status_var, records, load_cell, result_subs => move || {
            status_var.set(String::from("loading…"));
            let task = spawn_task(Arc::new(QtGuiScheduler), load_records);
            attach_result(&task, &status_var, &records, &result_subs);
            *load_cell.borrow_mut() = Some(task);
        }),
    );
    let start_fail = Rc::new(
        clone!(status_var, records, load_cell, result_subs => move || {
            status_var.set(String::from("loading…"));
            let task = spawn_task(Arc::new(QtGuiScheduler), fail_load);
            attach_result(&task, &status_var, &records, &result_subs);
            // Keep the task alive: dropping it would cancel it and remove its
            // delivery slot.
            *load_cell.borrow_mut() = Some(task);
        }),
    );

    load_action.on_trigger(clone!(start_load => move |_| start_load()));
    fail_action.on_trigger(clone!(start_fail => move |_| start_fail()));
    cancel_action.on_trigger(clone!(status_var, load_cell => move |_| {
        // Cancellation suppresses the (never-delivered) result by design.
        status_var.set(String::from("load cancelled"));
        if let Some(task) = load_cell.borrow().as_ref() {
            task.cancel();
        }
    }));

    // Restore persisted settings and start the first load.
    filter_var.set(settings.value("filter").unwrap_or_default());
    sort_col.set(
        settings
            .value("sort")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    );
    start_load();

    // Headless smoke run: drive the whole flow, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
        app.quit_after(2500);
        app.after(
            450,
            clone!(filter_var, sort_col, records, table, visible_indices => move || {
                filter_var.set(String::from("grace"));
                sort_col.set(1);
                refresh_view(&records, &filter_var.value(), 1, &table, &visible_indices);
                app.after(100, clone!(view => move || {
                    view.select(0);
                    app.after(80, clone!(name_edit, apply => move || {
                        name_edit.set_text(String::from("Grace Hopper X"));
                        apply.click();
                        app.after(80, clone!(undo_action => move || {
                            undo_action.trigger();
                            app.after(80, clone!(load_action => move || {
                                load_action.trigger();
                                app.after(40, clone!(cancel_action => move || {
                                    cancel_action.trigger();
                                    app.after(100, clone!(fail_action => move || {
                                        fail_action.trigger();
                                        app.after(150, clone!(about_action, about_box => move || {
                                            about_action.trigger();
                                            about_box.accept();
                                        }));
                                    }));
                                }));
                            }));
                        }));
                    }));
                }));
            }),
        );
    }

    window.show();
    let code = app.exec();

    settings.sync();
    println!(
        "[data-browser] status={:?} rows={} filter={:?} first={:?}",
        status_label.text(),
        table.row_count(),
        filter_var.value().as_str(),
        table.cell(0, 0),
    );
    std::process::exit(code);
}
