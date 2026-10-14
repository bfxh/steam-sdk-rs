# steamworks

Rust bindings to the [Steamworks SDK](https://partner.steamgames.com/doc/sdk), forked from
[Noxime/steamworks-rs](https://github.com/Noxime/steamworks-rs) and developed independently.

Single repository: the bindings, the generated `steamworks-sys` FFI crate, the examples and
the design notes all live here (no submodules).

## Layout

| Path | Contents |
| --- | --- |
| `src/` | the safe bindings |
| `steamworks-sys/` | bindgen-generated FFI, one bindings file per platform |
| `examples/` | examples: `stream_events.rs` (needs `--features futures`), `inventory.rs`, `http.rs`, and the multi-file game/networking demos. All of them are compile-checked by CI; running any of them needs a live Steam client, and they are not run-verified here. |
| `docs/` | design rationale and defect analysis; start with `docs/SuperSteam-Design.md` |

## Additions over upstream

| Feature | What it does |
| --- | --- |
| `Inventory` | Wraps `ISteamInventory`, which upstream never bound: item queries, consumption, exchange, trade, item drops, prices and purchases. Results are RAII-guarded — `InventoryResult` destroys the Steam handle on drop. Entry point `client.inventory()`. |
| `Http` | Wraps `ISteamHTTP`, also unbound upstream. Request building is a fallible chain (`header`, `query_param`, `raw_body`, `cookies`, timeouts); request and cookie-container handles are released on drop. Entry point `client.http()`, or `client.http().get(url, cb)` for a one-shot fetch. |
| `SafeUgcUpdate` | Panic-free UGC update builder. Invalid paths and interior nul bytes become `SafeUgcError` instead of `unwrap()` panics. Entry point `client.ugc().safe_item_update(app_id, file_id)`. |
| `CallbackStream` (`futures` feature) | Exposes the main-loop + `process_callbacks` model as a `futures::Stream`. Runtime-agnostic: depends only on the `Stream`/`Waker` traits, not tokio or any specific executor. |
| `ParentalSettings` | Wraps `ISteamParentalSettings` so a game can honour family view restrictions instead of offering gated content anyway. Both the "list membership" and "effective state" Steam answers are exposed separately. Entry point `client.parental_settings()`. |
| `GameServerStats` | Wraps `ISteamGameServerStats`: per-user stat/achievement read-write for dedicated servers, with the store/load round trip and the `StatsUnloaded` signal that tells you to re-request. Entry point `server.stats()`. |
| `Music` | Wraps `ISteamMusic` — the user's own Steam music client. Volume is clamped into Steam's [0, 1] range, and an unrecognised playback state reports `Undefined` instead of guessing. Entry point `client.music()`. |
| `Video` | Wraps `ISteamVideo`: broadcast state with viewer count, store-page opening, and Open Graph metadata reads that fail loudly when the caller's buffer is too small instead of returning truncated XML. Entry point `client.video()`. |
| `Parties` | Wraps `ISteamParties`: beacon discovery and advertising, joining with the host's connect string, the reservation accept/decline pair, and the reservation and "updated" notifications. Entry point `client.parties()`. |
| `HtmlSurface` | Wraps all 37 methods of `ISteamHTMLSurface` plus the navigation, dialog and link callbacks. `Browser` removes itself on drop, and the three "you MUST respond" callbacks are documented against their responses. Entry point `client.html_surface()`. |

`SteamId` / `AppId` / `PublishedFileId` are distinct newtypes, so the compiler rejects mixing
them. Steam does expose `PrioritizeHTTPRequest` and `GetHTTPStreamingResponseBodyData`, but this
crate's generated binding for the first carries no priority argument and the second reports no
byte count, so neither is wrapped here rather than wrapped wrongly.

## Usage

```toml
[dependencies]
steamworks = { git = "https://github.com/bfxh/steam-sdk-rs" }
```

| Crate  | SDK   | MSRV   |
| ------ | ----- | ------ |
| git    | 1.64  | 1.80.0 |
| 0.13.0 | 1.64  | 1.80.0 |
| 0.12.0 | 1.62  | 1.80.0 |

The MSRV holds for the default feature set. The optional `image` feature pulls transitive
dependencies that need a newer cargo than 1.80.0 ships, so CI checks `--all-features` on stable
only.

```rust,no_run
use steamworks::{Client, FriendFlags, PersonaStateChange};

fn main() {
    let client = Client::init().unwrap();

    let _cb = client.register_callback(|p: PersonaStateChange| {
        println!("Got callback: {p:?}");
    });

    let friends = client.friends();
    for f in friends.get_friends(FriendFlags::IMMEDIATE) {
        println!("Friend: {:?} - {}({:?})", f.id(), f.name(), f.state());
        friends.request_user_information(f.id(), true);
    }

    for _ in 0..50 {
        client.run_callbacks();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
```

Fetching a URL through the new HTTP wrapper, without leaving the callback model:

```rust,no_run
use steamworks::{Client, HttpMethod};

fn main() {
    let client = Client::init().unwrap();
    client
        .http()
        .request(HttpMethod::Get, "https://example.com/data.json")
        .and_then(|req| req.header("Accept", "application/json"))
        .and_then(|req| req.send(|result| match result {
            Ok(response) => println!("{} bytes", response.body().unwrap().len()),
            Err(e) => println!("request failed: {e}"),
        }))
        .unwrap();

    loop {
        client.run_callbacks();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
```

## Features

- `serde` — serialization for the ID and result types
- `futures` — `CallbackStream`
- `image` — icon/image data via the `image` crate
- `raw-bindings` — re-export `steamworks_sys` for calls this crate does not wrap

## Development

```bash
cargo build --all-features
cargo test --lib --all-features
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
```

Tests that need a live Steam client are marked `#[ignore]`, so `cargo test` is green on a machine
without Steam; run them with `cargo test -- --ignored`.

Advisory checks, reported but non-blocking in CI:

```bash
cargo +nightly udeps --all-targets --all-features
cargo geiger --all-features
cargo audit
```

## License

Dual-licensed under [Apache](./LICENSE-APACHE) and [MIT](./LICENSE-MIT), except for the files in
`steamworks-sys/lib/steam/`.

## Troubleshooting

Errors such as `STATUS_DLL_NOT_FOUND` or `Image not found` mean the Steamworks SDK redistributable
libraries are missing where the OS can find them — usually next to your game binary. They are in the
SDK zip under `lib\steam\redistributable_bin`.
