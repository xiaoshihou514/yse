//! Panics inside user callbacks must be contained at the FFI boundary: they
//! are logged and swallowed instead of unwinding through C++ (which CXX would
//! turn into an uncaught `rust::Error` exception and terminate the app).

use std::cell::RefCell;
use std::rc::Rc;
use yse_ui::{Application, StringTableModel, Window, clone};

type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// Silence the default panic hook while a test deliberately panics, so the
/// (expected) panics do not clutter test output. Restored on drop.
struct SilentPanics {
    previous: Option<PanicHook>,
}

impl SilentPanics {
    fn new() -> Self {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for SilentPanics {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(previous);
        }
    }
}

#[test]
fn panicking_click_handler_is_contained() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    let _silent = SilentPanics::new();

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let panic_button = column.button("Panic");
    let panic_button_2 = column.button("Panic too");

    panic_button.on_click(|_| panic!("click boom"));
    panic_button_2.on_click(|_| panic!("second boom"));

    // Neither panic unwinds through the Qt boundary; the test thread lives on.
    panic_button.click();
    panic_button_2.click();

    // The app is still fully functional afterwards.
    let count = yse_model::Var::new(0u32);
    let ok_button = column.button("Ok");
    ok_button.on_click(clone!(count => move |_| count.set(*count.value() + 1)));
    ok_button.click();
    assert_eq!(*count.value(), 1);

    drop(window);
}

#[test]
fn panicking_text_change_handler_is_contained() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    let _silent = SilentPanics::new();

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let edit = column.line_edit("");
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    edit.on_text_change(|_| panic!("text boom"));
    let seen_rc = seen.clone();
    edit.on_text_change(move |text| seen_rc.borrow_mut().push(text.clone()));

    // Programmatic writes still flow through the widget's textChanged signal;
    // the panicking handler is contained and the healthy one still runs.
    edit.set_text("hello");
    edit.set_text("world");

    assert_eq!(
        *seen.borrow(),
        vec![String::from("hello"), String::from("world")]
    );
    drop(window);
}

#[test]
fn panicking_selection_handler_is_contained() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    let _silent = SilentPanics::new();

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let table = StringTableModel::new(1, vec![String::from("Name")]);
    table.push_row([String::from("Ada")]);
    table.push_row([String::from("Grace")]);
    let view = column.table_view(&table);
    let seen: Rc<RefCell<Vec<Vec<usize>>>> = Rc::new(RefCell::new(Vec::new()));

    view.on_selection(|_| panic!("selection boom"));
    let seen_rc = seen.clone();
    view.on_selection(move |rows| seen_rc.borrow_mut().push(rows.clone()));

    view.select(1);
    view.select(0);

    assert_eq!(
        *seen.borrow(),
        vec![vec![1usize], vec![0usize]],
        "healthy selection handler must observe both changes"
    );
    drop(window);
}
