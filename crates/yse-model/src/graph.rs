//! Internal reactive graph: nodes, transactions, and glitch-free propagation.

use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostics::DiagnosticEvent;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NodeKind {
    Signal,
    Stream,
}

/// Shared per-node state used by the propagation machinery.
pub(crate) struct NodeCommon {
    kind: NodeKind,
    id: u64,
    rank: Cell<usize>,
    parents: RefCell<Vec<Weak<dyn Node>>>,
    dependents: RefCell<Vec<Rc<dyn Node>>>,
    scheduled: Cell<bool>,
    incoming: RefCell<Vec<Rc<dyn Any>>>,
    ever_observed: Cell<bool>,
}

impl NodeCommon {
    pub fn new(kind: NodeKind) -> Self {
        static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            kind,
            id: NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed),
            rank: Cell::new(0),
            parents: RefCell::new(Vec::new()),
            dependents: RefCell::new(Vec::new()),
            scheduled: Cell::new(false),
            incoming: RefCell::new(Vec::new()),
            ever_observed: Cell::new(false),
        }
    }
}

/// A node in the reactive graph.
pub(crate) trait Node {
    fn common(&self) -> &NodeCommon;

    fn kind(&self) -> NodeKind {
        self.common().kind
    }

    fn id(&self) -> u64 {
        self.common().id
    }

    fn rank(&self) -> usize {
        self.common().rank.get()
    }

    fn set_rank(&self, rank: usize) {
        self.common().rank.set(rank);
    }

    fn parents(&self) -> Ref<'_, Vec<Weak<dyn Node>>> {
        self.common().parents.borrow()
    }

    fn dependents(&self) -> Ref<'_, Vec<Rc<dyn Node>>> {
        self.common().dependents.borrow()
    }

    fn is_scheduled(&self) -> bool {
        self.common().scheduled.get()
    }

    fn set_scheduled(&self, scheduled: bool) {
        self.common().scheduled.set(scheduled);
    }

    fn push_incoming(&self, value: Rc<dyn Any>) {
        self.common().incoming.borrow_mut().push(value);
    }

    fn take_incoming(&self) -> Vec<Rc<dyn Any>> {
        std::mem::take(&mut *self.common().incoming.borrow_mut())
    }

    /// Current value for signal nodes, `None` for stream nodes.
    fn current_any(&self) -> Option<Rc<dyn Any>> {
        None
    }

    /// Whether this node is a terminal observer (always scheduled, never pruned).
    fn is_observer(&self) -> bool {
        false
    }

    /// Whether this stream derives exclusively from signal state (used for
    /// restart re-synchronisation).
    fn is_signal_backed(&self) -> bool {
        false
    }

    fn ever_observed(&self) -> bool {
        self.common().ever_observed.get()
    }

    fn mark_observed(&self) {
        self.common().ever_observed.set(true);
    }

    /// Recompute the current output of a signal-backed stream chain from the
    /// current signal state, without emitting through the graph.
    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        None
    }

    fn apply_write(&self, _value: Rc<dyn Any>) {
        panic!("this observable is not writable");
    }

    fn deactivate(&self) {}

    /// Invoke the observer callback (observer nodes only).
    fn fire(&self, _value: Rc<dyn Any>) {}

    /// Recompute a signal node's value from its parents (signal nodes only).
    fn recompute(&self) -> bool {
        false
    }

    /// Process pending events / parent changes during a transaction and
    /// return the outputs that should be delivered to dependents.
    fn process(&self, tx: &TxState) -> Vec<Rc<dyn Any>>;

    fn add_parent(&self, parent: &Rc<dyn Node>) {
        self.common()
            .parents
            .borrow_mut()
            .push(Rc::downgrade(parent));
    }

    fn remove_dependent(&self, child: &Rc<dyn Node>) {
        self.common()
            .dependents
            .borrow_mut()
            .retain(|dep| !Rc::ptr_eq(dep, child));
    }

    /// Receive an event forwarded from a `flat_map_switch` inner stream.
    fn forward_incoming(&self, _value: Rc<dyn Any>) {
        panic!("node does not accept forwarded events");
    }
}

/// A write performed during propagation, applied after the current pass.
pub(crate) struct DeferredWrite {
    pub node: Rc<dyn Node>,
    pub value: Rc<dyn Any>,
}

/// Per-thread transaction state.
pub(crate) struct TxState {
    pub id: u64,
    pub seq: Cell<u64>,
    pub work: RefCell<Vec<(u64, Rc<dyn Node>)>>,
    pub deferred: RefCell<Vec<DeferredWrite>>,
    pub fires: RefCell<Vec<(Weak<dyn Node>, Rc<dyn Any>)>>,
    pub panic: RefCell<Option<Box<dyn Any + Send>>>,
    pub propagating: Cell<bool>,
    pub processed: Cell<usize>,
    pub fired: Cell<usize>,
    pub deferred_count: Cell<usize>,
}

