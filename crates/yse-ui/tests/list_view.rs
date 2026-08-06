use yse_ui::{Application, StringListModel, Window};

fn list_view_applies_incremental_changes() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringListModel::new();
    let _view = column.list_view(&model);

    model.push("a");
    model.push("b");
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.row_text(0), "a");
    assert_eq!(model.row_text(1), "b");

    model.insert(1, "x");
    assert_eq!(model.row_count(), 3);
    assert_eq!(model.row_text(1), "x");

    model.remove(0);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.row_text(0), "x");

    model.set(0, "z");
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.row_text(0), "z");

    model.replace_all(vec!["m".to_string(), "n".to_string(), "o".to_string()]);
    assert_eq!(model.row_count(), 3);
    assert_eq!(model.row_text(2), "o");

    model.clear();
    assert_eq!(model.row_count(), 0);
}

fn list_view_selection_streams_changes() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringListModel::new();
    model.extend(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let view = column.list_view(&model);

    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = view
        .selection_changed()
        .observe(move |rows| seen_rc.borrow_mut().push(rows.clone()));

    assert!(view.selected_rows().is_empty());
    view.select(1);
    assert_eq!(view.selected_rows(), vec![1]);
    view.select(0);
    assert_eq!(view.selected_rows(), vec![0]);
    view.clear_selection();
    assert!(view.selected_rows().is_empty());

    assert_eq!(*seen.borrow(), vec![vec![1], vec![0], Vec::<usize>::new()]);
}

// Qt permits one QApplication per process and binds it to its creating
// thread. Keeping this integration binary to one test prevents Rust's test
// harness from running the two cases on different worker threads.
#[test]
fn list_view_cases_share_one_qt_thread() {
    list_view_applies_incremental_changes();
    list_view_selection_streams_changes();
}
