//! Concrete node types for sources, operators, and observers.

use crate::graph::{
    Node, NodeCommon, NodeKind, Subscription, TxState, assert_no_cycle, connect, schedule_node,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

// --- Sources -----------------------------------------------------------------

pub(crate) struct SignalSourceNode<T> {
    pub common: NodeCommon,
    pub value: RefCell<Rc<T>>,
    prev: RefCell<Option<Rc<dyn Any>>>,
}

impl<T: 'static> SignalSourceNode<T> {
    pub fn new(value: T) -> Self {
        let value = Rc::new(value);
        Self {
            common: NodeCommon::new(NodeKind::Signal),
            value: RefCell::new(value.clone()),
            prev: RefCell::new(Some(value as Rc<dyn Any>)),
        }
    }
}

impl<T: 'static + PartialEq> Node for SignalSourceNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn current_any(&self) -> Option<Rc<dyn Any>> {
        Some(self.value.borrow().clone())
    }

    fn apply_write(&self, value: Rc<dyn Any>) {
        *self.value.borrow_mut() = value.downcast::<T>().expect("type mismatch on write");
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let current: Rc<T> = self.value.borrow().clone();
        let changed = match &*self.prev.borrow() {
            None => true,
            Some(previous) => {
                let previous = previous.clone().downcast::<T>().expect("type mismatch");
                *previous != *current
            }
        };
        if changed {
            *self.prev.borrow_mut() = Some(current.clone());
            vec![current]
        } else {
            Vec::new()
        }
    }
}

pub(crate) struct StreamSourceNode<T> {
    pub common: NodeCommon,
    _marker: std::marker::PhantomData<T>,
}

impl<T> StreamSourceNode<T> {
    pub fn new() -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: 'static> Node for StreamSourceNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn apply_write(&self, value: Rc<dyn Any>) {
        self.push_incoming(value);
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        self.take_incoming()
    }
}

// --- Signal operators ---------------------------------------------------------

pub(crate) struct MapSignalNode<A, B> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    pub f: RefCell<Box<dyn FnMut(&A) -> B>>,
    pub current: RefCell<Option<Rc<B>>>,
}

impl<A, B> MapSignalNode<A, B> {
    pub fn new(parent: Rc<dyn Node>, f: Box<dyn FnMut(&A) -> B>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Signal),
            parent,
            f: RefCell::new(f),
            current: RefCell::new(None),
        }
    }
}

impl<A: 'static, B: 'static + PartialEq> Node for MapSignalNode<A, B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn current_any(&self) -> Option<Rc<dyn Any>> {
        self.current
            .borrow()
            .clone()
            .map(|value| value as Rc<dyn Any>)
    }

    fn recompute(&self) -> bool {
        let a = self
            .parent
            .current_any()
            .expect("signal parent has a value")
            .downcast::<A>()
            .expect("type mismatch");
        let b = (self.f.borrow_mut())(&a);
        let changed = match &*self.current.borrow() {
            Some(current) => **current != b,
            None => true,
        };
        if changed {
            *self.current.borrow_mut() = Some(Rc::new(b));
        }
        changed
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        if self.recompute() {
            vec![self.current.borrow().clone().expect("value") as Rc<dyn Any>]
        } else {
            Vec::new()
        }
    }
}

pub(crate) struct CombineNode<A, B, R> {
    pub common: NodeCommon,
    pub a: Rc<dyn Node>,
    pub b: Rc<dyn Node>,
    pub f: RefCell<Box<dyn FnMut(&A, &B) -> R>>,
    pub current: RefCell<Option<Rc<R>>>,
}

impl<A, B, R> CombineNode<A, B, R> {
    pub fn new(a: Rc<dyn Node>, b: Rc<dyn Node>, f: Box<dyn FnMut(&A, &B) -> R>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Signal),
            a,
            b,
            f: RefCell::new(f),
            current: RefCell::new(None),
        }
    }
}

