use yse_ui::{Application, StringTableModel, Window};

#[test]
fn table_view_applies_incremental_row_changes() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringTableModel::new(2, vec![String::from("Name"), String::from("Score")]);
    let _view = column.table_view(&model);

    assert_eq!(model.column_count(), 2);
    assert_eq!(model.row_count(), 0);

    model.push_row(vec![String::from("Ada"), String::from("10")]);
    model.push_row(vec![String::from("Grace"), String::from("9")]);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.cell(0, 0), "Ada");
    assert_eq!(model.cell(0, 1), "10");
    assert_eq!(model.cell(1, 0), "Grace");

    model.insert_row(1, vec![String::from("Linus"), String::from("8")]);
    assert_eq!(model.row_count(), 3);
    assert_eq!(model.cell(1, 0), "Linus");

    model.remove_row(0);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.cell(0, 0), "Linus");

    model.set_row(1, vec![String::from("Grace"), String::from("10")]);
    assert_eq!(model.cell(1, 1), "10");

    model.replace_all(vec![
        vec![String::from("X"), String::from("1")],
        vec![String::from("Y"), String::from("2")],
    ]);
    assert_eq!(model.row_count(), 2);
    assert_eq!(model.cell(1, 1), "2");

    model.clear();
    assert_eq!(model.row_count(), 0);
}

#[test]
fn table_view_selection_streams_changes() {
    unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };

    let _app = Application::init();
    let window = Window::new();
    let column = window.column();
    let model = StringTableModel::new(2, vec![String::from("Name"), String::from("Score")]);
    model.push_row(vec![String::from("Ada"), String::from("10")]);
    model.push_row(vec![String::from("Grace"), String::from("9")]);
    let view = column.table_view(&model);

    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = view
        .selection_changed()
        .observe(move |rows| seen_rc.borrow_mut().push(rows.clone()));

    view.select(1);
    assert_eq!(view.selected_rows(), vec![1]);
    view.select(0);
    assert_eq!(view.selected_rows(), vec![0]);
    view.clear_selection();
    assert!(view.selected_rows().is_empty());
    assert_eq!(*seen.borrow(), vec![vec![1], vec![0], Vec::<usize>::new()]);
}
