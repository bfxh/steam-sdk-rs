#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
// bindgen emits a Rust struct for every C vtable, and a wrapper crate that only ever hands
// `*mut ISteamX` back to Steam never constructs most of them. `dead_code` reads that as two
// warnings per platform (`ISteamContentServer`, `obstack`) on rustc 1.80.0 and none on current
// stable, which is why the repository's `-D warnings` check step cannot pass without this. The
// allow belongs here rather than in the CI command line because cargo's `RUSTFLAGS` reaches
// dependencies built in the same invocation, and this crate is a workspace member -- so cargo
// does not apply its `--cap-lints allow` protection to it.
#![allow(dead_code)]

#[cfg(target_os = "windows")]
include!("windows_bindings.rs");

#[cfg(target_os = "macos")]
include!("macos_bindings.rs");

#[cfg(all(target_os = "linux", not(target_arch = "aarch64")))]
include!("linux_bindings.rs");

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
include!("linuxarm_bindings.rs");
