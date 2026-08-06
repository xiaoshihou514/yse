use std::cell::RefCell;
use std::rc::Rc;
use yse_model::*;

fn collect<T>() -> Rc<RefCell<Vec<T>>> {
    Rc::new(RefCell::new(Vec::new()))
}

#[test]
fn signal_observers_receive_current_value_and_changes() {
    let v = Var::new(0);
    let s = v.signal().map(|x| x + 10);
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = s.observe(move |x| seen_rc.borrow_mut().push(*x));

    assert_eq!(*seen.borrow(), vec![10]);
    v.set(1);
    v.set(1); // no change: nothing is emitted
    v.set(2);
    assert_eq!(*seen.borrow(), vec![10, 11, 12]);
}

#[test]
fn diamond_graph_is_glitch_free() {
    let a = Var::new(1);
    let b = a.signal().map(|x| x + 1);
    let c = a.signal().map(|x| x * 10);
    let computed = collect();
    let computed_rc = computed.clone();
    let d = b.combine(&c, move |x, y| {
        // Record every pair of inputs this derived signal ever computes.
        computed_rc.borrow_mut().push((*x, *y));
        x + y
    });
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = d.observe(move |v| seen_rc.borrow_mut().push(*v));

    assert_eq!(*d.value(), 12);
    transaction(|| a.set(2));

    // d must never have computed from a mixed (b new, c old) or vice versa.
    assert_eq!(*computed.borrow(), vec![(2, 10), (3, 20)]);
    assert_eq!(*seen.borrow(), vec![12, 23]);
}

#[test]
fn observers_read_consistent_sibling_state() {
    let a = Var::new(1);
    let b = a.signal().map(|x| x + 1);
    let c = a.signal().map(|x| x * 10);
    let reads = collect();
    let reads_rc = reads.clone();
    let _sub = b.observe(move |v| reads_rc.borrow_mut().push((*v, *c.value())));

    a.set(2);
    // The b-observer always sees c already settled: never (3, 10) or (2, 20).
    assert_eq!(*reads.borrow(), vec![(2, 10), (3, 20)]);
}

#[test]
fn transaction_batches_multiple_writes() {
    let v = Var::new(0);
    let s = v.signal();
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = s.observe(move |x| seen_rc.borrow_mut().push(*x));

    transaction(|| {
        v.set(1);
        assert_eq!(*v.value(), 1); // writes are immediately readable
        v.set(2);
        assert_eq!(*v.value(), 2);
    });

    // Observers see only the final value of the transaction.
    assert_eq!(*seen.borrow(), vec![0, 2]);
}

#[test]
fn writes_during_propagation_are_queued_fifo() {
    let v = Var::new(0);
    let s = v.signal();
    let seen = collect();
    let seen_rc = seen.clone();
    let v_closure = v.clone();
    let _sub = s.observe(move |x| {
        seen_rc.borrow_mut().push(*x);
        if *x == 1 {
            v_closure.set(2);
        }
    });

    v.set(1);
    // The re-entrant write is applied after the current pass, never corrupting
    // propagation; the observer still receives the follow-up value.
    assert_eq!(*seen.borrow(), vec![0, 1, 2]);
}

#[test]
fn streams_start_and_stop_with_observers() {
    let sink = Sink::new();
    let mapped = sink.stream().map(|x| x * 2);
    let seen = collect();
    let seen_rc = seen.clone();
    let sub = mapped.observe(move |x| seen_rc.borrow_mut().push(*x));
    sink.send(1);
    drop(sub);

    // No observers: the derived stream is stopped and drops events.
    sink.send(2);
    assert_eq!(*seen.borrow(), vec![2]);

    let seen_rc = seen.clone();
    let sub2 = mapped.observe(move |x| seen_rc.borrow_mut().push(*x));
    sink.send(3);
    drop(sub2);
    sink.send(4);

    // Streams do not replay old events on restart.
    assert_eq!(*seen.borrow(), vec![2, 6]);
}

