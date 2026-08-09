//! The facade re-exports both layers' public APIs under one `use yse::*`.

#[test]
fn facade_exposes_model_and_ui() {
    use yse::*;

    // Pure model: Var, Signal, transactions, lists, undo.
    let value = Var::new(0);
    let doubled = value.signal().map(|v| v * 2);
    transaction(|| value.set(21));
    assert_eq!(*doubled.value(), 42);

    let list = ListModel::from(vec![1, 2, 3]);
    assert_eq!(list.len(), 3);

    let stack = UndoStack::new();
    assert!(!*stack.can_undo().value());

    // UI types are reachable through the facade without linking Qt yet.
    let _app: Option<Application> = None;
    let _ = std::any::type_name::<Window>();
    let _ = std::any::type_name::<StringTableModel>();
}

#[test]
fn prelude_exposes_normal_application_surface() {
    use yse::prelude::*;

    let value = Var::new(String::from("ready"));
    let _window: Option<Window> = None;
    assert_eq!(value.value().as_str(), "ready");
}