impl<A: 'static, B: 'static, R: 'static + PartialEq> Node for CombineNode<A, B, R> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn current_any(&self) -> Option<Rc<dyn Any>> {
        self.current
            .borrow()
            .clone()
            .map(|value| value as Rc<dyn Any>)
    }

    fn recompute(&self) -> bool {
        let a = self
            .a
            .current_any()
            .expect("signal parent has a value")
            .downcast::<A>()
            .expect("type mismatch");
        let b = self
            .b
            .current_any()
            .expect("signal parent has a value")
            .downcast::<B>()
            .expect("type mismatch");
        let r = (self.f.borrow_mut())(&a, &b);
        let changed = match &*self.current.borrow() {
            Some(current) => **current != r,
            None => true,
        };
        if changed {
            *self.current.borrow_mut() = Some(Rc::new(r));
        }
        changed
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        if self.recompute() {
            vec![self.current.borrow().clone().expect("value") as Rc<dyn Any>]
        } else {
            Vec::new()
        }
    }
}

macro_rules! combine_node {
    ($name:ident, $($ty:ident: $field:ident),+) => {
        pub(crate) struct $name<$($ty),+, R> {
            pub common: NodeCommon,
            $(pub $field: Rc<dyn Node>,)+
            pub f: RefCell<Box<dyn FnMut($(&$ty),+) -> R>>,
            pub current: RefCell<Option<Rc<R>>>,
        }

        impl<$($ty),+, R> $name<$($ty),+, R> {
            pub fn new($($field: Rc<dyn Node>,)+ f: Box<dyn FnMut($(&$ty),+) -> R>) -> Self {
                Self {
                    common: NodeCommon::new(NodeKind::Signal),
                    $($field,)+
                    f: RefCell::new(f),
                    current: RefCell::new(None),
                }
            }
        }

        impl<$($ty: 'static),+, R: 'static + PartialEq> Node for $name<$($ty),+, R> {
            fn common(&self) -> &NodeCommon { &self.common }

            fn current_any(&self) -> Option<Rc<dyn Any>> {
                self.current.borrow().clone().map(|value| value as Rc<dyn Any>)
            }

            fn recompute(&self) -> bool {
                $(
                    let $field = self.$field.current_any()
                        .expect("signal parent has a value")
                        .downcast::<$ty>()
                        .expect("type mismatch");
                )+
                let r = (self.f.borrow_mut())($(&$field),+);
                let changed = match &*self.current.borrow() {
                    Some(current) => **current != r,
                    None => true,
                };
                if changed {
                    *self.current.borrow_mut() = Some(Rc::new(r));
                }
                changed
            }

            fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
                if self.recompute() {
                    vec![self.current.borrow().clone().expect("value") as Rc<dyn Any>]
                } else {
                    Vec::new()
                }
            }
        }
    };
}

combine_node!(Combine3Node, A: a, B: b, C: c);
combine_node!(Combine4Node, A: a, B: b, C: c, D: d);

pub(crate) struct DistinctSignalNode<A> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    pub current: RefCell<Option<Rc<A>>>,
}

impl<A> DistinctSignalNode<A> {
    pub fn new(parent: Rc<dyn Node>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Signal),
            parent,
            current: RefCell::new(None),
        }
    }
}

impl<A: 'static + PartialEq> Node for DistinctSignalNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn current_any(&self) -> Option<Rc<dyn Any>> {
        self.current
            .borrow()
            .clone()
            .map(|value| value as Rc<dyn Any>)
    }

    fn recompute(&self) -> bool {
        let a = self
            .parent
            .current_any()
            .expect("signal parent has a value")
            .downcast::<A>()
            .expect("type mismatch");
        let changed = match &*self.current.borrow() {
            Some(current) => **current != *a,
            None => true,
        };
        if changed {
            *self.current.borrow_mut() = Some(a);
        }
        changed
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        if self.recompute() {
            vec![self.current.borrow().clone().expect("value") as Rc<dyn Any>]
        } else {
            Vec::new()
        }
    }
}

pub(crate) struct StartWithNode<A> {
    pub common: NodeCommon,
    pub current: RefCell<Rc<A>>,
    prev: RefCell<Option<Rc<dyn Any>>>,
}

