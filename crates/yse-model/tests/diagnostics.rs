use std::cell::RefCell;
use std::rc::Rc;
use yse_model::*;

fn collect<T>() -> Rc<RefCell<Vec<T>>> {
    Rc::new(RefCell::new(Vec::new()))
}

#[test]
fn diagnostics_explain_a_transaction() {
    let diagnostics = Diagnostics::install();
    let events = collect();
    let events_rc = events.clone();
    let _sub = diagnostics
        .events()
        .observe(move |event| events_rc.borrow_mut().push(event.clone()));

    let v = Var::new(1);
    let b = v.signal().map(|x| x + 1);
    let c = v.signal().map(|x| x * 10);
    let d = b.combine(&c, |x, y| x + y);
    let seen = collect();
    let seen_rc = seen.clone();
    let _observer = d.observe(move |value| seen_rc.borrow_mut().push(*value));

    v.set(2);

    let events = events.borrow();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DiagnosticEvent::TransactionStarted { .. }))
    );
    assert!(events.iter().any(
        |e| matches!(e, DiagnosticEvent::TransactionEnded { processed, .. } if *processed >= 3)
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DiagnosticEvent::NodeProcessed { signal: true, .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DiagnosticEvent::ObserverFired { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, DiagnosticEvent::CycleRejected))
    );

    diagnostics.uninstall();
}

#[test]
fn deferred_writes_are_reported() {
    let diagnostics = Diagnostics::install();
    let events = collect();
    let events_rc = events.clone();
    let _sub = diagnostics
        .events()
        .observe(move |event| events_rc.borrow_mut().push(event.clone()));

    let v = Var::new(0);
    let v_closure = v.clone();
    let _observer = v.signal().observe(move |x| {
        if *x == 1 {
            v_closure.set(2);
        }
    });
    v.set(1);

    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, DiagnosticEvent::DeferredWriteQueued { .. }))
    );
    diagnostics.uninstall();
}

#[test]
fn logger_filters_by_level() {
    let logger = Logger::install(LogLevel::Info);
    let records = collect();
    let records_rc = records.clone();
    let _sub = logger
        .records()
        .observe(move |record| records_rc.borrow_mut().push(record.clone()));

    logger.info("started");
    logger.debug("hidden detail");
    logger.warn("careful");
    logger.set_min_level(LogLevel::Debug);
    logger.debug("now visible");
    log_error("global error");

    let records = records.borrow();
    assert_eq!(
        records
            .iter()
            .map(|r| (r.level, r.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (LogLevel::Info, "started"),
            (LogLevel::Warn, "careful"),
            (LogLevel::Debug, "now visible"),
            (LogLevel::Error, "global error"),
        ]
    );
    logger.uninstall();
}