impl TxState {
    pub fn new() -> Self {
        static NEXT_TX_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_TX_ID.fetch_add(1, Ordering::Relaxed),
            seq: Cell::new(0),
            work: RefCell::new(Vec::new()),
            deferred: RefCell::new(Vec::new()),
            fires: RefCell::new(Vec::new()),
            panic: RefCell::new(None),
            propagating: Cell::new(false),
            processed: Cell::new(0),
            fired: Cell::new(0),
            deferred_count: Cell::new(0),
        }
    }
}

thread_local! {
    static TX: RefCell<Option<Rc<TxState>>> = const { RefCell::new(None) };
}

fn tx() -> Option<Rc<TxState>> {
    TX.with(|t| t.borrow().clone())
}

/// Queue a node for processing in the current transaction, if any.
pub(crate) fn schedule_node(node: Rc<dyn Node>) {
    if let Some(tx) = tx() {
        schedule_into(&tx, node);
    }
}

fn schedule_into(tx: &TxState, node: Rc<dyn Node>) {
    if node.is_scheduled() {
        return;
    }
    node.set_scheduled(true);
    let seq = tx.seq.get();
    tx.seq.set(seq + 1);
    tx.work.borrow_mut().push((seq, node));
}

/// Run `f` inside a single transaction. Nested calls join the outer transaction.
///
/// Writes applied inside the block become visible immediately to readers, and
/// are propagated once when the block ends. Writes performed during
/// propagation (from inside observers) are queued and applied in FIFO order
/// after the current pass.
pub fn transaction<R>(f: impl FnOnce() -> R) -> R {
    if tx().is_some() {
        return f();
    }

    let tx_rc = Rc::new(TxState::new());
    TX.with(|t| *t.borrow_mut() = Some(tx_rc.clone()));
    crate::diagnostics::Diagnostics::emit(DiagnosticEvent::TransactionStarted { id: tx_rc.id });

    let result = catch_unwind(AssertUnwindSafe(f));
    if result.is_ok() {
        tx_rc.propagating.set(true);
        loop {
            process_work(&tx_rc);
            apply_deferred(&tx_rc);
            if tx_rc.work.borrow().is_empty() && tx_rc.deferred.borrow().is_empty() {
                deliver_fires(&tx_rc);
                if tx_rc.deferred.borrow().is_empty() {
                    break;
                }
            }
        }
    }

    TX.with(|t| *t.borrow_mut() = None);
    crate::diagnostics::Diagnostics::emit(DiagnosticEvent::TransactionEnded {
        id: tx_rc.id,
        processed: tx_rc.processed.get(),
        fired: tx_rc.fired.get(),
        deferred: tx_rc.deferred_count.get(),
    });
    crate::diagnostics::Diagnostics::flush_global();

    match result {
        Ok(value) => {
            if let Some(panic) = tx_rc.panic.borrow_mut().take() {
                resume_unwind(panic);
            }
            value
        }
        Err(payload) => resume_unwind(payload),
    }
}

/// Write a value into a writable node (a `Var` or `Sink`).
pub(crate) fn write_node(node: Rc<dyn Node>, value: Rc<dyn Any>) {
    match tx() {
        Some(tx) if tx.propagating.get() => {
            crate::diagnostics::Diagnostics::emit(DiagnosticEvent::DeferredWriteQueued {
                transaction: tx.id,
            });
            tx.deferred_count.set(tx.deferred_count.get() + 1);
            tx.deferred.borrow_mut().push(DeferredWrite { node, value });
        }
        Some(tx) => {
            node.apply_write(value);
            schedule_into(&tx, node);
        }
        None => transaction(|| write_node(node, value)),
    }
}

/// Connect `child` as a dependent of `parent`, rejecting cycles.
pub(crate) fn connect(parent: Rc<dyn Node>, child: Rc<dyn Node>) {
    assert_no_cycle(&parent, &child);
    child.add_parent(&parent);
    parent.common().dependents.borrow_mut().push(child.clone());
    bubble_ranks(&child);
}

/// Panic if `target` is already an ancestor of `parent` (i.e. the edge would
/// create a cycle).
pub(crate) fn assert_no_cycle(parent: &Rc<dyn Node>, target: &Rc<dyn Node>) {
    let mut stack = vec![parent.clone()];
    let mut seen = HashSet::new();
    while let Some(node) = stack.pop() {
        if Rc::ptr_eq(&node, target) {
            crate::diagnostics::Diagnostics::emit(DiagnosticEvent::CycleRejected);
            panic!("reactive cycle detected: an observable cannot depend on itself");
        }
        let addr = Rc::as_ptr(&node) as *const () as usize;
        if !seen.insert(addr) {
            continue;
        }
        for ancestor in node.parents().iter().filter_map(|w| w.upgrade()) {
            stack.push(ancestor);
        }
    }
}

fn bubble_ranks(node: &Rc<dyn Node>) {
    let new_rank = node
        .parents()
        .iter()
        .filter_map(|w| w.upgrade())
        .map(|p| p.rank())
        .max()
        .map_or(0, |rank| rank + 1);
    if new_rank != node.rank() {
        node.set_rank(new_rank);
        let dependents: Vec<Rc<dyn Node>> = node.dependents().iter().cloned().collect();
        for dependent in dependents {
            bubble_ranks(&dependent);
        }
    }
}

