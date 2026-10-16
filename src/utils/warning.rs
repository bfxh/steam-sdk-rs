//! Steam's warning channel. One process-wide slot behind an `RwLock`, a `extern "C"`
//! trampoline that catches a panicking user callback rather than unwinding through Steam's
//! own frame, and the setter that fills the slot.

use super::*;

/// Global rust warning callback
static WARNING_CALLBACK: RwLock<Option<Box<dyn Fn(i32, &CStr) + Send + Sync>>> = RwLock::new(None);

/// C function to pass as the real callback, which forwards to the `WARNING_CALLBACK` if any
unsafe extern "C" fn c_warning_callback(level: i32, msg: *const c_char) {
    let lock = WARNING_CALLBACK.read().expect("warning func lock poisoned");
    let cb = match lock.as_ref() {
        Some(cb) => cb,
        None => {
            return;
        }
    };

    let s = CStr::from_ptr(msg);

    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| cb(level, s)));
    if let Err(err) = res {
        if let Some(err) = err.downcast_ref::<&str>() {
            println!("Steam warning callback panicked: {}", err);
        } else if let Some(err) = err.downcast_ref::<String>() {
            println!("Steam warning callback panicked: {}", err);
        } else {
            println!("Steam warning callback panicked");
        }
        abort();
    }
}

impl Utils {
    /// Sets the Steam warning callback, which is called to emit warning messages.
    ///
    /// The passed-in function takes two arguments: a severity level (0 = info, 1 = warning) and
    /// the message itself.
    ///
    /// See [Steamwork's debugging page](https://partner.steamgames.com/doc/sdk/api/debugging) for more info.
    pub fn set_warning_callback<F>(&self, cb: F)
    where
        F: Fn(i32, &CStr) + Send + Sync + 'static,
    {
        let mut lock = WARNING_CALLBACK
            .write()
            .expect("warning func lock poisoned");
        *lock = Some(Box::new(cb));
        unsafe {
            sys::SteamAPI_ISteamUtils_SetWarningMessageHook(self.utils, Some(c_warning_callback));
        }
    }
}
