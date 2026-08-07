//! Time-based stream operators: `debounce`, `throttle`, and `delay`.
//!
//! Operators take a [`Timer`] that runs one-shot tasks on the graph's thread
//! (the GUI thread in a Qt application). [`ManualTimer`] drives them
//! deterministically in tests.

use crate::EventStream;
use crate::graph::{Node, NodeCommon, NodeKind, TxState, schedule_node};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};
use std::time::Duration;

/// A source of one-shot delayed tasks. Tasks must run on the thread that owns
/// the reactive graph.
pub trait Timer {
    /// Run `task` once after `delay`, on the graph's thread.
    fn schedule_after(&self, delay: Duration, task: Box<dyn FnOnce()>);
}

/// A test [`Timer`] that queues tasks until the test fires them.
#[derive(Default)]
pub struct ManualTimer {
    queue: RefCell<VecDeque<(Duration, Box<dyn FnOnce()>)>>,
}

impl ManualTimer {
    /// Create an empty timer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of queued tasks.
    pub fn pending(&self) -> usize {
        self.queue.borrow().len()
    }

    /// Run the oldest queued task.
    pub fn fire_next(&self) {
        let task = self.queue.borrow_mut().pop_front().map(|(_, task)| task);
        if let Some(task) = task {
            task();
        }
    }
}

impl Timer for ManualTimer {
    fn schedule_after(&self, delay: Duration, task: Box<dyn FnOnce()>) {
        self.queue.borrow_mut().push_back((delay, task));
    }
}

struct DebounceNode<T> {
    common: NodeCommon,
    timer: Rc<dyn Timer>,
    window: Duration,
    generation: Cell<u64>,
    timer_fired: Cell<bool>,
    pending: RefCell<Option<Rc<dyn Any>>>,
    self_weak: Weak<Self>,
    _marker: PhantomData<T>,
}

impl<T: 'static> DebounceNode<T> {
    fn new(timer: Rc<dyn Timer>, window: Duration) -> Rc<Self> {
        Rc::new_cyclic(|self_weak| Self {
            common: NodeCommon::new(NodeKind::Stream),
            timer,
            window,
            generation: Cell::new(0),
            timer_fired: Cell::new(false),
            pending: RefCell::new(None),
            self_weak: self_weak.clone(),
            _marker: PhantomData,
        })
    }

    fn on_timer(&self, generation: u64) {
        if generation != self.generation.get() {
            return; // superseded by a newer event
        }
        self.timer_fired.set(true);
        let node: Rc<dyn Node> = self
            .self_weak
            .upgrade()
            .expect("debounce node must be alive while its timer fires");
        crate::transaction(|| schedule_node(node));
    }

    fn arm(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let weak = self.self_weak.clone();
        let window = self.window;
        let timer = self.timer.clone();
        timer.schedule_after(
            window,
            Box::new(move || {
                if let Some(node) = weak.upgrade() {
                    node.on_timer(generation);
                }
            }),
        );
    }
}

impl<T: 'static> Node for DebounceNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        if self.timer_fired.take() {
            return match self.pending.borrow_mut().take() {
                Some(event) => vec![event],
                None => Vec::new(),
            };
        }
        let events = self.take_incoming();
        if let Some(last) = events.into_iter().last() {
            *self.pending.borrow_mut() = Some(last);
            self.arm();
        }
        Vec::new()
    }
}

struct ThrottleNode<T> {
    common: NodeCommon,
    timer: Rc<dyn Timer>,
    window: Duration,
    generation: Cell<u64>,
    timer_fired: Cell<bool>,
    in_window: Cell<bool>,
    trailing: RefCell<Option<Rc<dyn Any>>>,
    self_weak: Weak<Self>,
    _marker: PhantomData<T>,
}

impl<T: 'static> ThrottleNode<T> {
    fn new(timer: Rc<dyn Timer>, window: Duration) -> Rc<Self> {
        Rc::new_cyclic(|self_weak| Self {
            common: NodeCommon::new(NodeKind::Stream),
            timer,
            window,
            generation: Cell::new(0),
            timer_fired: Cell::new(false),
            in_window: Cell::new(false),
            trailing: RefCell::new(None),
            self_weak: self_weak.clone(),
            _marker: PhantomData,
        })
    }

    fn on_timer(&self, generation: u64) {
        if generation != self.generation.get() {
            return;
        }
        self.timer_fired.set(true);
        let node: Rc<dyn Node> = self
            .self_weak
            .upgrade()
            .expect("throttle node must be alive while its timer fires");
        crate::transaction(|| schedule_node(node));
    }

    fn arm(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let weak = self.self_weak.clone();
        let window = self.window;
        let timer = self.timer.clone();
        timer.schedule_after(
            window,
            Box::new(move || {
                if let Some(node) = weak.upgrade() {
                    node.on_timer(generation);
                }
            }),
        );
    }
}

