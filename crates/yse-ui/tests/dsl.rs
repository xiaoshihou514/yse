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

    let (label, button) = window.ui(|| {
        column(|| {
            let label = label("");
            label.bind_text(&count.signal().map(|c| format!("Count: {c}")));
            let button = button("Increment");
            button.on_click(clone!(count => move |_| count.set(*count.value() + 1)));
            (label, button)
        })
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
fn leaf_widgets_require_a_layout() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        window.ui(|| {
            label("orphan");
        });
    }));
    assert!(result.is_err(), "a leaf widget without a layout must panic");
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
