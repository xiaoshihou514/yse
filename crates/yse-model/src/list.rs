//! An incremental list model: a current snapshot plus a stream of structural
//! changes that views can apply without rebuilding.

use crate::{EventStream, Sink};
use std::cell::RefCell;

/// A structural change to a [`ListModel`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListChange<T> {
    /// `items` inserted starting at `index`.
    Insert { index: usize, items: Vec<T> },
    /// `len` rows removed starting at `index`.
    Remove { index: usize, len: usize },
    /// Rows replaced starting at `index`.
    Update { index: usize, items: Vec<T> },
    /// The whole list was replaced.
    Reset { items: Vec<T> },
}

/// An incremental list model.
pub struct ListModel<T> {
    items: RefCell<Vec<T>>,
    changes: Sink<ListChange<T>>,
}

impl<T> ListModel<T>
where
    T: Clone + 'static,
{
    /// Create an empty model.
    pub fn new() -> Self {
        Self {
            items: RefCell::new(Vec::new()),
            changes: Sink::new(),
        }
    }

    /// Create a model from `items`.
    pub fn from(items: Vec<T>) -> Self {
        let model = Self::new();
        model.replace_all(items);
        model
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether the model is empty.
    pub fn is_empty(&self) -> bool {
        self.items.borrow().is_empty()
    }

    /// A stream of structural changes.
    pub fn changes(&self) -> EventStream<ListChange<T>> {
        self.changes.stream()
    }

    /// The row at `index`, if any.
    pub fn get(&self, index: usize) -> Option<T> {
        self.items.borrow().get(index).cloned()
    }

    /// A snapshot of the current rows.
    pub fn snapshot(&self) -> Vec<T> {
        self.items.borrow().clone()
    }

    /// Append `item`.
    pub fn push(&self, item: T) {
        self.insert(self.len(), item);
    }

    /// Append every item in `items`.
    pub fn extend(&self, items: Vec<T>) {
        if items.is_empty() {
            return;
        }
        let index = self.len();
        self.items.borrow_mut().extend(items.iter().cloned());
        self.changes.send(ListChange::Insert { index, items });
    }

    /// Insert `item` at `index`.
    pub fn insert(&self, index: usize, item: T) {
        let mut items = self.items.borrow_mut();
        if index > items.len() {
            return;
        }
        items.insert(index, item.clone());
        self.changes.send(ListChange::Insert {
            index,
            items: vec![item],
        });
    }

    /// Remove the row at `index`; returns whether a row was removed.
    pub fn remove(&self, index: usize) -> bool {
        self.remove_range(index, 1)
    }

    /// Remove `len` rows starting at `index`; returns whether anything was
    /// removed.
    pub fn remove_range(&self, index: usize, len: usize) -> bool {
        let mut items = self.items.borrow_mut();
        if index >= items.len() || len == 0 {
            return false;
        }
        let end = (index + len).min(items.len());
        items.drain(index..end);
        self.changes.send(ListChange::Remove {
            index,
            len: end - index,
        });
        true
    }

    /// Replace the row at `index`; returns whether the row existed.
    pub fn set(&self, index: usize, item: T) -> bool {
        let mut items = self.items.borrow_mut();
        let Some(slot) = items.get_mut(index) else {
            return false;
        };
        *slot = item.clone();
        self.changes.send(ListChange::Update {
            index,
            items: vec![item],
        });
        true
    }

    /// Remove every row.
    pub fn clear(&self) {
        let len = self.items.borrow().len();
        if len == 0 {
            return;
        }
        self.items.borrow_mut().clear();
        self.changes.send(ListChange::Remove { index: 0, len });
    }

    /// Replace the entire list.
    pub fn replace_all(&self, items: Vec<T>) {
        *self.items.borrow_mut() = items.clone();
        self.changes.send(ListChange::Reset { items });
    }

    /// Sort the list in place with `compare`, emitting a single `Reset`.
    pub fn sort_by<F>(&self, compare: F)
    where
        F: FnMut(&T, &T) -> std::cmp::Ordering,
    {
        let mut items = self.items.borrow_mut();
        items.sort_by(compare);
        let snapshot = items.clone();
        drop(items);
        self.changes.send(ListChange::Reset { items: snapshot });
    }

    /// Remove every row that does not satisfy `keep`, emitting one `Remove`
    /// change per contiguous removed run (in ascending index order).
    pub fn retain<F>(&self, mut keep: F)
    where
        F: FnMut(&T) -> bool,
    {
        let mut items = self.items.borrow_mut();
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut run_start: Option<usize> = None;
        for (index, item) in items.iter().enumerate() {
            if keep(item) {
                if let Some(start) = run_start.take() {
                    runs.push((start, index - start));
                }
            } else if run_start.is_none() {
                run_start = Some(index);
            }
        }
        if let Some(start) = run_start {
            runs.push((start, items.len() - start));
        }
        // Remove from the end so earlier indices stay valid.
        for (index, len) in runs.iter().rev() {
            items.drain(*index..*index + *len);
        }
        drop(items);
        // Descending order keeps earlier indices valid for the view.
        runs.reverse();
        for (index, len) in runs {
            self.changes.send(ListChange::Remove { index, len });
        }
    }
}

impl<T> Default for ListModel<T>
where
    T: Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
