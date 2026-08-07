use std::rc::Rc;
use std::time::Duration;
use yse_model::*;

fn collect<T>() -> Rc<std::cell::RefCell<Vec<T>>> {
    Rc::new(std::cell::RefCell::new(Vec::new()))
}

#[test]
fn debounce_emits_only_the_latest_event_after_the_window() {
    let timer = Rc::new(ManualTimer::new());
    let sink = Sink::new();
    let debounced = sink
        .stream()
        .debounce(timer.clone(), Duration::from_millis(50));
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = debounced.observe(move |v| seen_rc.borrow_mut().push(*v));

    sink.send(1);
    sink.send(2);
    sink.send(3);
    assert_eq!(timer.pending(), 3);

    // Stale timer firings are no-ops; only the last one emits.
    timer.fire_next();
    timer.fire_next();
    assert!(seen.borrow().is_empty());
    timer.fire_next();
    assert_eq!(*seen.borrow(), vec![3]);
}

#[test]
fn throttle_emits_leading_and_trailing_edges() {
    let timer = Rc::new(ManualTimer::new());
    let sink = Sink::new();
    let throttled = sink
        .stream()
        .throttle(timer.clone(), Duration::from_millis(50));
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = throttled.observe(move |v| seen_rc.borrow_mut().push(*v));

    sink.send(1); // leading edge, immediate
    assert_eq!(*seen.borrow(), vec![1]);
    assert_eq!(timer.pending(), 1);

    sink.send(2);
    sink.send(3); // coalesced into the trailing slot
    timer.fire_next();
    assert_eq!(*seen.borrow(), vec![1, 3]);

    // Gate reopens: the next event leads a fresh window.
    sink.send(4);
    assert_eq!(*seen.borrow(), vec![1, 3, 4]);
    timer.fire_next(); // window ends without trailing events
    assert_eq!(*seen.borrow(), vec![1, 3, 4]);
}

#[test]
fn delay_preserves_order() {
    let timer = Rc::new(ManualTimer::new());
    let sink = Sink::new();
    let delayed = sink
        .stream()
        .delay(timer.clone(), Duration::from_millis(50));
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = delayed.observe(move |v| seen_rc.borrow_mut().push(*v));

    sink.send(1);
    sink.send(2);
    assert_eq!(timer.pending(), 1);

    timer.fire_next();
    assert_eq!(*seen.borrow(), vec![1]);
    assert_eq!(timer.pending(), 1); // rescheduled for the next queued event
    timer.fire_next();
    assert_eq!(*seen.borrow(), vec![1, 2]);
}