impl<A: 'static> StartWithNode<A> {
    pub fn new(initial: A) -> Self {
        let initial = Rc::new(initial);
        Self {
            common: NodeCommon::new(NodeKind::Signal),
            current: RefCell::new(initial.clone()),
            prev: RefCell::new(Some(initial as Rc<dyn Any>)),
        }
    }
}

impl<A: 'static + PartialEq> Node for StartWithNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn current_any(&self) -> Option<Rc<dyn Any>> {
        Some(self.current.borrow().clone())
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.clone().downcast::<A>().expect("type mismatch");
            let changed = match &*self.prev.borrow() {
                None => true,
                Some(previous) => *previous.clone().downcast::<A>().expect("type mismatch") != *a,
            };
            if changed {
                *self.current.borrow_mut() = a;
                *self.prev.borrow_mut() = Some(event.clone());
                outputs.push(event);
            }
        }
        outputs
    }
}

// --- Stream operators --------------------------------------------------------

pub(crate) struct ChangesNode<A> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    _marker: std::marker::PhantomData<A>,
}

impl<A> ChangesNode<A> {
    pub fn new(parent: Rc<dyn Node>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<A: 'static> Node for ChangesNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_signal_backed(&self) -> bool {
        true
    }

    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        self.parent.current_any()
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        match self.parent.current_any() {
            Some(value) => vec![value],
            None => Vec::new(),
        }
    }
}

pub(crate) struct MapStreamNode<A, B> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    pub f: RefCell<Box<dyn FnMut(&A) -> B>>,
    signal_backed: Cell<bool>,
}

impl<A, B> MapStreamNode<A, B> {
    pub fn new(parent: Rc<dyn Node>, f: Box<dyn FnMut(&A) -> B>) -> Self {
        let signal_backed = parent.is_signal_backed();
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent,
            f: RefCell::new(f),
            signal_backed: Cell::new(signal_backed),
        }
    }
}

impl<A: 'static, B: 'static> Node for MapStreamNode<A, B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_signal_backed(&self) -> bool {
        self.signal_backed.get()
    }

    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        let a = self.parent.pull_output()?.downcast::<A>().ok()?;
        let b = (self.f.borrow_mut())(&a);
        Some(Rc::new(b))
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.downcast::<A>().expect("type mismatch");
            let b = (self.f.borrow_mut())(&a);
            outputs.push(Rc::new(b) as Rc<dyn Any>);
        }
        outputs
    }
}

pub(crate) struct FilterStreamNode<A> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    pub f: RefCell<Box<dyn FnMut(&A) -> bool>>,
    signal_backed: Cell<bool>,
}

impl<A> FilterStreamNode<A> {
    pub fn new(parent: Rc<dyn Node>, f: Box<dyn FnMut(&A) -> bool>) -> Self {
        let signal_backed = parent.is_signal_backed();
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent,
            f: RefCell::new(f),
            signal_backed: Cell::new(signal_backed),
        }
    }
}

impl<A: 'static> Node for FilterStreamNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_signal_backed(&self) -> bool {
        self.signal_backed.get()
    }

    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        let value = self.parent.pull_output()?;
        let a = value.clone().downcast::<A>().ok()?;
        if (self.f.borrow_mut())(&a) {
            Some(value)
        } else {
            None
        }
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.clone().downcast::<A>().expect("type mismatch");
            if (self.f.borrow_mut())(&a) {
                outputs.push(event);
            }
        }
        outputs
    }
}

pub(crate) struct FilterMapStreamNode<A, B> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    pub f: RefCell<Box<dyn FnMut(&A) -> Option<B>>>,
    signal_backed: Cell<bool>,
}

impl<A, B> FilterMapStreamNode<A, B> {
    pub fn new(parent: Rc<dyn Node>, f: Box<dyn FnMut(&A) -> Option<B>>) -> Self {
        let signal_backed = parent.is_signal_backed();
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent,
            f: RefCell::new(f),
            signal_backed: Cell::new(signal_backed),
        }
    }
}

impl<A: 'static, B: 'static> Node for FilterMapStreamNode<A, B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_signal_backed(&self) -> bool {
        self.signal_backed.get()
    }

    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        let a = self.parent.pull_output()?.downcast::<A>().ok()?;
        let b = (self.f.borrow_mut())(&a)?;
        Some(Rc::new(b))
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.downcast::<A>().expect("type mismatch");
            if let Some(b) = (self.f.borrow_mut())(&a) {
                outputs.push(Rc::new(b) as Rc<dyn Any>);
            }
        }
        outputs
    }
}

