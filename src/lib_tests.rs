//! The crate root's own tests: the live-client smoke test, the bindings agreement between
//! platforms, and the send/sync statics. Kept out of `lib.rs` so the file that every interface
//! getter is added to stays the small one.

use serial_test::serial;

use super::*;

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn basic_test() {
    let client = Client::init().unwrap();

    let _cb = client.register_callback(|p: PersonaStateChange| {
        println!("Got callback: {:?}", p);
    });

    let utils = client.utils();
    println!("Utils:");
    println!("AppId: {:?}", utils.app_id());
    println!("UI Language: {}", utils.ui_language());

    let apps = client.apps();
    println!("Apps");
    println!("IsInstalled(480): {}", apps.is_app_installed(AppId(480)));
    println!("InstallDir(480): {}", apps.app_install_dir(AppId(480)));
    println!("BuildId: {}", apps.app_build_id());
    println!("AppOwner: {:?}", apps.app_owner());
    println!("Langs: {:?}", apps.available_game_languages());
    println!("Lang: {}", apps.current_game_language());
    println!("Beta: {:?}", apps.current_beta_name());

    let friends = client.friends();
    println!("Friends");
    let list = friends.get_friends(FriendFlags::IMMEDIATE);
    println!("{:?}", list);
    for f in &list {
        println!("Friend: {:?} - {}({:?})", f.id(), f.name(), f.state());
        friends.request_user_information(f.id(), true);
    }
    friends.request_user_information(SteamId(76561198174976054), true);

    for _ in 0..50 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}

#[test]
fn steamid_test() {
    let steamid = SteamId(76561198040894045);
    assert_eq!("STEAM_0:1:40314158", steamid.steamid32());

    let steamid = SteamId(76561198174976054);
    assert_eq!("STEAM_0:0:107355163", steamid.steamid32());
}

/// A second dispatch of the same context on the same thread is what used to
/// block on the callback lock; refusing entry is what makes it a no-op.
#[test]
fn nested_dispatch_of_same_context_is_refused() {
    let owner = 0u8;
    let other = 0u8;
    let owner_ptr = &owner as *const u8 as *const ();
    let other_ptr = &other as *const u8 as *const ();

    let outer = DispatchGuard::try_new(owner_ptr).expect("first entry allowed");
    assert!(
        DispatchGuard::try_new(owner_ptr).is_none(),
        "nested dispatch of the same context must be refused"
    );
    {
        // A different context has its own lock, so it stays dispatchable.
        let second = DispatchGuard::try_new(other_ptr).expect("other context allowed");
        assert!(DispatchGuard::try_new(owner_ptr).is_some());
        drop(second);
    }
    drop(outer);

    // After the loop unwinds, the context is enterable again.
    let again = DispatchGuard::try_new(owner_ptr).expect("re-entry after exit allowed");
    drop(again);
}

fn recorder(order: &Arc<Mutex<Vec<u8>>>, n: u8) -> impl FnOnce(&Callbacks) + Send {
    let order = Arc::clone(order);
    move |_: &Callbacks| {
        order.lock().unwrap().push(n);
    }
}

/// The dispatch loop holds the callback lock while user code runs, and
/// `std::sync::Mutex` is not reentrant, so a callback that registers or
/// drops a handle has to be deferred instead of re-entering a lock it
/// already owns.
#[test]
fn deferral_is_per_context_and_drains_in_arrival_order() {
    let cbs = Callbacks::new();
    // Only the addresses are used, as opaque context identities.
    let owner = 0u8;
    let other = 0u8;
    let owner_ptr = &owner as *const u8 as *const ();
    let other_ptr = &other as *const u8 as *const ();

    let order: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    // Not inside a dispatch loop: the operation applies on the spot.
    cbs.run_or_defer(owner_ptr, recorder(&order, 1));
    assert_eq!(*order.lock().unwrap(), vec![1]);

    {
        let _outer = DispatchGuard::new(owner_ptr);

        // Same context as the dispatch loop: queued, nothing run yet.
        cbs.run_or_defer(owner_ptr, recorder(&order, 2));
        // A different context owns a different lock, so it must not be
        // made to wait for a dispatch loop it is not part of.
        cbs.run_or_defer(other_ptr, recorder(&order, 3));
        assert_eq!(*order.lock().unwrap(), vec![1, 3]);

        {
            let _inner = DispatchGuard::new(other_ptr);
            // Nesting replaced the identity, so owner is no longer
            // "this thread's dispatch" and this applies immediately.
            cbs.run_or_defer(owner_ptr, recorder(&order, 4));
            assert_eq!(*order.lock().unwrap(), vec![1, 3, 4]);
        }

        cbs.drain();
        assert_eq!(*order.lock().unwrap(), vec![1, 3, 4, 2]);
    }

    // Leaving the guard restored the thread state.
    cbs.run_or_defer(owner_ptr, recorder(&order, 5));
    assert_eq!(*order.lock().unwrap(), vec![1, 3, 4, 2, 5]);
}

