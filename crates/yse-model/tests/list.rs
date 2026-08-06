use yse_model::*;

#[test]
fn insert_emits_structural_change_and_updates_snapshot() {
    let model = ListModel::new();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = model.changes().observe(move |change| {
        seen_rc.borrow_mut().push(change.clone());
    });

    model.push(1);
    model.insert(0, 9);
    model.extend(vec![2, 3]);

    assert_eq!(
        *seen.borrow(),
        vec![
            ListChange::Insert {
                index: 0,
                items: vec![1]
            },
            ListChange::Insert {
                index: 0,
                items: vec![9]
            },
            ListChange::Insert {
                index: 2,
                items: vec![2, 3]
            },
        ]
    );
    assert_eq!(model.snapshot(), vec![9, 1, 2, 3]);
}

#[test]
fn remove_and_set_emit_precise_changes() {
    let model = ListModel::from(vec![1, 2, 3, 4]);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = model.changes().observe(move |change| {
        seen_rc.borrow_mut().push(change.clone());
    });

    assert!(model.remove(1));
    assert!(!model.remove(10)); // out of range: no change
    assert!(model.set(0, 42));
    assert!(!model.set(99, 42));
    model.remove_range(0, 2);

    assert_eq!(
        *seen.borrow(),
        vec![
            ListChange::Remove { index: 1, len: 1 },
            ListChange::Update {
                index: 0,
                items: vec![42]
            },
            ListChange::Remove { index: 0, len: 2 },
        ]
    );
    assert_eq!(model.snapshot(), vec![4]);
}

#[test]
fn clear_and_reset_are_incremental() {
    let model = ListModel::from(vec![1, 2]);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = model.changes().observe(move |change| {
        seen_rc.borrow_mut().push(change.clone());
    });

    model.clear();
    model.clear(); // already empty: no event
    model.replace_all(vec![7, 8, 9]);
    model.clear();

    assert_eq!(
        *seen.borrow(),
        vec![
            ListChange::Remove { index: 0, len: 2 },
            ListChange::Reset {
                items: vec![7, 8, 9]
            },
            ListChange::Remove { index: 0, len: 3 },
        ]
    );
    assert!(model.is_empty());
}

#[test]
fn get_and_snapshot_reflect_latest_state() {
    let model = ListModel::from(vec![10, 20, 30]);
    assert_eq!(model.get(1), Some(20));
    assert_eq!(model.get(99), None);
    model.set(2, 99);
    assert_eq!(model.snapshot(), vec![10, 20, 99]);
    assert_eq!(model.len(), 3);
}

#[test]
fn sort_emits_reset_with_ordered_rows() {
    let model = ListModel::from(vec![3, 1, 2]);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = model.changes().observe(move |change| {
        seen_rc.borrow_mut().push(change.clone());
    });

    model.sort_by(|a, b| a.cmp(b));
    assert_eq!(
        *seen.borrow(),
        vec![ListChange::Reset {
            items: vec![1, 2, 3]
        }]
    );
    assert_eq!(model.snapshot(), vec![1, 2, 3]);
}

#[test]
fn retain_removes_runs_incrementally() {
    let model = ListModel::from(vec![1, 2, 3, 4, 5, 6]);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_rc = seen.clone();
    let _sub = model.changes().observe(move |change| {
        seen_rc.borrow_mut().push(change.clone());
    });

    model.retain(|x| x % 2 == 1); // keep 1, 3, 5
    assert_eq!(model.snapshot(), vec![1, 3, 5]);
    assert_eq!(
        *seen.borrow(),
        vec![
            ListChange::Remove { index: 5, len: 1 }, // 6
            ListChange::Remove { index: 3, len: 1 }, // 4
            ListChange::Remove { index: 1, len: 1 }, // 2
        ]
    );
}

#[test]
fn large_table_updates_are_incremental() {
    let model = ListModel::new();
    let events = std::rc::Rc::new(std::cell::RefCell::new(0usize));
    let events_rc = events.clone();
    let _sub = model.changes().observe(move |_| {
        *events_rc.borrow_mut() += 1;
    });

    let rows: Vec<String> = (0..10_000).map(|i| format!("row-{i}")).collect();
    model.replace_all(rows);
    for i in 0..1_000 {
        model.set(i, format!("updated-{i}"));
    }

    // One Reset for the bulk load, then per-row Update events: the widget tree
    // is never rebuilt for ordinary edits.
    assert_eq!(model.len(), 10_000);
    assert_eq!(*events.borrow(), 1_001);
    assert_eq!(model.get(999), Some(String::from("updated-999")));
}
