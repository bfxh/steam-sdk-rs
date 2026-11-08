//! Getting a guard out of a lock that another thread may have panicked while holding.
//!
//! Every lock in this crate guards routing state -- the callback table, the deferred-operation queue,
//! the registered networking sockets, the warning sink -- and every one of them is taken from inside
//! dispatch, which runs user code. A panic in that user code used to poison the lock and then
//! `.unwrap()` it into a second panic, in a different caller, at a later time: one bad callback turned
//! into a permanently unusable client, with the failure surfacing far from the code that caused it.
//!
//! Recovering is a trade, not a free win, and it is stated here rather than assumed away: a thread that
//! panicked mid-update may have left its data half-changed, and `into_inner` hands that state over
//! anyway. What makes it acceptable in this crate is what those locks actually hold -- tables of
//! `handle -> sender` and queues of boxed closures, whose partial state costs a stale or dropped entry,
//! not an unsafe read. The alternative was already measured to be worse: a poison that never clears,
//! because nothing re-initialises the table, and a panic that lands in an unrelated caller.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// The guard for a mutex that a panicking callback may have poisoned.
pub(crate) fn mutex_guard<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The read guard for an `RwLock` that a panicking callback may have poisoned.
pub(crate) fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The write guard for an `RwLock` that a panicking callback may have poisoned.
pub(crate) fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The guard for a lock taken while something is being dropped: recover, and say so, but never panic.
///
/// `mutex_guard` above recovers quietly, which is right when the caller has nowhere to report to.
/// A drop path is different: the registry it is repairing may genuinely be inconsistent, and swallowing
/// that silently would hide the only trace of it -- while panicking here would be worse still, because
/// a panic raised in `Drop` while another panic is unwinding aborts the process. So this variant
/// trades the guard for a printed complaint and returns `None`, which the caller skips over. It lives
/// here rather than at the call site so that the *choice* between recovering, reporting and aborting
/// is made in exactly one file.
pub(crate) fn try_mutex_guard<T>(lock: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match lock.lock() {
        Ok(guard) => Some(guard),
        Err(err) => {
            eprintln!("a callback panicked while holding this lock; skipping that step: {err:?}");
            None
        }
    }
}

#[cfg(test)]
mod poison_tests {
    //! Poison is reachable without a Steam client: it only takes a thread that panics while holding the
    //! lock. These tests are what makes the recovery above a judged decision rather than a comment.
    use super::{mutex_guard, read_guard, write_guard};
    use std::sync::{Arc, Mutex, RwLock};

    /// A panicking holder poisons the lock; the caller still gets the data back instead of a second
    /// panic in an unrelated frame.
    #[test]
    fn a_poisoned_mutex_still_yields_its_data() {
        let lock = Arc::new(Mutex::new(7u32));
        let held = Arc::clone(&lock);
        let worker = std::thread::spawn(move || {
            let _guard = held.lock().unwrap();
            assert_eq!(*_guard, 7);
            panic!("a callback panicked while holding this lock");
        });
        assert!(
            worker.join().is_err(),
            "the worker must really have panicked"
        );
        assert!(lock.is_poisoned(), "and really have poisoned it");

        assert_eq!(*mutex_guard(&lock), 7, "recovered, not aborted");
        assert!(
            std::panic::catch_unwind(|| lock.lock().unwrap()).is_err(),
            "the old `lock().unwrap()` is what this replaced -- it still panics"
        );
    }

    #[test]
    fn a_poisoned_rwlock_recovers_for_both_directions() {
        let lock = Arc::new(RwLock::new(vec![1u8, 2]));
        let held = Arc::clone(&lock);
        let worker = std::thread::spawn(move || {
            let mut guard = held.write().unwrap();
            guard.push(3);
            panic!("panicking after a partial update poisons the lock");
        });
        assert!(worker.join().is_err());
        assert!(lock.is_poisoned());

        assert_eq!(read_guard(&lock).len(), 3, "the read side keeps working");
        write_guard(&lock).push(4);
        assert_eq!(*read_guard(&lock), vec![1, 2, 3, 4]);
    }

    /// An unpoisoned lock behaves exactly as it did before: the recovery arm must not become the
    /// ordinary path in a way that hides a real failure.
    #[test]
    fn an_unpoisoned_lock_is_unaffected() {
        let mutex = Mutex::new(1u32);
        *mutex_guard(&mutex) += 1;
        assert_eq!(*mutex_guard(&mutex), 2);
        assert!(!mutex.is_poisoned());

        let rw = RwLock::new(0u32);
        *write_guard(&rw) = 5;
        assert_eq!(*read_guard(&rw), 5);
        assert!(!rw.is_poisoned());
    }
}
