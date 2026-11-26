//! Worker thread pool for parallel descriptor mirroring (`VICTORY_VIEW_THREADS`).

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

/// Default worker threads (including the caller thread) for parallel descriptor work.
const DEFAULT_VIEW_THREADS: usize = 5;

#[derive(Clone, Copy)]
struct Work(NonNull<dyn Fn(usize) + Sync>);

// SAFETY: `&dyn Fn(usize) + Sync` is `Send + Sync`, and `Pool::each` joins all
// active workers before returning to the caller.
unsafe impl Send for Work {}
unsafe impl Sync for Work {}

impl Work {
    fn from_fn(f: &(dyn Fn(usize) + Sync)) -> Self {
        let ptr: *const (dyn Fn(usize) + Sync) = f;
        // SAFETY: `Pool::each` ensures workers never touch `Work` after `each` returns.
        let erased: *const (dyn Fn(usize) + Sync + 'static) = unsafe { std::mem::transmute(ptr) };
        Work(NonNull::new(erased.cast_mut()).expect("closure reference is non-null"))
    }

    fn call(self, index: usize) {
        // SAFETY: the referenced closure stays alive until `Pool::each` drains `running == 0`.
        let f = unsafe { self.0.as_ref() };
        f(index);
    }
}

struct State {
    work: Option<Work>,
    next: usize,
    count: usize,
    running: usize,
    panicked: bool,
}

struct Pool {
    state: Mutex<State>,
    more: Condvar,
    done: Condvar,
}

impl Pool {
    fn new(name: &'static str, threads: usize) -> &'static Pool {
        let pool: &'static Pool = Box::leak(Box::new(Pool {
            state: Mutex::new(State {
                work: None,
                next: 0,
                count: 0,
                running: 0,
                panicked: false,
            }),
            more: Condvar::new(),
            done: Condvar::new(),
        }));
        for i in 1..threads {
            let _ = std::thread::Builder::new()
                .name(format!("{name}-{i}"))
                .spawn(move || pool.help());
        }
        pool
    }

    fn help(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            while state.work.is_none() || state.next >= state.count {
                state = self.more.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            let work = state.work.expect("checked above");
            let index = state.next;
            state.next += 1;
            state.running += 1;
            drop(state);

            let panicked = catch_unwind(AssertUnwindSafe(|| work.call(index))).is_err();

            state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if panicked {
                state.panicked = true;
                state.next = state.count;
            }
            state.running -= 1;
            if state.running == 0 {
                self.done.notify_one();
            }
        }
    }

    fn each(&self, count: usize, f: &(dyn Fn(usize) + Sync)) {
        if count <= 1 {
            for i in 0..count {
                f(i);
            }
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        // Re-entrant or concurrent callers run inline instead of waiting on `done`.
        if state.work.is_some() {
            drop(state);
            for i in 0..count {
                f(i);
            }
            return;
        }
        let work = Work::from_fn(f);
        state.work = Some(work);
        state.next = 0;
        state.count = count;
        state.panicked = false;
        self.more.notify_all();

        let mut caller_panic = None;
        while state.next < state.count {
            let index = state.next;
            state.next += 1;
            drop(state);

            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| f(index))) {
                state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                state.panicked = true;
                state.next = state.count;
                caller_panic = Some(payload);
                break;
            }

            state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        }

        while state.running > 0 {
            state = self.done.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        state.work = None;
        let worker_panicked = state.panicked;
        drop(state);

        if let Some(payload) = caller_panic {
            resume_unwind(payload);
        }
        if worker_panicked {
            panic!("worker panicked during each_view");
        }
    }
}

fn configured_threads() -> usize {
    static THREADS: AtomicUsize = AtomicUsize::new(0);
    let cached = THREADS.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let threads = std::env::var("VICTORY_VIEW_THREADS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_VIEW_THREADS)
        .clamp(1, 16);
    THREADS.store(threads, Ordering::Relaxed);
    threads
}

/// Calls `f` with every number below `count`, on this thread and on threads
/// kept for making descriptors (`VICTORY_VIEW_THREADS`, 5 with this one; 1:
/// on this one alone).
pub(crate) fn each_view(count: usize, f: &(dyn Fn(usize) + Sync)) {
    let threads = configured_threads();
    if count <= 1 || threads <= 1 {
        for i in 0..count {
            f(i);
        }
        return;
    }
    static POOL: OnceLock<&'static Pool> = OnceLock::new();
    POOL.get_or_init(|| Pool::new("victory-views", threads))
        .each(count, f);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn the_pool_calls_every_number_once() {
        for threads in [1usize, 2, 4, 7] {
            let pool = Pool::new("victory-views-test", threads);
            std::thread::scope(|s| {
                for _ in 0..3 {
                    s.spawn(|| {
                        let seen: Vec<AtomicUsize> = (0..64).map(|_| AtomicUsize::new(0)).collect();
                        pool.each(64, &|i| {
                            seen[i].fetch_add(1, Ordering::Relaxed);
                        });
                        for c in &seen {
                            assert_eq!(c.load(Ordering::Relaxed), 1);
                        }
                    });
                }
            });
        }
    }

    #[test]
    fn a_panicking_job_reaches_the_caller() {
        let pool = Pool::new("victory-views-test", 3);
        for bad in 0..8usize {
            let ran = catch_unwind(AssertUnwindSafe(|| {
                pool.each(8, &|i| assert_ne!(i, bad, "(expected)"));
            }));
            assert!(ran.is_err());
        }
        let ran = AtomicUsize::new(0);
        pool.each(8, &|_| {
            ran.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(ran.load(Ordering::Relaxed), 8);
    }
}