pub(crate) struct DistinctStreamNode<A> {
    pub common: NodeCommon,
    pub parent: Rc<dyn Node>,
    last: RefCell<Option<Rc<dyn Any>>>,
    signal_backed: Cell<bool>,
    _marker: std::marker::PhantomData<A>,
}

impl<A> DistinctStreamNode<A> {
    pub fn new(parent: Rc<dyn Node>) -> Self {
        let signal_backed = parent.is_signal_backed();
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent,
            last: RefCell::new(None),
            signal_backed: Cell::new(signal_backed),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<A: 'static + PartialEq> Node for DistinctStreamNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_signal_backed(&self) -> bool {
        self.signal_backed.get()
    }

    fn pull_output(&self) -> Option<Rc<dyn Any>> {
        let value = self.parent.pull_output()?;
        let a = value.clone().downcast::<A>().ok()?;
        let is_new = match &*self.last.borrow() {
            None => true,
            Some(last) => last
                .clone()
                .downcast::<A>()
                .map_or(true, |previous| *previous != *a),
        };
        if is_new {
            *self.last.borrow_mut() = Some(value.clone());
            Some(value)
        } else {
            None
        }
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.clone().downcast::<A>().expect("type mismatch");
            let is_new = match &*self.last.borrow() {
                None => true,
                Some(last) => last
                    .clone()
                    .downcast::<A>()
                    .map_or(true, |previous| *previous != *a),
            };
            if is_new {
                *self.last.borrow_mut() = Some(event.clone());
                outputs.push(event);
            }
        }
        outputs
    }
}

pub(crate) struct FoldStreamNode<A, B> {
    pub common: NodeCommon,
    pub acc: RefCell<Rc<B>>,
    pub f: RefCell<Box<dyn FnMut(&B, &A) -> B>>,
}

impl<A, B> FoldStreamNode<A, B> {
    pub fn new(init: B, f: Box<dyn FnMut(&B, &A) -> B>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            acc: RefCell::new(Rc::new(init)),
            f: RefCell::new(f),
        }
    }
}

impl<A: 'static, B: 'static> Node for FoldStreamNode<A, B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let a = event.downcast::<A>().expect("type mismatch");
            let acc = self.acc.borrow();
            let next = (self.f.borrow_mut())(&acc, &a);
            drop(acc);
            let next = Rc::new(next);
            self.acc.replace(next.clone());
            outputs.push(next as Rc<dyn Any>);
        }
        outputs
    }
}

pub(crate) struct MergeStreamNode<A> {
    pub common: NodeCommon,
    _marker: std::marker::PhantomData<A>,
}

impl<A> MergeStreamNode<A> {
    pub fn new() -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<A: 'static> Node for MergeStreamNode<A> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        self.take_incoming()
    }
}

pub(crate) struct SampleStreamNode<A, S> {
    pub common: NodeCommon,
    pub signal: Weak<dyn Node>,
    _marker: std::marker::PhantomData<A>,
    _marker_signal: std::marker::PhantomData<S>,
}

impl<A, S> SampleStreamNode<A, S> {
    pub fn new(signal: &Rc<dyn Node>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            signal: Rc::downgrade(signal),
            _marker: std::marker::PhantomData,
            _marker_signal: std::marker::PhantomData,
        }
    }
}

impl<A: 'static, S: 'static> Node for SampleStreamNode<A, S> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        let mut outputs = Vec::new();
        for event in events {
            let _a = event.downcast::<A>().expect("type mismatch");
            if let Some(signal) = self.signal.upgrade()
                && let Some(value) = signal.current_any()
            {
                outputs.push(value);
            }
        }
        outputs
    }
}

pub(crate) struct FlatMapSwitchNode<A, B> {
    pub common: NodeCommon,
    pub f: RefCell<Box<dyn FnMut(&A) -> Rc<dyn Node>>>,
    inner_sub: RefCell<Option<Subscription>>,
    inner_incoming: RefCell<Vec<Rc<dyn Any>>>,
    self_weak: Weak<Self>,
    _marker: std::marker::PhantomData<B>,
}

