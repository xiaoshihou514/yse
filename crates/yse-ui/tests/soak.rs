//! Soak test: repeatedly build and tear down a small window tree, asserting
//! that every reactive binding is released when the native tree dies.

use yse_model::Var;
use yse_ui::{Application, StringTableModel, Window};

#[test]
fn repeated_window_open_close_releases_all_bindings() {
    // Qt has no display on CI runners; the offscreen platform is enough.
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let enabled = Var::new(true);
    let text = Var::new(String::from("hello"));
    let checked = Var::new(false);

    for iteration in 0..40 {
        let window = Window::new();
        let column = window.column();

        let button = column.button("Go");
        button.bind_enabled(&enabled.signal());

        let edit = column.line_edit("");
        edit.bind_text_two_way(&text);

        let checkbox = column.checkbox("Opt in");
        checkbox.bind_checked(&checked.signal());

        let model = StringTableModel::new(1, vec![String::from("Name")]);
        model.push_row([String::from("Ada")]);
        model.push_row([String::from("Grace")]);
        let view = column.table_view(&model);
        // A selection stream whose observer lives in the table's owner.
        view.on_selection(|_| {});

        let action = window.menu_bar().menu("File").action("Quit");
        action.bind_enabled(&enabled.signal());

        // Every binding is live while the tree is alive.
        assert!(
            enabled.signal().observer_count() >= 2,
            "iteration {iteration}"
        );
        assert!(text.signal().observer_count() >= 1, "iteration {iteration}");
        assert!(
            checked.signal().observer_count() >= 1,
            "iteration {iteration}"
        );

        // Exercise the tree so callbacks and sinks exist, then tear it down
        // in the same order an application would.
        button.click();
        view.select(1);
        drop(column);
        drop(window);

        // Destroying the Qt tree fires the destroyed hooks, which clear every
        // component's owner. No binding may survive to the next iteration.
        assert_eq!(
            enabled.signal().observer_count(),
            0,
            "iteration {iteration}"
        );
        assert_eq!(text.signal().observer_count(), 0, "iteration {iteration}");
        assert_eq!(
            checked.signal().observer_count(),
            0,
            "iteration {iteration}"
        );
    }
}
