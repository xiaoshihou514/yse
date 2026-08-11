use yse_model::Var;
use yse_ui::{Application, Window};

fn closing_window_releases_all_bindings() {
    // Qt has no display on CI runners; the offscreen platform is enough.
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let button = column.button("Go");
    let grid = window.grid();
    let grid_label = grid.label("cell", 0, 0);
    column.add(&grid);
    let _grid_label_text = grid_label.text();
    let flag = Var::new(true);

    button.bind_enabled(&flag.signal());
    assert!(flag.signal().observer_count() >= 1);

    drop(window);

    // Destroying the Qt tree fires the destroyed hooks, which clear every
    // component's owner: no manual connection cleanup needed.
    assert_eq!(flag.signal().observer_count(), 0);
}

fn menu_actions_trigger_and_release_with_window() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let menubar = window.menu_bar();
    let file_menu = menubar.menu("File");
    let action = file_menu.action("Quit");
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = action
        .triggered()
        .observe(move |()| seen_rc.borrow_mut().push(()));

    action.trigger();
    assert_eq!(seen.borrow().len(), 1);

    let flag = Var::new(true);
    action.bind_enabled(&flag.signal());
    assert!(flag.signal().observer_count() >= 1);

    drop(window);
    // The action's bindings are released with the window, and triggering a
    // destroyed action is a safe no-op.
    assert_eq!(flag.signal().observer_count(), 0);
    action.trigger();
    assert_eq!(seen.borrow().len(), 1);
}

fn child_dropped_before_window_is_safe() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let button = column.button("Go");

    // Rust wrappers go away before their Qt parent; the later Qt teardown
    // must not fire callbacks through freed wrapper state.
    drop(column);
    drop(button);
    drop(window);
}

fn temporary_child_keeps_its_binding_until_window_teardown() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let flag = Var::new(true);

    // The only public Button wrapper is immediately dropped. The window's
    // retained component tree must still own the binding while its native
    // child exists.
    window.column().button("Go").bind_enabled(&flag.signal());
    assert_eq!(flag.signal().observer_count(), 1);

    drop(window);
    assert_eq!(flag.signal().observer_count(), 0);
}

#[test]
fn lifecycle_cases_share_one_qt_thread() {
    closing_window_releases_all_bindings();
    menu_actions_trigger_and_release_with_window();
    child_dropped_before_window_is_safe();
    temporary_child_keeps_its_binding_until_window_teardown();
}
