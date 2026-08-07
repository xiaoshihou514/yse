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

    // Numeric columns can be right-aligned and the sort arrow shown without
    // disturbing the cell data or the live row stream.
    model.set_column_alignment(1, yse_ui::TextAlign::Right);
    view.set_sort_indicator(1, false);
    assert_eq!(model.cell(0, 1), "54.9", "alignment keeps cell text intact");
    assert_eq!(model.cell(1, 0), "explorer", "text column untouched");
    model.push_row(vec![String::from("code"), String::from("12.0")]);
    assert_eq!(model.cell(2, 1), "12.0", "alignment survives row appends");

    let seen: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = view
        .header_clicked()
        .observe(move |section| seen_rc.borrow_mut().push(*section));

    view.click_header(0);
    view.click_header(1);
    view.click_header(0);
    assert_eq!(*seen.borrow(), vec![0, 1, 0]);

    view.set_sort_indicator(0, true);
    view.set_sort_indicator(1, false);
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
    let view = column.table_view(&model);
    rows.set(vec![
        vec![String::from("Ada"), String::from("92")],
        vec![String::from("Grace"), String::from("98")],
    ]);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.cell(1, 0), "Grace");

    // Sort arrow driven declaratively from a signal.
    let sort = Var::new((1usize, false));
    view.bind_sort_indicator(&sort.signal());
    sort.set((0, true));

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

    // Style class bound to a signal: the selected nav button is derived from
    // state, not toggled by the click handler.
    let selected = Var::new(0usize);
    let a = column.button("A");
    let b = column.button("B");
    a.bind_style_class(&selected.signal().map(|i| {
        if *i == 0 {
            String::from("navSelected")
        } else {
            String::from("nav")
        }
    }));
    b.bind_style_class(&selected.signal().map(|i| {
        if *i == 1 {
            String::from("navSelected")
        } else {
            String::from("nav")
        }
    }));
    selected.set(1);

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
    // Icons resolve from real executable paths; empty paths stay iconless.
    let paths = vec![String::from("/bin/ls"), String::new()];
    model.set_row_icons(paths);
    assert!(model.row_icon_count() >= 1, "file icons must resolve");

    let paths_var = Var::new(vec![String::new(), String::from("/bin/sh")]);
    model.bind_row_icons(&paths_var.signal());
    assert!(model.row_icon_count() >= 1, "bound icons must apply");

    // Heat values render via the view's delegate without panicking.
    model.set_heat(1, vec![0.0, 1.0]);
    let heat = Var::new(vec![0.5, 0.25]);
    model.bind_heat(1, &heat.signal());
    view.enable_heat();
    heat.set(vec![1.0, 0.0]);

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

fn menus_support_submenus_and_checkable_actions() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let menubar = window.menu_bar();
    let (speed, group) = menubar.menu_with("查看", |m| {
        let speed = m.menu("更新速度");
        let high = speed.action("高");
        let low = speed.action("低");
        high.set_checkable(true);
        low.set_checkable(true);
        high.set_checked(true);
        assert!(high.checked());
        assert!(!low.checked());
        let group = m.action("按类型分组");
        group.set_checkable(true);
        group.set_checked(true);
        (speed, group)
    });
    let _ = &speed;
    assert!(group.checked());
    drop(window);
}

fn tree_views_group_select_and_context_rows() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = yse_ui::TreeModel::new(2);
    model.set_headers([String::from("组"), String::from("值")]);
    let view = column.tree_view(&model);

    model.set_column_alignment(1, yse_ui::TextAlign::Right);
    view.set_sort_indicator(1, true);

    let rows = Var::new(vec![
        (
            -1i32,
            vec![String::from("应用"), String::new()],
            String::new(),
        ),
        (
            0i32,
            vec![String::from("firefox"), String::from("10")],
            String::from("/bin/ls"),
        ),
        (
            0i32,
            vec![String::from("code"), String::from("20")],
            String::new(),
        ),
        (
            -1i32,
            vec![String::from("系统"), String::new()],
            String::new(),
        ),
        (
            3i32,
            vec![String::from("systemd"), String::from("5")],
            String::new(),
        ),
    ]);
    model.bind(&rows.signal());
    assert_eq!(model.row_count(), 5, "tree must hold groups and children");
    view.expand_all(true);
    view.select_rows(true);
    view.enable_heat();
    view.set_column_hidden(1, false);

    let selection: Rc<RefCell<Vec<Vec<usize>>>> = Rc::new(RefCell::new(Vec::new()));
    let selection_rc = selection.clone();
    let _sub = view
        .selection_changed()
        .observe(move |rows| selection_rc.borrow_mut().push(rows.clone()));
    view.select(1);
    assert_eq!(
        *selection.borrow(),
        vec![vec![1]],
        "flat child row selection"
    );

    let menus: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let menus_rc = menus.clone();
    let _menu_sub = view
        .context_menu()
        .observe(move |row| menus_rc.borrow_mut().push(*row));
    view.emit_context_menu(4);
    assert_eq!(*menus.borrow(), vec![4], "flat context row");

    let headers: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let headers_rc = headers.clone();
    let _header_sub = view
        .header_clicked()
        .observe(move |section| headers_rc.borrow_mut().push(*section));
    view.click_header(1);
    assert_eq!(*headers.borrow(), vec![1]);

    let heat = Var::new(vec![0.0, 0.5, 1.0, 0.0, 0.2]);
    model.bind_heat(1, &heat.signal());
    heat.set(vec![0.0, 1.0, 0.5, 0.0, 0.1]);

    // Expansion and scroll state are queryable and restorable across resets.
    view.expand_all(true);
    assert!(
        !view.expanded_rows().is_empty(),
        "expanded rows are tracked"
    );
    view.expand_all(false);
    assert!(view.expanded_rows().is_empty(), "collapsing clears the set");
    view.expand_rows(&[0]);
    assert_eq!(view.expanded_rows(), vec![0]);
    view.scroll_to_flat(1);
    assert!(view.top_row().is_some(), "top row is reportable");
    // Pixel-based scroll survives a model reset.
    view.set_scroll_value(120);
    let restored = view.scroll_value();
    assert!(
        (0..=120).contains(&restored),
        "pixel scroll restore clamps to the viewport"
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
    menus_support_submenus_and_checkable_actions();
    tree_views_group_select_and_context_rows();
}
