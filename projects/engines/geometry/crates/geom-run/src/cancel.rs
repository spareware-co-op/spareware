//! Cooperative cancellation: the caller's way to stop an operation.
//!
//! The kernel reads no clock (architecture chapter 10.1). A time limit, a person's cancel button or
//! a closed document reaches an operation only through the caller's [`Cancel`], which the helper
//! reads before every chunk and every [`POLL_EVERY`](crate::meter::POLL_EVERY) items, and which
//! every [`WorkMeter`](crate::WorkMeter) reads every `POLL_EVERY` charges and whenever it is
//! polled. An operation that sees a request stops and returns
//! [`RunError::Cancelled`](crate::RunError::Cancelled), never partial results. Cancellation is the
//! one stop that is not deterministic, so it never becomes model state: a cancelled operation
//! publishes nothing (chapter 23.10).
//!
//! Method: cooperative cancellation by a shared flag polled at fixed points, as in chapter 23.10
//! ("Polling") and in the cancellation-token pattern of .NET's `CancellationToken`
//! (<https://learn.microsoft.com/en-us/dotnet/standard/threading/cancellation-in-managed-threads>).
//! It never unwinds: WebAssembly builds cannot catch an unwinding panic.
//!
//! Cost: a [`CancelToken`] is one atomic flag; [`CancelToken::cancel`] is one relaxed store, and
//! each read one relaxed load. Nothing allocates. What another [`Cancel`] costs is the caller's.
//!
//! Invariants:
//!
//! - A [`CancelToken`] is a latch: once cancelled it stays cancelled.
//! - Reading it is one atomic load, cheap enough to do before every chunk.

use core::sync::atomic::{AtomicBool, Ordering};

/// The caller's cancellation source: a flag, a deadline, a button (architecture chapter 23.10).
///
/// Implementations must be cheap (the helper asks before every chunk), must not panic, and should
/// be monotone: once `requested` has returned `true` it should keep returning `true`. The helper
/// does not rely on that: whenever any question gets `true`, the whole phase stops with
/// `Cancelled` and its meter stays stopped.
pub trait Cancel: Sync {
    /// Whether the caller has asked the operation to stop.
    fn requested(&self) -> bool;
}

/// A cancellation flag that any thread may set, once, for good.
#[derive(Debug, Default)]
pub struct CancelToken {
    requested: AtomicBool,
}

impl CancelToken {
    /// A token that has not been cancelled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requested: AtomicBool::new(false),
        }
    }

    /// Ask every operation that reads this token to stop. Safe to call from any thread, any number
    /// of times.
    pub fn cancel(&self) {
        // Relaxed is enough: the flag carries no other data, and an operation only needs to see it
        // soon, not in any order relative to other memory.
        self.requested.store(true, Ordering::Relaxed);
    }

    /// Whether [`cancel`](Self::cancel) has been called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Relaxed)
    }
}

impl Cancel for CancelToken {
    fn requested(&self) -> bool {
        self.is_cancelled()
    }
}
