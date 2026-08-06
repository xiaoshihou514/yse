//! Rough benchmark: incremental updates on a large table.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p yse-model --release --example bench_large_table
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;
use yse_model::*;

fn main() {
    const ROWS: usize = 100_000;
    const UPDATES: usize = 10_000;

    let model = ListModel::new();
    let events = Rc::new(Cell::new(0usize));
    let events_rc = events.clone();
    let _sub = model
        .changes()
        .observe(move |_| events_rc.set(events_rc.get() + 1));

    let rows: Vec<String> = (0..ROWS).map(|i| format!("row-{i:05}")).collect();

    let started = Instant::now();
    model.replace_all(rows);
    let load = started.elapsed();

    let started = Instant::now();
    for i in 0..UPDATES {
        model.set(i % ROWS, format!("updated-{i:05}"));
    }
    let updates = started.elapsed();

    let started = Instant::now();
    model.sort_by(|a, b| a.cmp(b));
    let sort = started.elapsed();

    println!(
        "rows={ROWS} updates={UPDATES} change-events={} ({} Reset + {UPDATES} Update, no rebuilds)\n\
         load={load:?}  {UPDATES} per-row updates={updates:?}  sort={sort:?}",
        events.get(),
        events.get() - UPDATES
    );
}