#[test]
fn owner_kills_all_subscriptions() {
    let mut owner = Owner::new();
    let v = Var::new(0);
    let sink = Sink::new();
    let seen = collect();
    let seen_rc = seen.clone();
    v.signal()
        .observe(move |x| seen_rc.borrow_mut().push(*x))
        .owned_by(&mut owner);
    let seen_rc = seen.clone();
    sink.stream()
        .observe(move |x| seen_rc.borrow_mut().push(*x))
        .owned_by(&mut owner);
    v.set(1);
    sink.send(10);
    assert_eq!(*seen.borrow(), vec![0, 1, 10]);

    drop(owner);
    v.set(2);
    sink.send(20);
    assert_eq!(*seen.borrow(), vec![0, 1, 10]);
}

#[test]
fn restarted_signals_resync_to_current_value() {
    let v = Var::new(0);
    let s = v.signal().map(|x| x + 10);
    let seen = collect();
    let seen_rc = seen.clone();
    let sub = s.observe(move |x| seen_rc.borrow_mut().push(*x));
    assert_eq!(*seen.borrow(), vec![10]);
    v.set(1);
    drop(sub);
    v.set(2);
    assert_eq!(*seen.borrow(), vec![10, 11]);

    let seen_rc = seen.clone();
    let sub2 = s.observe(move |x| seen_rc.borrow_mut().push(*x));
    assert_eq!(*seen.borrow(), vec![10, 11, 12]); // restart delivers the current value
    v.set(3);
    drop(sub2);
    assert_eq!(*seen.borrow(), vec![10, 11, 12, 13]);
}

#[test]
fn restarted_signal_streams_resync_to_current_value() {
    let v = Var::new(1);
    let ch = v.signal().changes().map(|x| x * 10);
    let seen = collect();
    let seen_rc = seen.clone();
    let sub = ch.observe(move |x| seen_rc.borrow_mut().push(*x));
    assert!(seen.borrow().is_empty()); // first observation: no replay
    v.set(2);
    assert_eq!(*seen.borrow(), vec![20]);

    drop(sub);
    v.set(3);
    assert_eq!(*seen.borrow(), vec![20]);

    let seen_rc = seen.clone();
    let sub2 = ch.observe(move |x| seen_rc.borrow_mut().push(*x));
    assert_eq!(*seen.borrow(), vec![20, 30]); // restart re-synchronises
    v.set(4);
    drop(sub2);
    assert_eq!(*seen.borrow(), vec![20, 30, 40]);
}

#[test]
fn observer_panic_does_not_corrupt_transactions() {
    let v = Var::new(0);
    let s = v.signal();
    let ok = collect();
    let _panicky = s.observe(move |x| {
        if *x == 1 {
            panic!("observer boom");
        }
    });
    let ok_rc = ok.clone();
    let _healthy = s.observe(move |x| ok_rc.borrow_mut().push(*x));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| v.set(1)));
    assert!(
        result.is_err(),
        "the observer panic propagates after the transaction"
    );

    // The transaction machinery is still usable and other observers still fire.
    v.set(2);
    assert_eq!(*ok.borrow(), vec![0, 1, 2]);
}

#[test]
fn operators_map_filter_filter_map_fold() {
    let sink = Sink::new();
    let filtered_mapped = sink.stream().filter(|x| x % 2 == 0).map(|x| x * 10);
    let folded = sink.stream().fold(0, |acc, x| acc + x);
    let filter_mapped = sink
        .stream()
        .filter_map(|x| if *x > 0 { Some(*x * 2) } else { None });

    let seen = collect();
    let folded_seen = collect();
    let fm_seen = collect();
    let seen_rc = seen.clone();
    let _sub_seen = filtered_mapped.observe(move |x| seen_rc.borrow_mut().push(*x));
    let folded_rc = folded_seen.clone();
    let _sub_folded = folded.observe(move |x| folded_rc.borrow_mut().push(*x));
    let fm_rc = fm_seen.clone();
    let _sub_fm = filter_mapped.observe(move |x| fm_rc.borrow_mut().push(*x));

    sink.send(1);
    sink.send(2);
    sink.send(3);
    sink.send(4);

    assert_eq!(*seen.borrow(), vec![20, 40]);
    assert_eq!(*folded_seen.borrow(), vec![1, 3, 6, 10]);
    assert_eq!(*fm_seen.borrow(), vec![2, 4, 6, 8]);
}

