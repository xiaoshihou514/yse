//! `yse-model` is the pure-Rust reactive runtime at the core of Yse.
//!
//! The design is inspired by [Airstream](https://github.com/raquo/Airstream):
//!
//! - [`EventStream`] has no current value; it carries discrete events.
//! - [`Signal`] always has a current value; it carries state.
//! - Propagation is transactional and glitch-free: inside one transaction,
//!   derived values are recomputed in topological order, so a diamond-shaped
//!   graph never observes inconsistent intermediate state.
//! - Every observation is owned by a [`Subscription`]; dropping it (directly
//!   or through an [`Owner`]) unsubscribes.
//!
//! The crate is pure Rust: no Qt and no `unsafe`. The reactive graph itself is
//! single-threaded; background work must return to it through a [`Scheduler`]
//! (see [`spawn_task`]).

#![forbid(unsafe_code)]
// The typed operator nodes store nested closure types (e.g.
// `RefCell<Box<dyn FnMut(&A, &B) -> R>>`) by design; the complexity is
// inherent to the API, not accidental.
#![allow(clippy::type_complexity)]

mod diagnostics;
mod graph;
mod list;
mod nodes;
mod timing;
mod undo;

pub use diagnostics::{DiagnosticEvent, Diagnostics, LogLevel, LogRecord, Logger};
pub use diagnostics::{log, log_debug, log_error, log_info, log_warn};
use graph::{Node, connect, resync_signal_ancestors, write_node};
pub use graph::{Subscription, transaction};
pub use list::{ListChange, ListModel};
use nodes::{
    ChangesNode, Combine3Node, Combine4Node, CombineNode, DistinctSignalNode, DistinctStreamNode,
    FilterMapStreamNode, FilterStreamNode, FlatMapSwitchNode, FoldStreamNode, MapSignalNode,
    MapStreamNode, MergeStreamNode, ObserverNode, SampleStreamNode, SignalSourceNode,
    StartWithNode, StreamSourceNode,
};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
pub use timing::{ManualTimer, Timer};
pub use undo::{Command, UndoStack};

/// Executes deferred work. Yse's reactive graph is synchronous and
/// single-threaded; this trait is the boundary where background work returns
/// to the UI thread. A scheduler must run scheduled tasks on the thread that
/// owns the reactive graph (the UI thread in a Qt application).
pub trait Scheduler {
    /// Schedule `task` for execution.
    fn schedule(&self, task: Box<dyn FnOnce() + Send>);

    /// Schedule a closure, boxing it for you.
    fn schedule_fn<F>(&self, f: F)
    where
        Self: Sized,
        F: FnOnce() + Send + 'static,
    {
        self.schedule(Box::new(f));
    }
}

/// A scheduler that runs tasks immediately on the current thread.
///
/// This is only useful when scheduling happens on the graph's own thread; a
/// background worker must use a queue- or GUI-thread-based scheduler instead.
pub struct SyncScheduler;

impl Scheduler for SyncScheduler {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
        task();
    }
}

/// A scheduler that queues tasks and runs them on demand on the calling
/// thread. Useful for deterministic tests and for wiring background results
/// into the reactive graph.
pub struct QueueScheduler {
    queue: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

impl QueueScheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Number of tasks waiting to run.
    pub fn pending(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Run every queued task on the calling thread, in FIFO order.
    pub fn drain(&self) {
        let tasks = std::mem::take(&mut *self.queue.lock().unwrap());
        for task in tasks {
            task();
        }
    }
}

impl Default for QueueScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for QueueScheduler {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
        self.queue.lock().unwrap().push(task);
    }
}

/// A writable source of signal state.
pub struct Var<T> {
    node: Rc<SignalSourceNode<T>>,
}

impl<T> Clone for Var<T> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
        }
    }
}

impl<T> Var<T>
where
    T: 'static + PartialEq,
{
    /// Create a new `Var` holding `value`.
    pub fn new(value: T) -> Self {
        Self {
            node: Rc::new(SignalSourceNode::new(value)),
        }
    }

    /// Write `value`, propagating through the graph inside a transaction.
    pub fn set(&self, value: T) {
        let node: Rc<dyn Node> = self.node.clone();
        write_node(node, Rc::new(value));
    }

    /// The current value.
    pub fn value(&self) -> Rc<T> {
        self.node.value.borrow().clone()
    }

    /// This `Var` as a [`Signal`].
    pub fn signal(&self) -> Signal<T> {
        Signal::new(self.node.clone())
    }
}

/// A read-only signal with a current value.
pub struct Signal<T> {
    node: Rc<dyn Node>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self::new(self.node.clone())
    }
}