fn process_work(tx: &TxState) {
    loop {
        let next = {
            let mut work = tx.work.borrow_mut();
            let index = work
                .iter()
                .enumerate()
                .min_by_key(|(_, (seq, node))| (node.rank(), *seq))
                .map(|(index, _)| index);
            index.map(|index| work.swap_remove(index))
        };
        let Some((_, node)) = next else { break };
        node.set_scheduled(false);
        tx.processed.set(tx.processed.get() + 1);
        crate::diagnostics::Diagnostics::emit(DiagnosticEvent::NodeProcessed {
            node: node.id(),
            rank: node.rank(),
            signal: node.kind() == NodeKind::Signal,
        });
        let outputs = node.process(tx);
        deliver(tx, &node, outputs);
    }
}

fn deliver(tx: &TxState, parent: &Rc<dyn Node>, outputs: Vec<Rc<dyn Any>>) {
    if outputs.is_empty() {
        return;
    }
    let dependents: Vec<Rc<dyn Node>> = parent.dependents().iter().cloned().collect();
    for child in dependents {
        // Prune streams that have no observers downstream: they are stopped.
        if !child.is_observer() && child.kind() == NodeKind::Stream && child.dependents().is_empty()
        {
            continue;
        }
        match parent.kind() {
            NodeKind::Signal => schedule_into(tx, child),
            NodeKind::Stream => {
                for output in &outputs {
                    child.push_incoming(output.clone());
                }
                schedule_into(tx, child);
            }
        }
    }
}

fn apply_deferred(tx: &TxState) {
    let writes: Vec<DeferredWrite> = std::mem::take(&mut *tx.deferred.borrow_mut());
    for write in writes {
        write.node.apply_write(write.value);
        schedule_into(tx, write.node.clone());
    }
}

fn deliver_fires(tx: &TxState) {
    let fires: Vec<(Weak<dyn Node>, Rc<dyn Any>)> = std::mem::take(&mut *tx.fires.borrow_mut());
    for (weak, value) in fires {
        let Some(node) = weak.upgrade() else { continue };
        tx.fired.set(tx.fired.get() + 1);
        crate::diagnostics::Diagnostics::emit(DiagnosticEvent::ObserverFired {
            observer: node.id(),
        });
        let result = catch_unwind(AssertUnwindSafe(|| node.fire(value)));
        if let Err(payload) = result
            && tx.panic.borrow().is_none()
        {
            *tx.panic.borrow_mut() = Some(payload);
        }
    }
}

/// Recompute every signal ancestor of `node` in topological order, so that a
/// restarted observation sees fully current state.
pub(crate) fn resync_signal_ancestors(node: &Rc<dyn Node>) {
    let mut signals = Vec::new();
    let mut stack = vec![node.clone()];
    let mut seen = HashSet::new();
    while let Some(current) = stack.pop() {
        let addr = Rc::as_ptr(&current) as *const () as usize;
        if !seen.insert(addr) {
            continue;
        }
        if current.kind() == NodeKind::Signal {
            signals.push(current.clone());
        }
        for parent in current.parents().iter().filter_map(|w| w.upgrade()) {
            stack.push(parent);
        }
    }
    signals.sort_by_key(|node| node.rank());
    for signal in signals {
        signal.recompute();
    }
}

/// Handle to an active observation. Dropping it unsubscribes.
pub struct Subscription {
    pub(crate) node: Rc<dyn Node>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.node.deactivate();
        let parents: Vec<Rc<dyn Node>> = self
            .node
            .parents()
            .iter()
            .filter_map(|w| w.upgrade())
            .collect();
        for parent in parents {
            parent.remove_dependent(&self.node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::{MapSignalNode, SignalSourceNode};

    #[test]
    fn cycles_are_rejected() {
        let diagnostics = crate::diagnostics::Diagnostics::install();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let seen_rc = seen.clone();
        let _sub = diagnostics
            .events()
            .observe(move |event| seen_rc.borrow_mut().push(event.clone()));

        let a: Rc<dyn Node> = Rc::new(SignalSourceNode::new(1i32));
        let b: Rc<dyn Node> = Rc::new(MapSignalNode::new(
            a.clone(),
            Box::new(|value: &i32| value + 1),
        ));
        connect(a.clone(), b.clone());
        let c: Rc<dyn Node> = Rc::new(MapSignalNode::new(
            b.clone(),
            Box::new(|value: &i32| value + 1),
        ));
        connect(b.clone(), c.clone());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            connect(c.clone(), a.clone())
        }));
        assert!(result.is_err(), "connecting a cycle must panic");
        diagnostics.flush();
        assert!(
            seen.borrow()
                .iter()
                .any(|e| matches!(e, crate::diagnostics::DiagnosticEvent::CycleRejected)),
            "cycle rejection must be reported by diagnostics"
        );
        diagnostics.uninstall();
    }
}
