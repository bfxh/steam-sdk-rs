## Problem

The UGC `UpdateHandle`/`QueryHandle` builder methods panic on bad input: `CString::new(...).unwrap()` on NUL-containing strings (11 sites), `path.canonicalize().unwrap()` for missing content/preview paths (2), `assert!(sys::Set*(...))` when Steam rejects a property (13), and `SteamParamStringArray::new`'s `expect` reached through `tags` (1). A NUL byte in a workshop item title takes down the process.

## Fix (breaking)

22 builder methods return `Result<Self, SteamError>`:

- 13 UpdateHandle: title, description, language, preview_path, content_path, metadata, visibility, tags, add_key_value_tag, remove_key_value_tag, add_content_descriptor, remove_content_descriptor, remove_all_key_value_tags
- 9 QueryHandle: exclude_tag, require_tag, language, add_required_tag, add_excluded_tag, set_language, set_cloud_file_name_filter, set_search_text, add_required_key_value_tag

Error mapping: NUL byte -> `SteamError::InvalidParameter`; canonicalize NotFound -> `FileNotFound`, other io -> `IOFailure`; false from the sys `Set*`/`Add*` call -> `InvalidParameter`. The three "Panics if ..." doc sections become "# Errors" sections. `#[must_use]` is dropped from the converted methods (Result is implicitly `must_use`).

Behavior improvements beyond the signatures:

- QueryHandle string methods previously failed silently in release builds (`debug_assert!` compiles out); they now return the error.
- `SteamParamStringArray::new` returns `Option<...>` and frees already-converted strings on failure.
- `init_for_game_server` keeps its `-> bool` signature (NUL -> false) to avoid cascading into `Server::init_for_game_server`.

The workshop example chains with `?` and reports a preparation error.

Out of scope: `QueryHandle::fetch`'s taken-handle unwrap (fetch-consumed semantics, separate concern) and `submit`'s `change_note` `.ok()` silent drop (no panic; documented degradation).

## Migration

Callers chaining `.title(...).description(...)...` add `?` in a fallible context (or handle the error where the chain is built). Meant for the next major-version window (cf. the breaking-change plans discussed around #253).

## Testing

- 8 offline tests construct handles with a null `ISteamUGC` and assert the error mapping on every converted error path; they pass without any Steam client, and a regression that reached the sys call would fail on `handle.unwrap()` instead of crashing the process.
- Gates: fmt clean / build clean / clippy lint-kind multiset identical / serial 15-11-2 (= 7-11-2 + 8 new) / parallel 10x clean.