/// Deferring must not lose the registration: the drained operation writes
/// the same handler into the same map a direct call would have used.
#[test]
fn deferred_registration_lands_in_the_handler_map_and_fires() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let cbs = Callbacks::new();
    let owner = 0u8;
    let owner_ptr = &owner as *const u8 as *const ();
    let hits = Arc::new(AtomicUsize::new(0));

    {
        let _guard = DispatchGuard::new(owner_ptr);
        let hits = Arc::clone(&hits);
        cbs.run_or_defer(owner_ptr, move |callbacks| {
            callbacks.set_handler(
                7,
                Box::new(move |_| {
                    hits.fetch_add(1, Ordering::SeqCst);
                }),
            );
        });
        assert_eq!(
            cbs.callbacks.lock().unwrap().len(),
            0,
            "queued registration applied early"
        );

        cbs.drain();
    }

    let mut map = cbs.callbacks.lock().unwrap();
    let handler = map
        .get_mut(&7)
        .expect("drain should have installed the handler");
    handler(std::ptr::null_mut());
    drop(map);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Removal defers the same way, which is what makes dropping a handle
    // from inside a callback safe.
    {
        let _guard = DispatchGuard::new(owner_ptr);
        cbs.run_or_defer(owner_ptr, |callbacks| callbacks.remove_handler(7));
        assert_eq!(cbs.callbacks.lock().unwrap().len(), 1);
        cbs.drain();
    }
    assert!(cbs.callbacks.lock().unwrap().is_empty());
}

/// Every test that needs a running Steam client must be `#[ignore]`d, or CI
/// (which has no client) goes red for the wrong reason -- and worse, a
/// green local run then means nothing, because the dev box does have Steam.
///
/// This scans the sources rather than trusting a convention nobody enforces.
/// Two numbers are checked, not one: a scan that found nothing must not
/// count as a pass.
#[test]
fn steam_dependent_tests_are_marked_ignored() {
    // Assembled at runtime so this scanner does not match its own source:
    // written literally, the gate would flag itself as a client-dependent
    // test that is missing `#[ignore]`.
    let init_calls: Vec<String> = ["Client", "Server", "SteamAPI"]
        .iter()
        .map(|owner| format!("{owner}::init"))
        .collect();
    let mut scanned = 0usize;
    let mut needing_client = 0usize;
    let mut violations: Vec<String> = Vec::new();

    for path in source_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        for (index, line) in lines.iter().enumerate() {
            // A `#[test]` hidden behind a comment declares nothing; counting
            // it would make this scan look busier than it is.
            if *line != "#[test]" {
                continue;
            }
            scanned += 1;

            // Attributes sit between `#[test]` and the `fn` line.
            let mut attributes = Vec::new();
            let mut body_start = index + 1;
            for j in (index + 1)..lines.len() {
                let next = lines[j];
                if next.starts_with("#[") || next.starts_with("/**") || next.starts_with("///") {
                    attributes.push(next.to_string());
                }
                if next.starts_with("fn ") || next.contains("fn ") {
                    body_start = j;
                    break;
                }
            }
            let ignored = attributes
                .iter()
                .any(|a| a.starts_with("#[ignore") || a.starts_with("#[cfg_attr(test, ignore"));

            // Read exactly this function's body: brace depth from the `fn`
            // line until it closes again. A fixed line window would bleed
            // into the next function and blame this one for its calls.
            let mut body = String::new();
            let mut depth = 0i32;
            let mut seen_body = false;
            for next in lines[body_start..].iter() {
                body.push_str(next);
                body.push('\n');
                depth += next.matches('{').count() as i32;
                depth -= next.matches('}').count() as i32;
                if next.contains('{') {
                    seen_body = true;
                }
                if seen_body && depth <= 0 {
                    break;
                }
            }
            if init_calls.iter().any(|call| body.contains(call)) {
                needing_client += 1;
                if !ignored {
                    violations.push(format!("{}:{}", path.display(), body_start + 1));
                }
            }
        }
    }

    // Floor: this repo has well over 60 test functions, so a scan that came
    // back nearly empty means the scan itself broke.
    assert!(
        scanned >= 60,
        "only found {scanned} #[test] functions -- the scanner is broken, not the code"
    );
    assert!(
        needing_client >= 10,
        "only found {needing_client} client-dependent tests -- the matcher is broken"
    );
    assert!(
        violations.is_empty(),
        "tests that need a Steam client must be #[ignore]d, found at: {violations:?}"
    );
}

/// The four generated bindings must expose the same set of entry points.
///
/// They do not agree on everything -- the same enum has been generated as
/// `c_int` on one platform and `c_uint` on another, which is a real bug that
/// only showed up when Linux compiled it -- so this gate compares the
/// callable names, where any difference means a regeneration dropped or
/// renamed an API on one platform.
#[test]
fn generated_bindings_expose_the_same_api_on_every_platform() {
    let mut sets: Vec<(String, std::collections::BTreeSet<String>)> = Vec::new();
    for os in ["windows", "linux", "macos", "linuxarm"] {
        let path = format!(
            "{}/steamworks-sys/src/{os}_bindings.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let Ok(text) = std::fs::read_to_string(&path) else {
            // Running from a packaged crate, where the `-sys` sources come
            // from the registry instead of this directory.
            eprintln!("skipping bindings parity: {path} is not present");
            return;
        };
        let names = text
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                line.strip_prefix("pub fn SteamAPI_")
                    .or(line.strip_prefix("pub unsafe fn SteamAPI_"))
                    .map(|rest| rest.split('(').next().unwrap_or("").trim().to_string())
            })
            .filter(|name| !name.is_empty())
            .collect();
        sets.push((os.to_string(), names));
    }

    assert_eq!(sets.len(), 4, "expected four bindings files");
    for (os, set) in &sets {
        assert!(
            set.len() >= 1000,
            "{os} bindings only exposed {} names -- the parse is broken",
            set.len()
        );
    }
    let reference = &sets[0];
    for other in &sets[1..] {
        let missing: Vec<&String> = reference.1.difference(&other.1).collect();
        let extra: Vec<&String> = other.1.difference(&reference.1).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "{} and {} disagree: missing={missing:?} extra={extra:?}",
            reference.0,
            other.0
        );
    }
}

fn source_files() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
