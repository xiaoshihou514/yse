//! Phase 3 demonstration: a small data browser.
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
    let status_rc = status.clone();
    let records_rc = records.clone();
    let subscription = task.results().observe(move |result| match result {
        Ok(rows) => {
            let count = rows.len();
            records_rc.replace_all(rows.to_vec());
            status_rc.set(format!("loaded {count} rows"));
        }
        Err(error) => {
            status_rc.set(format!("error: {error}"));
        }
    });
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
    let column = window.column();

    // Menus and shortcuts.
    let menubar = window.menu_bar();
    let file_menu = menubar.menu("File");
    let load_action = file_menu.action("Load records");
    load_action.set_shortcut("Ctrl+O");
    let cancel_action = file_menu.action("Cancel load");
    let fail_action = file_menu.action("Simulate failure");
    file_menu.separator();
    let quit_action = file_menu.action("Quit");
    quit_action.set_shortcut("Ctrl+Q");
    let edit_menu = menubar.menu("Edit");
    let undo_action = edit_menu.action("Undo");
    undo_action.set_shortcut("Ctrl+Z");
    let redo_action = edit_menu.action("Redo");
    redo_action.set_shortcut("Ctrl+Y");
    let view_menu = menubar.menu("View");
    let sort_name = view_menu.action("Sort by name");
    let sort_score = view_menu.action("Sort by score");
    let help_menu = menubar.menu("Help");
    let about_action = help_menu.action("About");

    // State and persistence.
    let settings = Settings::new("Yse", "DataBrowser");
    let records: Rc<ListModel<Vec<String>>> = Rc::new(ListModel::new());
    let filter_var = Var::new(String::new());
    let sort_col = Var::new(0usize);
    let status_var = Var::new(String::from("idle"));
    let visible_indices: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let undo_stack = Rc::new(UndoStack::new());

    // Table + filter + form + status.
    let table = StringTableModel::new(2, vec![String::from("Name"), String::from("Score")]);
    let view = column.table_view(&table);
    let filter_edit = column.line_edit("Filter");
    filter_edit.bind_text_two_way(&filter_var);
    let name_edit = column.line_edit("Name");
    let score_edit = column.line_edit("Score");
    let apply = column.button("Apply edit");
    let status_label = column.label("idle");
    status_label.bind_text(&status_var.signal());

    // Reactive refresh of the visible (filtered + sorted) rows.
    let records_rc = records.clone();
    let filter_rc = filter_var.clone();
    let sort_rc = sort_col.clone();
    let table_rc = table.clone();
    let visible_rc = visible_indices.clone();
    let _records_sub = records.changes().observe(move |_| {
        refresh_view(
            &records_rc,
            &filter_rc.value(),
            *sort_rc.value(),
            &table_rc,
            &visible_rc,
        );
    });
    let records_rc = records.clone();
    let filter_rc = filter_var.clone();
    let sort_rc = sort_col.clone();
    let table_rc = table.clone();
    let visible_rc = visible_indices.clone();
    let settings_rc = settings.clone();
    let _filter_sub = filter_var.signal().changes().observe(move |_| {
        settings_rc.set("filter", &filter_rc.value());
        refresh_view(
            &records_rc,
            &filter_rc.value(),
            *sort_rc.value(),
            &table_rc,
            &visible_rc,
        );
    });

    // Selection -> form.
    let records_rc = records.clone();
    let visible_rc = visible_indices.clone();
    let name_edit_rc = name_edit.clone();
    let score_edit_rc = score_edit.clone();
    let _selection_sub = view.selection_changed().observe(move |rows| {
        if let Some(&row) = rows.first()
            && let Some(&index) = visible_rc.borrow().get(row)
            && let Some(record) = records_rc.get(index)
        {
            name_edit_rc.set_text(record[0].clone());
            score_edit_rc.set_text(record[1].clone());
        }
    });

    // Undo/redo actions.
    undo_action.bind_enabled(&undo_stack.can_undo());
    redo_action.bind_enabled(&undo_stack.can_redo());
    let undo_rc = undo_stack.clone();
    let _undo_sub = undo_action.triggered().observe(move |_| {
        undo_rc.undo();
    });
    let redo_rc = undo_stack.clone();
    let _redo_sub = redo_action.triggered().observe(move |_| {
        redo_rc.redo();
    });

    // Apply edit: push an undoable SetRow command.
    let records_rc = records.clone();
    let visible_rc = visible_indices.clone();
    let view_rc = view.clone();
    let name_edit_rc = name_edit.clone();
    let score_edit_rc = score_edit.clone();
    let undo_rc = undo_stack.clone();
    let _apply_sub = apply.clicked().observe(move |_| {
        if let Some(&row) = view_rc.selected_rows().first()
            && let Some(&index) = visible_rc.borrow().get(row)
            && let Some(old) = records_rc.get(index)
        {
            let new = vec![name_edit_rc.text(), score_edit_rc.text()];
            if new != old {
                undo_rc.push(Box::new(SetRow {
                    records: records_rc.clone(),
                    index,
                    old,
                    new,
                }));
            }
        }
    });

    // Sorting actions.
    let records_rc = records.clone();
    let filter_rc = filter_var.clone();
    let sort_rc = sort_col.clone();
    let table_rc = table.clone();
    let visible_rc = visible_indices.clone();
    let settings_rc = settings.clone();
    let _sort_name_sub = sort_name.triggered().observe(move |_| {
        sort_rc.set(0);
        settings_rc.set("sort", "0");
        refresh_view(&records_rc, &filter_rc.value(), 0, &table_rc, &visible_rc);
    });
    let records_rc = records.clone();
    let filter_rc = filter_var.clone();
    let sort_rc = sort_col.clone();
    let table_rc = table.clone();
    let visible_rc = visible_indices.clone();
    let settings_rc = settings.clone();
    let _sort_score_sub = sort_score.triggered().observe(move |_| {
        sort_rc.set(1);
        settings_rc.set("sort", "1");
        refresh_view(&records_rc, &filter_rc.value(), 1, &table_rc, &visible_rc);
    });

    // About dialog.
    let about_box = MessageBox::new(
        &window,
        "About",
        "Yse data browser (Phase 3)",
        MessageBoxButtons::Ok,
    );
    let about_box_rc = about_box.clone();
    let _about_trigger = about_action.triggered().observe(move |_| {
        about_box_rc.show();
    });

    // Quit: flush settings to disk.
    let settings_rc = settings.clone();
    let _quit_sub = quit_action.triggered().observe(move |_| settings_rc.sync());

    // Async loading with cancellation and error states.
    let load_cell: Rc<RefCell<Option<LoadTask>>> = Rc::new(RefCell::new(None));
    let result_subs: Rc<RefCell<Vec<yse_model::Subscription>>> = Rc::new(RefCell::new(Vec::new()));
    let status_rc = status_var.clone();
    let records_rc = records.clone();
    let cell_rc = load_cell.clone();
    let subs_rc = result_subs.clone();
    let start_load = move || {
        status_rc.set(String::from("loading…"));
        let task = spawn_task(Arc::new(QtGuiScheduler), load_records);
        attach_result(&task, &status_rc, &records_rc, &subs_rc);
        *cell_rc.borrow_mut() = Some(task);
    };
    let start_load = Rc::new(start_load);

    let status_rc = status_var.clone();
    let records_rc = records.clone();
    let subs_rc = result_subs.clone();
    let cell_rc = load_cell.clone();
    let start_fail = move || {
        status_rc.set(String::from("loading…"));
        let task = spawn_task(Arc::new(QtGuiScheduler), fail_load);
        attach_result(&task, &status_rc, &records_rc, &subs_rc);
        // Keep the task alive: dropping it would cancel it and remove its
        // delivery slot.
        *cell_rc.borrow_mut() = Some(task);
    };
    let start_fail = Rc::new(start_fail);

    let start_rc = start_load.clone();
    let _load_sub = load_action.triggered().observe(move |_| start_rc());
    let fail_rc = start_fail.clone();
    let _fail_sub = fail_action.triggered().observe(move |_| fail_rc());
    let cell_rc = load_cell.clone();
    let status_rc = status_var.clone();
    let _cancel_sub = cancel_action.triggered().observe(move |_| {
        // Cancellation suppresses the (never-delivered) result by design.
        status_rc.set(String::from("load cancelled"));
        if let Some(task) = cell_rc.borrow().as_ref() {
            task.cancel();
        }
    });

    // Restore persisted settings.
    filter_var.set(settings.value("filter").unwrap_or_default());
    sort_col.set(
        settings
            .value("sort")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    );

    // Start the first load.
    start_load();

    // Headless smoke run: drive the whole flow, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
        app.quit_after(2500);
        let filter_rc = filter_var.clone();
        let sort_rc = sort_col.clone();
        let records_rc = records.clone();
        let table_rc = table.clone();
        let visible_rc = visible_indices.clone();
        let view_rc = view.clone();
        let name_edit_rc = name_edit.clone();
        let apply_rc = apply.clone();
        let undo_rc = undo_action.clone();
        let load_rc = load_action.clone();
        let cancel_rc = cancel_action.clone();
        let fail_rc = fail_action.clone();
        let about_rc = about_action.clone();
        let about_box_rc = about_box.clone();
        app.after(450, move || {
            filter_rc.set(String::from("grace"));
            sort_rc.set(1);
            refresh_view(&records_rc, &filter_rc.value(), 1, &table_rc, &visible_rc);
            app.after(100, move || {
                view_rc.select(0);
                app.after(80, move || {
                    name_edit_rc.set_text(String::from("Grace Hopper X"));
                    apply_rc.click();
                    app.after(80, move || {
                        undo_rc.trigger();
                        app.after(80, move || {
                            load_rc.trigger();
                            app.after(40, move || {
                                cancel_rc.trigger();
                                app.after(100, move || {
                                    fail_rc.trigger();
                                    app.after(150, move || {
                                        about_rc.trigger();
                                        about_box_rc.accept();
                                    });
                                });
                            });
                        });
                    });
                });
            });
        });
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
