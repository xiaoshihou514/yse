use yse_model::*;

struct CounterCommand {
    value: Var<i32>,
    delta: i32,
    label: String,
}

impl Command for CounterCommand {
    fn execute(&self) {
        let current = self.value.value();
        self.value.set(*current + self.delta);
    }

    fn undo(&self) {
        let current = self.value.value();
        self.value.set(*current - self.delta);
    }

    fn text(&self) -> String {
        self.label.clone()
    }
}

fn collect<T>() -> std::rc::Rc<std::cell::RefCell<Vec<T>>> {
    std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))
}

#[test]
fn push_undo_redo_cycle_restores_state() {
    let value = Var::new(0);
    let stack = UndoStack::new();

    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 5,
        label: String::new(),
    }));
    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 3,
        label: String::new(),
    }));
    assert_eq!(*value.value(), 8);

    assert!(stack.undo());
    assert_eq!(*value.value(), 5);
    assert!(stack.undo());
    assert_eq!(*value.value(), 0);
    assert!(!stack.undo()); // empty

    assert!(stack.redo());
    assert_eq!(*value.value(), 5);
    assert!(stack.redo());
    assert_eq!(*value.value(), 8);
    assert!(!stack.redo()); // empty
}

#[test]
fn can_undo_and_can_redo_follow_the_stack() {
    let value = Var::new(0);
    let stack = UndoStack::new();
    let undo_seen = collect();
    let redo_seen = collect();
    let undo_rc = undo_seen.clone();
    let _u = stack
        .can_undo()
        .observe(move |v| undo_rc.borrow_mut().push(*v));
    let redo_rc = redo_seen.clone();
    let _r = stack
        .can_redo()
        .observe(move |v| redo_rc.borrow_mut().push(*v));

    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 1,
        label: String::new(),
    }));
    assert_eq!(*undo_seen.borrow(), vec![false, true]); // initial + after push
    assert_eq!(*redo_seen.borrow(), vec![false]); // initial only

    stack.undo();
    assert_eq!(*undo_seen.borrow(), vec![false, true, false]);
    assert_eq!(*redo_seen.borrow(), vec![false, true]);

    stack.redo();
    assert_eq!(*undo_seen.borrow(), vec![false, true, false, true]);
    assert_eq!(*redo_seen.borrow(), vec![false, true, false]);

    stack.clear();
    assert_eq!(*undo_seen.borrow(), vec![false, true, false, true, false]);
    // can_redo was already false after the redo, so clear() emits nothing.
    assert_eq!(*redo_seen.borrow(), vec![false, true, false]);
}

#[test]
fn push_after_undo_clears_redo() {
    let value = Var::new(0);
    let stack = UndoStack::new();
    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 1,
        label: String::new(),
    }));
    stack.undo();
    assert!(*stack.can_redo().value());

    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 10,
        label: String::new(),
    }));
    assert!(!*stack.can_redo().value());
    assert!(!stack.redo());
}

#[test]
fn changed_stream_and_texts() {
    let value = Var::new(0);
    let stack = UndoStack::new();
    let events = collect();
    let events_rc = events.clone();
    let _sub = stack
        .changed()
        .observe(move |()| events_rc.borrow_mut().push(()));

    assert_eq!(stack.undo_text(), None);
    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 1,
        label: String::from("add 1"),
    }));
    stack.push(Box::new(CounterCommand {
        value: value.clone(),
        delta: 2,
        label: String::from("add 2"),
    }));
    assert_eq!(stack.undo_text(), Some(String::from("add 2")));
    stack.undo();
    assert_eq!(stack.undo_text(), Some(String::from("add 1")));
    assert_eq!(stack.redo_text(), Some(String::from("add 2")));
    stack.redo();
    stack.clear();
    assert_eq!(events.borrow().len(), 5); // push, push, undo, redo, clear
}
