//! Phase 2 demonstration: a validated settings form built from yse-model
//! state and yse-ui widgets, with automatic cleanup when the window closes.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use yse_model::{Command, UndoStack, Var, spawn_task};
use yse_ui::{Application, MessageBox, MessageBoxButtons, QtGuiScheduler, Window};

/// A reversible name edit, demonstrating undo/redo integration.
struct SetName {
    var: Var<String>,
    old: String,
    new: String,
}

impl Command for SetName {
    fn execute(&self) {
        self.var.set(self.new.clone());
    }

    fn undo(&self) {
        self.var.set(self.old.clone());
    }

    fn text(&self) -> String {
        String::from("change name")
    }
}

fn main() {
    let app = Application::init();
    let window = Window::new();
    window.set_title("Yse settings");
    window.set_size(360, 220);
    let column = window.column();

    // Menus and toolbars.
    let menubar = window.menu_bar();
    let file_menu = menubar.menu("File");
    let about = file_menu.action("About");
    file_menu.separator();
    let quit = file_menu.action("Quit");
    quit.set_shortcut("Ctrl+Q");
    let edit_menu = menubar.menu("Edit");
    let undo_action = edit_menu.action("Undo");
    let redo_action = edit_menu.action("Redo");
    let help_menu = menubar.menu("Help");
    let about_box_action = help_menu.action("About Yse");
    let toolbar = window.toolbar();
    toolbar.add(&about);

    // Form state.
    let name_var = Var::new(String::from("Ada"));

    // Derived state: a greeting and submit enablement.
    let greeting = name_var.signal().map(|n| format!("Hello, {n}!"));
    let can_submit = name_var.signal().map(|n| !n.is_empty());

    let greeting_label = column.label("");
    greeting_label.bind_text(&greeting);

    let name = column.line_edit("Name");
    name.bind_text_two_way(&name_var);

    let remember = column.checkbox("Remember me");
    let remember_var = Var::new(false);
    remember.bind_checked(&remember_var.signal());

    let status_label = column.label("idle");
    let status_var = Var::new(String::from("idle"));
    status_label.bind_text(&status_var.signal());

    let submit = column.button("Submit");
    submit.bind_enabled(&can_submit);
    let status_var_rc = status_var.clone();
    let _sub = submit
        .clicked()
        .observe(move |_| status_var_rc.set(String::from("submitted")));

    let about_var = status_var.clone();
    let _about_sub = about
        .triggered()
        .observe(move |_| about_var.set(String::from("about")));
    let quit_var = status_var.clone();
    let _quit_sub = quit
        .triggered()
        .observe(move |_| quit_var.set(String::from("quit")));

    // Undo/redo integration: menu actions bound to the stack's signals.
    let undo_stack = Rc::new(UndoStack::new());
    undo_action.bind_enabled(&undo_stack.can_undo());
    redo_action.bind_enabled(&undo_stack.can_redo());
    let undo_stack_rc = undo_stack.clone();
    let _undo_sub = undo_action.triggered().observe(move |_| {
        undo_stack_rc.undo();
    });
    let undo_stack_rc = undo_stack.clone();
    let _redo_sub = redo_action.triggered().observe(move |_| {
        undo_stack_rc.redo();
    });

    // Standard dialog: a Help > About message box.
    let about_box = MessageBox::new(
        &window,
        "About Yse",
        "Yse Phase 3 demo",
        MessageBoxButtons::Ok,
    );
    let about_box_rc = about_box.clone();
    let _about_box_trigger = about_box_action
        .triggered()
        .observe(move |_| about_box_rc.show());
    let help_var = status_var.clone();
    let _about_box_result = about_box
        .result()
        .observe(move |_| help_var.set(String::from("help")));

    // Async work delivered onto the GUI thread through the Qt scheduler.
    let load_label = column.label("loading…");
    let load_var = Var::new(String::from("loading…"));
    load_label.bind_text(&load_var.signal());
    let load_task = spawn_task(Arc::new(QtGuiScheduler), |_token| {
        std::thread::sleep(Duration::from_millis(200));
        String::from("loaded")
    });
    let load_var_rc = load_var.clone();
    let _load_sub = load_task
        .results()
        .observe(move |status| load_var_rc.set(status.clone()));

    // Headless smoke run: drive the form, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
        // A command-driven edit: executes, then can be undone via the menu.
        undo_stack.push(Box::new(SetName {
            var: name_var.clone(),
            old: String::from("Ada"),
            new: String::from("Grace"),
        }));
        submit.click(); // enabled now that the name is non-empty
        about.trigger(); // menu/toolbar action -> stream -> state
        undo_action.trigger(); // undo the name edit via the menu action
        about_box_action.trigger(); // show the About box
        about_box.accept(); // close it: result stream -> state
        app.quit_after(800);
    }

    window.show();
    let code = app.exec();
    println!(
        "[settings] greeting={:?} status={:?} load={:?} remember={}",
        greeting_label.text(),
        status_label.text(),
        load_label.text(),
        remember.checked()
    );
    std::process::exit(code);
}