impl<T> Signal<T>
where
    T: 'static,
{
    fn new(node: Rc<dyn Node>) -> Self {
        Self {
            node,
            _marker: std::marker::PhantomData,
        }
    }

    /// The current value.
    pub fn value(&self) -> Rc<T> {
        self.node
            .current_any()
            .expect("signal has a value")
            .downcast::<T>()
            .expect("signal type mismatch")
    }

    /// Number of observers currently attached to this signal. Useful for
    /// lifecycle tests: dropping the owning component must return this to
    /// zero.
    pub fn observer_count(&self) -> usize {
        self.node
            .dependents()
            .iter()
            .filter(|dependent| dependent.is_observer())
            .count()
    }

    /// Observe this signal. The callback fires immediately with the current
    /// value and then on every change, in topological order.
    pub fn observe<F>(&self, f: F) -> Subscription
    where
        F: FnMut(&T) + 'static,
    {
        let callback: Rc<RefCell<dyn FnMut(&T)>> = Rc::new(RefCell::new(f));
        self.observe_callback(callback)
    }

    /// Observe this signal with a shared [`Observer`].
    pub fn observe_with(&self, observer: &Observer<T>) -> Subscription {
        self.observe_callback(observer.callback.clone())
    }

    fn observe_callback(&self, callback: Rc<RefCell<dyn FnMut(&T)>>) -> Subscription {
        let node =
            Rc::new_cyclic(|weak| ObserverNode::new(self.node.clone(), callback, weak.clone()));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        if let Some(value) = self.node.current_any() {
            node.fire(value);
        }
        Subscription { node: node_any }
    }

    /// Map this signal's value.
    pub fn map<U, F>(&self, f: F) -> Signal<U>
    where
        U: 'static + PartialEq,
        F: FnMut(&T) -> U + 'static,
    {
        let node = Rc::new(MapSignalNode::<T, U>::new(self.node.clone(), Box::new(f)));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        node.recompute();
        Signal::new(node_any)
    }

    /// Only propagate when the value actually changes.
    pub fn distinct(&self) -> Signal<T>
    where
        T: PartialEq,
    {
        let node = Rc::new(DistinctSignalNode::<T>::new(self.node.clone()));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        node.recompute();
        Signal::new(node_any)
    }

    /// Combine two signals. Recomputes only after every input has settled.
    pub fn combine<U, R, F>(&self, other: &Signal<U>, f: F) -> Signal<R>
    where
        U: 'static,
        R: 'static + PartialEq,
        F: FnMut(&T, &U) -> R + 'static,
    {
        let node = Rc::new(CombineNode::<T, U, R>::new(
            self.node.clone(),
            other.node.clone(),
            Box::new(f),
        ));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        connect(other.node.clone(), node_any.clone());
        node.recompute();
        Signal::new(node_any)
    }

    /// Combine three signals.
    pub fn combine3<U, V, R, F>(&self, b: &Signal<U>, c: &Signal<V>, f: F) -> Signal<R>
    where
        U: 'static,
        V: 'static,
        R: 'static + PartialEq,
        F: FnMut(&T, &U, &V) -> R + 'static,
    {
        let node = Rc::new(Combine3Node::<T, U, V, R>::new(
            self.node.clone(),
            b.node.clone(),
            c.node.clone(),
            Box::new(f),
        ));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        connect(b.node.clone(), node_any.clone());
        connect(c.node.clone(), node_any.clone());
        node.recompute();
        Signal::new(node_any)
    }

    /// Combine four signals.
    pub fn combine4<U, V, W, R, F>(
        &self,
        b: &Signal<U>,
        c: &Signal<V>,
        d: &Signal<W>,
        f: F,
    ) -> Signal<R>
    where
        U: 'static,
        V: 'static,
        W: 'static,
        R: 'static + PartialEq,
        F: FnMut(&T, &U, &V, &W) -> R + 'static,
    {
        let node = Rc::new(Combine4Node::<T, U, V, W, R>::new(
            self.node.clone(),
            b.node.clone(),
            c.node.clone(),
            d.node.clone(),
            Box::new(f),
        ));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        connect(b.node.clone(), node_any.clone());
        connect(c.node.clone(), node_any.clone());
        connect(d.node.clone(), node_any.clone());
        node.recompute();
        Signal::new(node_any)
    }

    /// A stream of change events: emits this signal's new value whenever it
    /// changes. Restarting an observation re-synchronises with the current
    /// value.
    pub fn changes(&self) -> EventStream<T> {
        let node = Rc::new(ChangesNode::<T>::new(self.node.clone()));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }
}