impl<A, B> FlatMapSwitchNode<A, B> {
    pub fn new(f: Box<dyn FnMut(&A) -> Rc<dyn Node>>, self_weak: Weak<Self>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            f: RefCell::new(f),
            inner_sub: RefCell::new(None),
            inner_incoming: RefCell::new(Vec::new()),
            self_weak,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<A: 'static, B: 'static> Node for FlatMapSwitchNode<A, B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn forward_incoming(&self, value: Rc<dyn Any>) {
        self.inner_incoming.borrow_mut().push(value);
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let outer_events = self.take_incoming();
        for event in outer_events {
            let a = event.downcast::<A>().expect("type mismatch");
            let inner = (self.f.borrow_mut())(&a);

            // Unsubscribe from the previous inner stream, if any.
            let _ = self.inner_sub.borrow_mut().take();

            // Reject inner streams that would create a cycle.
            let self_rc = self
                .self_weak
                .upgrade()
                .expect("flat_map_switch node must be alive while processing");
            let self_any: Rc<dyn Node> = self_rc;
            assert_no_cycle(&inner, &self_any);

            let forward = Rc::new(ForwardNode::<B>::new(inner.clone(), self.self_weak.clone()));
            connect(inner.clone(), forward.clone());
            *self.inner_sub.borrow_mut() = Some(Subscription { node: forward });
        }
        self.inner_incoming.take()
    }
}

// --- Observers ---------------------------------------------------------------

pub(crate) struct ObserverNode<T> {
    pub common: NodeCommon,
    parent: Weak<dyn Node>,
    callback: Rc<RefCell<dyn FnMut(&T)>>,
    active: Cell<bool>,
    self_weak: Weak<Self>,
}

impl<T> ObserverNode<T> {
    pub fn new(
        parent: Rc<dyn Node>,
        callback: Rc<RefCell<dyn FnMut(&T)>>,
        self_weak: Weak<Self>,
    ) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            parent: Rc::downgrade(&parent),
            callback,
            active: Cell::new(true),
            self_weak,
        }
    }
}

impl<T: 'static> Node for ObserverNode<T> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_observer(&self) -> bool {
        true
    }

    fn deactivate(&self) {
        self.active.set(false);
    }

    fn fire(&self, value: Rc<dyn Any>) {
        if !self.active.get() {
            return;
        }
        let value = value.downcast::<T>().expect("type mismatch");
        (self.callback.borrow_mut())(&value);
    }

    fn process(&self, tx: &TxState) -> Vec<Rc<dyn Any>> {
        if !self.active.get() {
            return Vec::new();
        }
        let weak: Weak<dyn Node> = self.self_weak.clone();
        let events = self.take_incoming();
        if events.is_empty() {
            if let Some(parent) = self.parent.upgrade()
                && let Some(value) = parent.current_any()
            {
                tx.fires.borrow_mut().push((weak, value));
            }
        } else {
            let mut fires = tx.fires.borrow_mut();
            for event in events {
                fires.push((weak.clone(), event));
            }
        }
        Vec::new()
    }
}

pub(crate) struct ForwardNode<B> {
    pub common: NodeCommon,
    target: Weak<dyn Node>,
    _marker: std::marker::PhantomData<B>,
}

impl<B> ForwardNode<B> {
    pub fn new(_parent: Rc<dyn Node>, target: Weak<dyn Node>) -> Self {
        Self {
            common: NodeCommon::new(NodeKind::Stream),
            target,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B: 'static> Node for ForwardNode<B> {
    fn common(&self) -> &NodeCommon {
        &self.common
    }

    fn is_observer(&self) -> bool {
        true
    }

    fn process(&self, _tx: &TxState) -> Vec<Rc<dyn Any>> {
        let events = self.take_incoming();
        for event in events {
            if let Some(target) = self.target.upgrade()
                && !target.dependents().is_empty()
            {
                target.forward_incoming(event);
                schedule_node(target);
            }
        }
        Vec::new()
    }
}
