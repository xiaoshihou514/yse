//! Tab pages, table header clicks, and the painted line chart.

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Var;
use yse_ui::{Application, StringTableModel, Window};

fn tab_pages_build_and_release() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let tabs = column.tab_widget();

    let process_page = tabs.add_tab("进程");
    let process_button = process_page.button("结束任务");
    let performance_page = tabs.add_tab("性能");
    let performance_chart = performance_page.line_chart();
    assert_eq!(tabs.count(), 2, "both pages must be registered");
    assert!(process_button.width() > 0, "page widgets must have size");

    // Both pages are alive and functional.
    process_button.set_enabled(false);
    performance_chart.set_series(vec![0.0, 25.0, 50.0, 100.0]);

    let flag = Var::new(true);
    process_button.bind_enabled(&flag.signal());
    assert!(flag.signal().observer_count() >= 1);

    drop(column);
    drop(window);
    // The tab pages' bindings die with the tab widget.
    assert_eq!(flag.signal().observer_count(), 0);
}

fn header_clicks_stream_and_survive_teardown() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringTableModel::new(2, vec![String::from("Name"), String::from("CPU")]);
    model.push_row(vec![String::from("terminal"), String::from("54.9")]);
    model.push_row(vec![String::from("explorer"), String::from("3.2")]);
    let view = column.table_view(&model);

    let seen: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = view
        .header_clicked()
        .observe(move |section| seen_rc.borrow_mut().push(*section));

    view.click_header(0);
    view.click_header(1);
    view.click_header(0);
    assert_eq!(*seen.borrow(), vec![0, 1, 0]);

    drop(column);
    drop(window);
    // Clicking a destroyed table's header is a safe no-op.
    view.click_header(1);
    assert_eq!(*seen.borrow(), vec![0, 1, 0]);
}

fn line_chart_accepts_single_and_multi_series() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let chart = column.line_chart();

    chart.set_series(vec![1.0, 2.0, 3.0]);
    // Two overlaid series of three points each, row-major.
    chart.set_series_multi(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2);
    chart.set_series_multi(Vec::new(), 0);

    drop(column);
    drop(window);
}

fn reactive_bindings_drive_widgets() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();

    // Table rows bound to a signal.
    let rows = Var::new(Vec::<Vec<String>>::new());
    let model = StringTableModel::new(2, vec![String::from("Name"), String::from("Score")]);
    model.bind_rows(&rows.signal());
    let _view = column.table_view(&model);
    rows.set(vec![
        vec![String::from("Ada"), String::from("92")],
        vec![String::from("Grace"), String::from("98")],
    ]);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.cell(1, 0), "Grace");

    // Button label bound to a signal.
    let caption = Var::new(String::from("Go"));
    let button = column.button("");
    button.bind_text(&caption.signal());
    assert_eq!(button.text(), "Go");
    caption.set(String::from("Stop"));
    assert_eq!(button.text(), "Stop");

    // Visibility bound to a signal.
    let visible = Var::new(false);
    let label = column.label("secret");
    label.bind_visible(&visible.signal());
    visible.set(true);
    visible.set(false);

    // Chart series bound to signals (single and multi).
    let series = Var::new(vec![1.0, 2.0, 3.0]);
    let chart = column.line_chart();
    chart.bind_series(&series.signal());
    series.set(vec![4.0, 5.0]);
    let cores = Var::new(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    let cores_chart = column.line_chart();
    cores_chart.bind_series_multi(&cores.signal());
    cores.set(vec![vec![5.0], vec![6.0]]);

    drop(column);
    drop(window);
}

fn context_menu_reports_the_row() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringTableModel::new(1, vec![String::from("Name")]);
    model.push_row([String::from("terminal")]);
    model.push_row([String::from("explorer")]);
    let view = column.table_view(&model);

    let seen: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = view
        .context_menu()
        .observe(move |row| seen_rc.borrow_mut().push(*row));

    view.emit_context_menu(1);
    view.emit_context_menu(0);
    assert_eq!(*seen.borrow(), vec![1, 0]);

    drop(column);
    drop(window);
    // Emitting on a destroyed view is a safe no-op.
    view.emit_context_menu(1);
    assert_eq!(*seen.borrow(), vec![1, 0]);
}

fn color_scheme_and_table_polish() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let app = Application::init();
    // The scheme APIs must run in either mode without panicking.
    let _dark = app.system_dark();
    app.apply_color_scheme(true);
    app.apply_color_scheme(false);
    let _followed = app.follow_system_color_scheme();

    let window = Window::new();
    let column = window.column();
    let model = StringTableModel::new(2, vec![String::from("Name"), String::from("CPU")]);
    model.push_row([String::from("terminal"), String::from("10")]);
    model.push_row([String::from("editor"), String::from("20")]);
    let view = column.table_view(&model);

    view.select_rows(true);
    view.set_alternating_row_colors(true);

    // Icons resolve from real executable paths; empty paths stay iconless.
    let paths = vec![String::from("/bin/ls"), String::new()];
    model.set_row_icons(paths);
    assert!(model.row_icon_count() >= 1, "file icons must resolve");

    let paths_var = Var::new(vec![String::new(), String::from("/bin/sh")]);
    model.bind_row_icons(&paths_var.signal());
    assert!(model.row_icon_count() >= 1, "bound icons must apply");

    // Rows and icons bound from the SAME source: a row reset must not wipe
    // the icons that arrive in the same propagation pass.
    let data = Var::new(vec![(vec![String::from("a")], String::from("/bin/ls"))]);
    let m2 = StringTableModel::new(1, vec![String::from("x")]);
    m2.bind_table(&data.signal());
    data.set(vec![
        (vec![String::from("b")], String::from("/bin/sh")),
        (vec![String::from("c")], String::from("/bin/ls")),
    ]);
    assert!(
        m2.row_icon_count() >= 2,
        "rows and icons must apply atomically"
    );

    drop(column);
    drop(window);
}

// Qt permits one QApplication per process and binds it to its creating
// thread. Keeping this integration binary to one test prevents Rust's test
// harness from running the cases on different worker threads.
#[test]
fn panel_cases_share_one_qt_thread() {
    tab_pages_build_and_release();
    header_clicks_stream_and_survive_teardown();
    line_chart_accepts_single_and_multi_series();
    reactive_bindings_drive_widgets();
    context_menu_reports_the_row();
    color_scheme_and_table_polish();
}