/// A writable source of stream events.
pub struct Sink<T> {
    node: Rc<StreamSourceNode<T>>,
}

impl<T> Clone for Sink<T> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
        }
    }
}

impl<T> Sink<T>
where
    T: 'static,
{
    /// Create a new `Sink`.
    pub fn new() -> Self {
        Self {
            node: Rc::new(StreamSourceNode::new()),
        }
    }

    /// Emit `value` to every current observer, inside a transaction.
    pub fn send(&self, value: T) {
        let node: Rc<dyn Node> = self.node.clone();
        write_node(node, Rc::new(value));
    }

    /// This `Sink` as an [`EventStream`].
    pub fn stream(&self) -> EventStream<T> {
        EventStream::new(self.node.clone())
    }
}

impl<T> Default for Sink<T>
where
    T: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// A stream of discrete events. It has no current value.
pub struct EventStream<T> {
    node: Rc<dyn Node>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> Clone for EventStream<T> {
    fn clone(&self) -> Self {
        Self::new(self.node.clone())
    }
}

impl<T> EventStream<T>
where
    T: 'static,
{
    fn new(node: Rc<dyn Node>) -> Self {
        Self {
            node,
            _marker: std::marker::PhantomData,
        }
    }

    /// Observe this stream. The callback fires for every event while the
    /// returned [`Subscription`] is alive. Streams are started by their first
    /// observer and stopped when the last observer is removed.
    pub fn observe<F>(&self, f: F) -> Subscription
    where
        F: FnMut(&T) + 'static,
    {
        let callback: Rc<RefCell<dyn FnMut(&T)>> = Rc::new(RefCell::new(f));
        self.observe_callback(callback)
    }

    /// Observe this stream with a shared [`Observer`].
    pub fn observe_with(&self, observer: &Observer<T>) -> Subscription {
        self.observe_callback(observer.callback.clone())
    }

    fn observe_callback(&self, callback: Rc<RefCell<dyn FnMut(&T)>>) -> Subscription {
        let node =
            Rc::new_cyclic(|weak| ObserverNode::new(self.node.clone(), callback, weak.clone()));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        if self.node.is_signal_backed() {
            resync_signal_ancestors(&self.node);
            if self.node.ever_observed()
                && let Some(value) = self.node.pull_output()
            {
                node.fire(value);
            }
            self.node.mark_observed();
        }
        Subscription { node: node_any }
    }

    /// Map every event.
    pub fn map<U, F>(&self, f: F) -> EventStream<U>
    where
        U: 'static,
        F: FnMut(&T) -> U + 'static,
    {
        let node = Rc::new(MapStreamNode::<T, U>::new(self.node.clone(), Box::new(f)));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Keep only events that satisfy the predicate.
    pub fn filter<F>(&self, f: F) -> EventStream<T>
    where
        F: FnMut(&T) -> bool + 'static,
    {
        let node = Rc::new(FilterStreamNode::<T>::new(self.node.clone(), Box::new(f)));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Map and filter in one step.
    pub fn filter_map<U, F>(&self, f: F) -> EventStream<U>
    where
        U: 'static,
        F: FnMut(&T) -> Option<U> + 'static,
    {
        let node = Rc::new(FilterMapStreamNode::<T, U>::new(
            self.node.clone(),
            Box::new(f),
        ));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Emit the running fold of every event.
    pub fn fold<U, F>(&self, init: U, f: F) -> EventStream<U>
    where
        U: 'static,
        F: FnMut(&U, &T) -> U + 'static,
    {
        let node = Rc::new(FoldStreamNode::<T, U>::new(init, Box::new(f)));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Drop consecutive duplicates.
    pub fn distinct(&self) -> EventStream<T>
    where
        T: PartialEq,
    {
        let node = Rc::new(DistinctStreamNode::<T>::new(self.node.clone()));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Merge events from two streams, preserving arrival order.
    pub fn merge(&self, other: &EventStream<T>) -> EventStream<T> {
        Self::merge_all(vec![self.clone(), other.clone()])
    }

    /// Merge events from many streams, preserving arrival order.
    pub fn merge_all(streams: Vec<EventStream<T>>) -> EventStream<T> {
        assert!(
            !streams.is_empty(),
            "merge_all requires at least one stream"
        );
        let node = Rc::new(MergeStreamNode::<T>::new());
        let node_any: Rc<dyn Node> = node.clone();
        for stream in streams {
            connect(stream.node.clone(), node_any.clone());
        }
        EventStream::new(node_any)
    }

    /// Emit the sampled signal's current value whenever this stream fires.
    pub fn sample<S>(&self, signal: &Signal<S>) -> EventStream<S>
    where
        S: 'static,
    {
        let node = Rc::new(SampleStreamNode::<T, S>::new(&signal.node));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Turn this stream into a signal that starts with `initial` and holds the
    /// latest event.
    pub fn start_with(&self, initial: T) -> Signal<T>
    where
        T: PartialEq,
    {
        let node = Rc::new(StartWithNode::new(initial));
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        Signal::new(node_any)
    }

    /// Switch to the latest inner stream produced by `f`; only the current
    /// inner stream's events are emitted.
    pub fn flat_map_switch<U, F>(&self, mut f: F) -> EventStream<U>
    where
        U: 'static,
        F: FnMut(&T) -> EventStream<U> + 'static,
    {
        let node = Rc::new_cyclic(|weak| {
            FlatMapSwitchNode::<T, U>::new(
                Box::new(move |value: &T| f(value).node.clone()),
                weak.clone(),
            )
        });
        let node_any: Rc<dyn Node> = node.clone();
        connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }
}

/// A reusable callback handle that can observe several observables at once.
pub struct Observer<T> {
    callback: Rc<RefCell<dyn FnMut(&T)>>,
}

impl<T> Observer<T> {
    /// Create an observer from a callback.
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(&T) + 'static,
    {
        Self {
            callback: Rc::new(RefCell::new(f)),
        }
    }

    /// Invoke the callback directly with `value`.
    pub fn on_next(&self, value: &T) {
        (self.callback.borrow_mut())(value);
    }
}

impl Subscription {
    /// Register this subscription with `owner`; dropping the owner unsubscribes.
    pub fn owned_by(self, owner: &mut Owner) {
        owner.add(self);
    }
}

/// Owns a set of subscriptions and unsubscribes all of them when dropped.
pub struct Owner {
    subscriptions: Vec<Subscription>,
}

impl Owner {
    /// Create an empty owner.
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    /// Take ownership of `subscription`.
    pub fn add(&mut self, subscription: Subscription) {
        self.subscriptions.push(subscription);
    }

    /// Take ownership of `subscription` and return `self` for chaining.
    pub fn track(&mut self, subscription: Subscription) -> &mut Self {
        self.subscriptions.push(subscription);
        self
    }

    /// Unsubscribe and drop every subscription, leaving the owner empty.
    pub fn clear(&mut self) {
        self.subscriptions.clear();
    }
}

impl Default for Owner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.subscriptions.clear();
    }
}

/// Cooperative cancellation token shared with a running [`Task`].
#[derive(Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Create an uncancelled token.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Request cancellation. Best-effort: the worker should check
    /// [`CancellationToken::is_cancelled`] regularly.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

type DeliverySlot = Box<dyn FnMut(Box<dyn Any + Send>)>;

thread_local! {
    static DELIVERY_SLOTS: RefCell<HashMap<u64, DeliverySlot>> = RefCell::new(HashMap::new());
}

/// Handle to a background task. The result is delivered to the thread that
/// spawned the task through the given [`Scheduler`]; subscribe with
/// [`Task::results`].
pub struct Task<T> {
    id: u64,
    token: CancellationToken,
    sink: Rc<Sink<T>>,
    handle: RefCell<Option<JoinHandle<()>>>,
}

impl<T> Task<T>
where
    T: 'static,
{
    /// Request cancellation of the background work.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// A stream of task results, delivered on the graph's thread.
    pub fn results(&self) -> EventStream<T> {
        self.sink.stream()
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        // Drop the delivery slot so a queued delivery is discarded safely if
        // the task is dropped before it runs.
        DELIVERY_SLOTS.with(|slots| {
            slots.borrow_mut().remove(&self.id);
        });
        self.token.cancel();
        // Detach the worker: it runs to completion, but cancellation is
        // cooperative and its result will not be delivered.
        self.handle.borrow_mut().take();
    }
}

/// Run `f` on a background thread and deliver its result onto the thread that
/// owns the reactive graph via `scheduler`.
///
/// `f` receives a [`CancellationToken`] so it can abort cooperatively. If the
/// task is cancelled (or the [`Task`] handle is dropped) before completion,
/// the result is never delivered. The scheduler must run tasks on the
/// spawning thread; otherwise delivery is safely discarded.
pub fn spawn_task<F, T>(scheduler: Arc<dyn Scheduler + Send + Sync>, f: F) -> Task<T>
where
    T: 'static + Send,
    F: FnOnce(&CancellationToken) -> T + Send + 'static,
{
    static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);

    let sink = Rc::new(Sink::new());
    let slot: DeliverySlot = {
        let sink = sink.clone();
        Box::new(move |value: Box<dyn Any + Send>| {
            let value = value.downcast::<T>().expect("task result type mismatch");
            sink.send(*value);
        })
    };
    DELIVERY_SLOTS.with(|slots| {
        slots.borrow_mut().insert(id, slot);
    });

    let token = CancellationToken::new();
    let worker_token = token.clone();
    let handle = std::thread::spawn(move || {
        let value = f(&worker_token);
        if worker_token.is_cancelled() {
            return;
        }
        scheduler.schedule(Box::new(move || {
            let value: Box<dyn Any + Send> = Box::new(value);
            DELIVERY_SLOTS.with(|slots| {
                if let Some(mut slot) = slots.borrow_mut().remove(&id) {
                    slot(value);
                }
            });
        }));
    });

    Task {
        id,
        token,
        sink,
        handle: RefCell::new(Some(handle)),
    }
}

/// Handle to a repeating background sampler started by [`spawn_interval`].
/// Each sample is delivered to the thread that spawned it through the given
/// [`Scheduler`]; subscribe with [`Interval::results`].
pub struct Interval<T> {
    id: u64,
    token: CancellationToken,
    stop: Arc<AtomicBool>,
    sink: Rc<Sink<T>>,
    handle: RefCell<Option<JoinHandle<()>>>,
}

impl<T> Interval<T>
where
    T: 'static,
{
    /// Request cancellation; the worker stops at the next tick boundary.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.token.cancel();
    }

    /// A stream of samples, delivered on the graph's thread.
    pub fn results(&self) -> EventStream<T> {
        self.sink.stream()
    }
}

impl<T> Drop for Interval<T> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.token.cancel();
        DELIVERY_SLOTS.with(|slots| {
            slots.borrow_mut().remove(&self.id);
        });
        // Detach the worker: it observes the stop flag within one tick.
        self.handle.borrow_mut().take();
    }
}

/// Run `f` on a background thread every `interval`, delivering each sample
/// onto the thread that owns the reactive graph via `scheduler`.
///
/// `f` receives a [`CancellationToken`] so it can abort cooperatively.
/// Deliveries are coalesced: if the previous sample is still pending on the
/// graph thread when the next one is ready, the pending one is dropped rather
/// than queueing an unbounded backlog. The scheduler must run tasks on the
/// spawning thread; otherwise delivery is safely discarded.
pub fn spawn_interval<F, T>(
    scheduler: Arc<dyn Scheduler + Send + Sync>,
    interval: Duration,
    mut f: F,
) -> Interval<T>
where
    T: 'static + Send,
    F: FnMut(&CancellationToken) -> T + Send + 'static,
{
    static NEXT_INTERVAL_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_INTERVAL_ID.fetch_add(1, Ordering::Relaxed);

    let sink = Rc::new(Sink::new());
    let slot: DeliverySlot = {
        let sink = sink.clone();
        Box::new(move |value: Box<dyn Any + Send>| {
            let value = value
                .downcast::<T>()
                .expect("interval sample type mismatch");
            sink.send(*value);
        })
    };
    DELIVERY_SLOTS.with(|slots| {
        slots.borrow_mut().insert(id, slot);
    });

    let token = CancellationToken::new();
    let worker_token = token.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let pending = Arc::new(AtomicUsize::new(0));
    let handle = std::thread::spawn(move || {
        loop {
            if worker_stop.load(Ordering::Relaxed) || worker_token.is_cancelled() {
                break;
            }
            let value = f(&worker_token);
            if worker_token.is_cancelled() {
                break;
            }
            // Coalesce bursts: skip delivery when the previous sample has not
            // been consumed by the graph thread yet.
            if pending.load(Ordering::Relaxed) == 0 {
                pending.store(1, Ordering::Relaxed);
                let pending = pending.clone();
                let id = id;
                scheduler.schedule(Box::new(move || {
                    let value: Box<dyn Any + Send> = Box::new(value);
                    DELIVERY_SLOTS.with(|slots| {
                        if let Some(slot) = slots.borrow_mut().get_mut(&id) {
                            slot(value);
                        }
                    });
                    pending.store(0, Ordering::Relaxed);
                }));
            }
            // Sleep in small chunks so stop() is honoured within ~20 ms.
            let deadline = Instant::now() + interval;
            while Instant::now() < deadline {
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        DELIVERY_SLOTS.with(|slots| {
            slots.borrow_mut().remove(&id);
        });
    });

    Interval {
        id,
        token,
        stop,
        sink,
        handle: RefCell::new(Some(handle)),
    }
}