#[test]
fn operators_merge_distinct_sample_start_with() {
    let a = Sink::new();
    let b = Sink::new();
    let merged = a.stream().merge(&b.stream());
    let distinct = a.stream().distinct();
    let v = Var::new(100);
    let sampled = a.stream().sample(&v.signal());
    let started = b.stream().start_with(5);

    let m = collect();
    let d = collect();
    let sp = collect();
    let st = collect();
    let m_rc = m.clone();
    let _sub_m = merged.observe(move |x| m_rc.borrow_mut().push(*x));
    let d_rc = d.clone();
    let _sub_d = distinct.observe(move |x| d_rc.borrow_mut().push(*x));
    let sp_rc = sp.clone();
    let _sub_sp = sampled.observe(move |x| sp_rc.borrow_mut().push(*x));
    let st_rc = st.clone();
    let _sub_st = started.observe(move |x| st_rc.borrow_mut().push(*x));

    assert_eq!(*st.borrow(), vec![5]);
    a.send(1);
    b.send(2);
    a.send(1);
    a.send(3);
    assert_eq!(*m.borrow(), vec![1, 2, 1, 3]);
    assert_eq!(*d.borrow(), vec![1, 3]);
    assert_eq!(*sp.borrow(), vec![100, 100, 100]);

    v.set(200); // signal changes alone do not sample
    a.send(9);
    assert_eq!(*sp.borrow(), vec![100, 100, 100, 200]);
    b.send(7);
    assert_eq!(*st.borrow(), vec![5, 2, 7]);
}

#[test]
fn combine_arity() {
    let a = Var::new(1);
    let b = Var::new(2);
    let c = Var::new(3);
    let d = Var::new(4);
    let s2 = a.signal().combine(&b.signal(), |x, y| x + y);
    let s3 = a
        .signal()
        .combine3(&b.signal(), &c.signal(), |x, y, z| x + y + z);
    let s4 = a
        .signal()
        .combine4(&b.signal(), &c.signal(), &d.signal(), |w, x, y, z| {
            w + x + y + z
        });
    assert_eq!(*s2.value(), 3);
    assert_eq!(*s3.value(), 6);
    assert_eq!(*s4.value(), 10);

    transaction(|| {
        a.set(10);
        b.set(20);
    });
    assert_eq!(*s2.value(), 30);
    assert_eq!(*s3.value(), 33);
    assert_eq!(*s4.value(), 37);
}

#[test]
fn flat_map_switch_emits_only_the_current_inner_stream() {
    let outer = Sink::new();
    let inner1 = Sink::new();
    let inner2 = Sink::new();
    let inner1_stream = inner1.stream();
    let inner2_stream = inner2.stream();
    let switched = outer.stream().flat_map_switch(move |n| {
        if *n == 1 {
            inner1_stream.clone()
        } else {
            inner2_stream.clone()
        }
    });
    let seen = collect();
    let seen_rc = seen.clone();
    let _sub = switched.observe(move |x| seen_rc.borrow_mut().push(*x));

    outer.send(1); // switch to inner1
    inner1.send(10);
    outer.send(2); // switch to inner2; inner1 detaches
    inner1.send(11);
    inner2.send(20);

    assert_eq!(*seen.borrow(), vec![10, 20]);
}

#[test]
fn shared_observer_receives_from_multiple_streams() {
    let sink = Sink::new();
    let seen = collect();
    let seen_rc = seen.clone();
    let observer = Observer::new(move |x: &i32| seen_rc.borrow_mut().push(*x));
    let s1 = sink.stream().map(|x| x * 2);
    let s2 = sink.stream().map(|x| x * 3);
    let _a = s1.observe_with(&observer);
    let _b = s2.observe_with(&observer);

    sink.send(1);
    observer.on_next(&99);
    assert_eq!(*seen.borrow(), vec![2, 3, 99]);
}

#[test]
fn observer_count_tracks_attached_observers() {
    let v = Var::new(0);
    let s = v.signal();
    assert_eq!(s.observer_count(), 0);

    let sub = s.observe(|_| {});
    assert_eq!(s.observer_count(), 1);

    drop(sub);
    assert_eq!(s.observer_count(), 0);
}
