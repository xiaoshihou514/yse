use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::*;

#[test]
fn declarative_tree_builds_and_retains_widgets() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let count = Var::new(0u32);

    let (label, button) = window.ui().column(|ui| {
        let label = ui.label("");
        label.bind_text(&count.signal().map(|c| format!("Count: {c}")));
        let button = ui.button("Increment");
        button.on_click(clone!(count => move |_| count.set(*count.value() + 1)));
        (label, button)
    });

    button.click();
    assert_eq!(label.text(), "Count: 1");

    // Dropping the handle does not kill the widget: the tree retained it.
    drop(button);
    count.set(7);
    assert_eq!(label.text(), "Count: 7");

    // Closing the window cascades: components and their owners are released.
    drop(window);
    count.set(9);
}

#[test]
fn laminar_style_views_mount_with_reactive_modifiers() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let count = Var::new(0u32);
    let caption = count.signal().map(|count| format!("Count: {count}"));
    let can_increment = count.signal().map(|count| *count < 2);

    let (label, (_, button)) = window.mount(column((
        label(caption),
        row((
            spacer(),
            button("Increment")
                .enabled(can_increment)
                .on_click(clone!(count => move |_| count.set(*count.value() + 1))),
        )),
    )));

    assert_eq!(label.text(), "Count: 0");
    button.click();
    assert_eq!(label.text(), "Count: 1");
    button.click();
    assert_eq!(label.text(), "Count: 2");
}

#[test]
fn controlled_input_uses_var_as_two_way_state() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let name = Var::new(String::from("Ada"));
    let (input,) = window.mount(column((line_edit("").controlled(name.clone()),)));

    assert_eq!(input.text(), "Ada");
    name.set(String::from("Grace"));
    assert_eq!(input.text(), "Grace");
}

#[test]
fn leaf_widgets_require_a_layout() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        window.ui().label("orphan");
    }));
    assert!(result.is_err(), "a leaf widget without a layout must panic");
}

#[test]
fn ui_scope_owns_non_widget_subscriptions() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let count = Var::new(0u32);

    window.ui().column(|ui| {
        ui.own(count.signal().observe(|_| {}));
    });
    assert_eq!(count.signal().observer_count(), 1);

    drop(window);
    assert_eq!(count.signal().observer_count(), 0);
}

#[test]
fn clone_macro_avoids_boilerplate() {
    let value = Rc::new(5i32);
    let closure = clone!(value => move || *value);
    assert_eq!(closure(), 5);
    // The original handle is still usable.
    assert_eq!(*value, 5);
}

#[test]
fn menus_chain_fluently() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let menubar = window.menu_bar();
    let file_menu = menubar.menu("File");
    let about = file_menu.action("About");
    let quit = file_menu.separator().action("Quit").shortcut("Ctrl+Q");

    let seen = Rc::new(RefCell::new(0usize));
    let seen_rc = seen.clone();
    let _sub = quit
        .triggered()
        .observe(move |_| *seen_rc.borrow_mut() += 1);
    quit.trigger();
    assert_eq!(*seen.borrow(), 1);

    // `about` and `quit` are separate handles from the same menu.
    let _ = about;
}
