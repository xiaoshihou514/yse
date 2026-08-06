//! Counter with derived validation and event sampling.
//!
//! Demonstrates that the public API can express counter state, derived state,
//! validation messages, and event sampling without any interior-mutability
//! boilerplate at the call site.

use std::cell::RefCell;
use std::rc::Rc;
use yse_model::*;

fn main() {
    // Counter state.
    let count = Var::new(0);

    // Derived state: a doubled value and a validation message.
    let doubled = count.signal().map(|c| c * 2);
    let validation = count.signal().map(|c| {
        if *c < 0 {
            "negative"
        } else if *c > 10 {
            "too large"
        } else {
            "ok"
        }
    });

    // Event sampling: what was the count when a "click" happened?
    let clicks = Sink::new();
    let clicked_value = clicks.stream().sample(&count.signal());

    let log: Rc<RefCell<Vec<(i32, &str)>>> = Rc::new(RefCell::new(Vec::new()));
    let log_rc = log.clone();
    let count_rc = count.clone();
    let validation_rc = validation.clone();
    let _count_sub = count_rc.signal().observe(move |value| {
        log_rc.borrow_mut().push((*value, *validation_rc.value()));
    });
    let log_rc = log.clone();
    let _clicked_sub = clicked_value.observe(move |value| {
        log_rc.borrow_mut().push((*value, "sampled"));
    });

    count.set(5);
    clicks.send(());
    count.set(12);

    assert_eq!(*doubled.value(), 24);
    assert_eq!(*validation.value(), "too large");
    assert_eq!(
        *log.borrow(),
        vec![(0, "ok"), (5, "ok"), (5, "sampled"), (12, "too large")]
    );
    println!(
        "counter={} doubled={} validation={}",
        count.value(),
        doubled.value(),
        validation.value()
    );
}
