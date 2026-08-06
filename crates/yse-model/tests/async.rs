use std::sync::Arc;
use std::time::{Duration, Instant};
use yse_model::*;

fn collect<T>() -> std::rc::Rc<std::cell::RefCell<Vec<T>>> {
    std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))
}

fn wait_until_pending(scheduler: &QueueScheduler, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while scheduler.pending() < expected && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        scheduler.pending(),
        expected,
        "worker never scheduled delivery"
    );
}

#[test]
fn task_delivers_result_on_the_spawning_thread() {
    let scheduler = Arc::new(QueueScheduler::new());
    let task = spawn_task(scheduler.clone(), |_token| 6 * 7);
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = task
        .results()
        .observe(move |v| seen_rc.borrow_mut().push(*v));

    wait_until_pending(&scheduler, 1);
    scheduler.drain();
    assert_eq!(*seen.borrow(), vec![42]);

    scheduler.drain(); // nothing left
    assert_eq!(scheduler.pending(), 0);
}

#[test]
fn cancelled_task_never_delivers() {
    let scheduler = Arc::new(QueueScheduler::new());
    let task = spawn_task(scheduler.clone(), |token| {
        std::thread::sleep(Duration::from_millis(50));
        if token.is_cancelled() { 0 } else { 1 }
    });
    task.cancel();
    let delivered = collect();
    let delivered_rc = delivered.clone();
    let _sub = task
        .results()
        .observe(move |v| delivered_rc.borrow_mut().push(*v));

    std::thread::sleep(Duration::from_millis(150));
    assert!(task.is_cancelled());
    assert_eq!(
        scheduler.pending(),
        0,
        "cancelled work must not schedule delivery"
    );
    scheduler.drain();
    assert!(delivered.borrow().is_empty());
}

#[test]
fn dropped_task_never_delivers() {
    let scheduler = Arc::new(QueueScheduler::new());
    let task = spawn_task(scheduler.clone(), |_token| {
        std::thread::sleep(Duration::from_millis(50));
        42
    });
    let delivered = collect();
    let delivered_rc = delivered.clone();
    let _sub = task
        .results()
        .observe(move |v| delivered_rc.borrow_mut().push(*v));
    drop(task);

    std::thread::sleep(Duration::from_millis(150));
    scheduler.drain();
    assert!(delivered.borrow().is_empty());
}

#[test]
fn deliveries_are_fifo_in_completion_order() {
    let scheduler = Arc::new(QueueScheduler::new());
    let slow = spawn_task(scheduler.clone(), |_token| {
        std::thread::sleep(Duration::from_millis(30));
        ("slow", 1)
    });
    let fast = spawn_task(scheduler.clone(), |_token| {
        std::thread::sleep(Duration::from_millis(5));
        ("fast", 2)
    });
    let order = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let order_rc = order.clone();
    let _slow_sub = slow.results().observe(move |(name, value)| {
        order_rc.borrow_mut().push((*name, *value));
    });
    let order_rc = order.clone();
    let _fast_sub = fast.results().observe(move |(name, value)| {
        order_rc.borrow_mut().push((*name, *value));
    });

    wait_until_pending(&scheduler, 2);
    scheduler.drain();
    // The faster worker finishes first, so its delivery is queued first.
    assert_eq!(*order.borrow(), vec![("fast", 2), ("slow", 1)]);
}

#[test]
fn cooperative_cancellation_is_observable() {
    let scheduler = Arc::new(QueueScheduler::new());
    let task = spawn_task(scheduler.clone(), |token| {
        let mut ticks = 0;
        while !token.is_cancelled() && ticks < 100 {
            ticks += 1;
            std::thread::sleep(Duration::from_millis(2));
        }
        ticks
    });
    std::thread::sleep(Duration::from_millis(30));
    task.cancel();
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = task
        .results()
        .observe(move |v| seen_rc.borrow_mut().push(*v));

    std::thread::sleep(Duration::from_millis(200));
    assert!(task.is_cancelled());
    assert_eq!(scheduler.pending(), 0);
    assert!(seen.borrow().is_empty(), "cancelled work must not deliver");
}
