//! ComboBox / SpinBox / Slider / ProgressBar behavior: values, ranges,
//! value-change streams, and signal bindings.

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::{Application, Window, column, combo_box, progress_bar, slider, spin_box};

fn combo_box_values_and_stream() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let combo = column.combo_box(["Ada", "Grace", "Linus"]);
    let seen: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    combo.on_value_change(move |index| seen_rc.borrow_mut().push(*index));

    assert_eq!(combo.current_index(), 0);
    assert_eq!(combo.current_text(), "Ada");

    combo.set_current_index(2);
    assert_eq!(combo.current_index(), 2);
    assert_eq!(combo.current_text(), "Linus");
    assert_eq!(*seen.borrow(), vec![2]);

    combo.set_current_text("Grace");
    assert_eq!(combo.current_index(), 1);

    combo.set_items(["X", "Y"]);
    assert_eq!(combo.current_index(), 0);
    assert_eq!(combo.current_text(), "X");
}

fn spin_box_and_slider_values() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let spin = column.spin_box(42);
    let slider = column.slider(30);
    let spin_seen: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let slider_seen: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));

    spin.on_value_change({
        let seen = spin_seen.clone();
        move |value| seen.borrow_mut().push(*value)
    });
    slider.on_value_change({
        let seen = slider_seen.clone();
        move |value| seen.borrow_mut().push(*value)
    });

    assert_eq!(spin.value(), 42);
    assert_eq!(slider.value(), 30);

    spin.set_value(50);
    slider.set_value(70);
    assert_eq!(spin.value(), 50);
    assert_eq!(slider.value(), 70);

    spin.set_range(-10, 10);
    spin.set_value(999);
    assert_eq!(spin.value(), 10, "spin box clamps to its range");

    slider.set_range(0, 5);
    slider.set_value(99);
    assert_eq!(slider.value(), 5, "slider clamps to its range");

    // Setting the same value emits no event; only real changes stream.
    let events_before = spin_seen.borrow().len();
    spin.set_value(10);
    assert_eq!(spin_seen.borrow().len(), events_before);
    assert_eq!(*spin_seen.borrow(), vec![50, 10]);
    assert_eq!(*slider_seen.borrow(), vec![70, 5]);
}

fn progress_bar_values() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let bar = column.progress_bar(0);
    assert_eq!(bar.value(), 0);

    bar.set_value(75);
    assert_eq!(bar.value(), 75);
    bar.set_range(0, 1000);
    bar.set_value(500);
    assert_eq!(bar.value(), 500);
}

fn value_bindings_follow_signals() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let index = Var::new(0i32);
    let amount = Var::new(10i32);

    let combo = column.combo_box(["a", "b", "c"]);
    combo.bind_value(&index.signal());
    let spin = column.spin_box(0);
    spin.bind_value(&amount.signal());
    let bar = column.progress_bar(0);
    bar.bind_value(&amount.signal());

    index.set(2);
    assert_eq!(combo.current_index(), 2);
    amount.set(60);
    assert_eq!(spin.value(), 60);
    assert_eq!(bar.value(), 60);

    // Dropping the window releases the bindings.
    drop(column);
    drop(window);
    assert_eq!(index.signal().observer_count(), 0);
    assert_eq!(amount.signal().observer_count(), 0);
}

fn declarative_views_mount_the_new_widgets() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let level = Var::new(5i32);
    let selected = Var::new(1i32);
    let seen: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();

    let (combo, spin, slider, bar) = window.mount(column((
        combo_box(["low", "mid", "high"])
            .value(selected.signal())
            .on_value_change(move |index| seen_rc.borrow_mut().push(*index)),
        spin_box(level.signal()).range(0, 20),
        slider(level.signal()).range(0, 20),
        progress_bar(level.signal()).range(0, 20),
    )));

    assert_eq!(combo.current_index(), 1);
    assert_eq!(spin.value(), 5);
    assert_eq!(slider.value(), 5);
    assert_eq!(bar.value(), 5);

    level.set(9);
    assert_eq!(spin.value(), 9);
    assert_eq!(slider.value(), 9);
    assert_eq!(bar.value(), 9);

    combo.set_current_index(2);
    assert_eq!(*seen.borrow(), vec![2]);
    assert_eq!(*selected.value(), 1, "one-way binding does not write back");
}

fn two_way_bindings_round_trip() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let index = Var::new(0i32);
    let amount = Var::new(10i32);
    let date = Var::new(String::from("2026-08-07"));
    let time = Var::new(String::from("09:00"));

    let combo = column.combo_box(["a", "b", "c"]);
    combo.bind_value_two_way(&index);
    let spin = column.spin_box(0);
    spin.bind_value_two_way(&amount);
    let date_edit = column.date_edit("2026-08-07");
    date_edit.bind_value_two_way(&date);
    let time_edit = column.time_edit("09:00");
    time_edit.bind_value_two_way(&time);

    // State drives the widgets.
    index.set(2);
    amount.set(60);
    assert_eq!(combo.current_index(), 2);
    assert_eq!(spin.value(), 60);

    // Widget changes drive the state back (user edits round-trip).
    combo.set_current_index(1);
    spin.set_value(75);
    date_edit.set_value("2030-01-02");
    time_edit.set_value("23:59");
    assert_eq!(*index.value(), 1);
    assert_eq!(*amount.value(), 75);
    assert_eq!(date.value().as_str(), "2030-01-02");
    assert_eq!(time.value().as_str(), "23:59");

    drop(column);
    drop(window);
}

// Qt permits one QApplication per process and binds it to its creating
// thread. Keeping this integration binary to one test prevents Rust's test
// harness from running the cases on different worker threads.
#[test]
fn value_widget_cases_share_one_qt_thread() {
    combo_box_values_and_stream();
    spin_box_and_slider_values();
    progress_bar_values();
    value_bindings_follow_signals();
    declarative_views_mount_the_new_widgets();
    two_way_bindings_round_trip();
}