impl<T: 'static> Node for ThrottleNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let mut outputs = Vec::new();
        if self.timer_fired.take() {
            // End of the window: emit the trailing event if one arrived, then
            // open the gate so the next event leads a fresh window.
            if let Some(event) = self.trailing.borrow_mut().take() {
                outputs.push(event);
            }
            self.in_window.set(false);
            return outputs;
        }
        for event in self.take_incoming() {
            if self.in_window.get() {
                *self.trailing.borrow_mut() = Some(event);
            } else {
                self.in_window.set(true);
                self.arm();
                outputs.push(event); // leading edge
            }
        }
        outputs
    }
}

struct DelayNode<T> {
    common: NodeCommon,
    timer: Rc<dyn Timer>,
    window: Duration,
    generation: Cell<u64>,
    timer_fired: Cell<bool>,
    timer_pending: Cell<bool>,
    queue: RefCell<VecDeque<Rc<dyn Any>>>,
    self_weak: Weak<Self>,
    _marker: PhantomData<T>,
}

impl<T: 'static> DelayNode<T> {
    fn new(timer: Rc<dyn Timer>, window: Duration) -> Rc<Self> {
        Rc::new_cyclic(|self_weak| Self {
            common: NodeCommon::new(NodeKind::Stream),
            timer,
            window,
            generation: Cell::new(0),
            timer_fired: Cell::new(false),
            timer_pending: Cell::new(false),
            queue: RefCell::new(VecDeque::new()),
            self_weak: self_weak.clone(),
            _marker: PhantomData,
        })
    }

    fn on_timer(&self, generation: u64) {
        if generation != self.generation.get() {
            return;
        }
        self.timer_fired.set(true);
        let node: Rc<dyn Node> = self
            .self_weak
            .upgrade()
            .expect("delay node must be alive while its timer fires");
        crate::transaction(|| schedule_node(node));
    }

    fn arm(&self) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.timer_pending.set(true);
        let weak = self.self_weak.clone();
        let window = self.window;
        let timer = self.timer.clone();
        timer.schedule_after(
            window,
            Box::new(move || {
                if let Some(node) = weak.upgrade() {
                    node.on_timer(generation);
                }
            }),
        );
    }
}

impl<T: 'static> Node for DelayNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        if self.timer_fired.take() {
            self.timer_pending.set(false);
            let mut outputs = Vec::new();
            let event = self.queue.borrow_mut().pop_front();
            if let Some(event) = event {
                outputs.push(event);
                if !self.queue.borrow().is_empty() {
                    self.arm();
                }
            }
            return outputs;
        }
        let events = self.take_incoming();
        if !events.is_empty() {
            let mut queue = self.queue.borrow_mut();
            for event in events {
                queue.push_back(event);
            }
            if !self.timer_pending.get() {
                self.arm();
            }
        }
        Vec::new()
    }
}

impl<T> EventStream<T>
where
    T: 'static,
{
    /// Emit only after the stream has been quiet for `window`.
    ///
    /// Each event restarts the window; when it elapses without a newer event,
    /// the latest event is emitted. Superseded timer firings are no-ops.
    pub fn debounce(&self, timer: Rc<dyn Timer>, window: Duration) -> EventStream<T> {
        let node = DebounceNode::<T>::new(timer, window);
        let node_any: Rc<dyn Node> = node.clone();
        crate::graph::connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Emit at most one event per `window`: the first event immediately
    /// (leading edge) and the last event of the window when it elapses
    /// (trailing edge).
    pub fn throttle(&self, timer: Rc<dyn Timer>, window: Duration) -> EventStream<T> {
        let node = ThrottleNode::<T>::new(timer, window);
        let node_any: Rc<dyn Node> = node.clone();
        crate::graph::connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }

    /// Emit every event in order, each at least `window` after it arrived.
    pub fn delay(&self, timer: Rc<dyn Timer>, window: Duration) -> EventStream<T> {
        let node = DelayNode::<T>::new(timer, window);
        let node_any: Rc<dyn Node> = node.clone();
        crate::graph::connect(self.node.clone(), node_any.clone());
        EventStream::new(node_any)
    }
}
