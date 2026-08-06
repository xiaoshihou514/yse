//! Phase 2/3 demonstration: a validated settings form with a declarative UI
//! tree, undo/redo, a standard dialog, and async work on the GUI thread.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use yse_model::{Command, UndoStack, Var, spawn_task};
use yse_ui::{
    Application, MessageBox, MessageBoxButtons, QtGuiScheduler, Window, button, checkbox, clone,
    column, label, line_edit, row,
};

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

    // Menus and toolbar: each menu builds inline, returning its action handles.
    let menubar = window.menu_bar();
    let (about, quit) = menubar.menu_with("File", |m| {
        (
            m.action("About"),
            m.separator().action("Quit").shortcut("Ctrl+Q"),
        )
    });
    let (undo_action, redo_action) = menubar.menu_with("Edit", |m| {
        (
            m.action("Undo").shortcut("Ctrl+Z"),
            m.action("Redo").shortcut("Ctrl+Y"),
        )
    });
    let about_box_action = menubar.menu_with("Help", |m| m.action("About Yse"));
    window.toolbar().add(&about);

    // State and derived signals.
    let name_var = Var::new(String::from("Ada"));
    let remember_var = Var::new(false);
    let status_var = Var::new(String::from("idle"));
    let load_var = Var::new(String::from("loading…"));
    let undo_stack = Rc::new(UndoStack::new());

    // Declarative widget tree: reads top-down; handlers and bindings are
    // registered on the widgets and die with the tree.
    let (greeting_label, status_label, load_label, submit, remember) = window.ui(|| {
        column(|| {
            let greeting_label = label("");
            greeting_label.bind_text(&name_var.signal().map(|n| format!("Hello, {n}!")));

            line_edit("Name").bind_text_two_way(&name_var);

            let remember = checkbox("Remember me");
            remember.bind_checked(&remember_var.signal());

            let status_label = label("idle");
            status_label.bind_text(&status_var.signal());

            let load_label = label("loading…");
            load_label.bind_text(&load_var.signal());

            let submit = row(|| {
                let submit = button("Submit");
                submit.bind_enabled(&name_var.signal().map(|n| !n.is_empty()));
                submit.on_click(clone!(status_var => move |_| {
                    status_var.set(String::from("submitted"));
                }));
                submit
            });

            (greeting_label, status_label, load_label, submit, remember)
        })
    });

    // Menu wiring.
    about.on_trigger(clone!(status_var => move |_| status_var.set(String::from("about"))));
    quit.on_trigger(clone!(status_var => move |_| status_var.set(String::from("quit"))));
    undo_action.on_trigger(clone!(undo_stack => move |_| {
        undo_stack.undo();
    }));
    redo_action.on_trigger(clone!(undo_stack => move |_| {
        undo_stack.redo();
    }));
    undo_action.bind_enabled(&undo_stack.can_undo());
    redo_action.bind_enabled(&undo_stack.can_redo());

    // Standard dialog: Help > About Yse.
    let about_box = MessageBox::new(
        &window,
        "About Yse",
        "Yse Phase 3 demo",
        MessageBoxButtons::Ok,
    );
    about_box_action.on_trigger(clone!(about_box => move |_| about_box.show()));
    let _about_box_result = about_box.result().observe(clone!(status_var => move |_| {
        status_var.set(String::from("help"));
    }));

    // Async work delivered onto the GUI thread.
    let load_task = spawn_task(Arc::new(QtGuiScheduler), |_token| {
        std::thread::sleep(Duration::from_millis(200));
        String::from("loaded")
    });
    let _load_sub = load_task
        .results()
        .observe(clone!(load_var => move |status| {
            load_var.set(status.clone());
        }));

    // Headless smoke run: drive the form, then quit.
    if std::env::var("YSE_SMOKE").is_ok() {
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
