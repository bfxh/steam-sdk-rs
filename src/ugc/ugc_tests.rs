//! Measurement guards for the workshop query/update surface.
//!
//! Every test here pins a decision that a future batch could quietly undo: which
//! Steam code means which variant, where a read stops, and what an unusable argument
//! answers as. They need no Steam client.
use super::*;

#[test]
fn steam_out_strings_are_cut_inside_the_buffer_that_owns_them() {
    let mut buffer = [0 as c_char; 8];
    buffer[..5].copy_from_slice(&[72, 105, 0, 33, 33].map(i8_cast));
    assert_eq!(steam_out_string(&buffer), "Hi");

    // No terminator inside the buffer: the whole thing is the answer, and
    // nothing past it is read.
    let full = b"12345678".map(i8_cast);
    assert_eq!(steam_out_string(&full), "12345678");

    let empty = [0 as c_char; 4];
    assert_eq!(steam_out_string(&empty), "");

    // Steam hands back bytes, not promised UTF-8.
    let not_utf8 = [0xffu8, b'a', 0].map(i8_cast);
    assert_eq!(steam_out_string(&not_utf8), "\u{fffd}a");
}

/// Keeps the fixtures byte-for-byte without `as` casts at every literal.
const fn i8_cast(byte: u8) -> i8 {
    byte as i8
}

#[test]
fn an_empty_cursor_is_no_next_page() {
    let none = [0 as c_char; 6];
    assert_eq!(next_cursor_from(&none), None);

    let mut page = [0 as c_char; 6];
    page[..5].copy_from_slice(&b"abc12".map(i8_cast));
    assert_eq!(next_cursor_from(&page).as_deref(), Some("abc12"));
}

#[test]
fn preview_types_map_without_collapsing_or_guessing() {
    let named = [
        (0, PreviewKind::Image),
        (1, PreviewKind::YouTubeVideo),
        (2, PreviewKind::Sketchfab),
        (3, PreviewKind::EnvironmentMapHorizontalCross),
        (4, PreviewKind::EnvironmentMapLatLong),
        (5, PreviewKind::Clip),
    ];
    for (raw, expected) in named {
        assert_eq!(preview_kind_from(raw), expected);
    }
    let mapped: Vec<PreviewKind> = named.iter().map(|(_, kind)| *kind).collect();
    for i in 0..mapped.len() {
        for j in (i + 1)..mapped.len() {
            assert_ne!(mapped[i], mapped[j], "Steam codes must not merge");
        }
    }
    // The sentinel and the gaps are answers this binding cannot name.
    for unknown in [-1, 6, 128, 255, i32::MAX] {
        assert_eq!(
            preview_kind_from(unknown),
            PreviewKind::Unknown,
            "{unknown} must not be reported as a named preview type"
        );
    }
}

#[test]
fn the_constants_the_query_readers_size_their_buffers_with_are_usable() {
    // Guards against an edit that makes a buffer smaller than the length
    // handed to Steam: the call would then write past the allocation.
    for len in [UGC_KEY_VALUE_TAG_MAX, UGC_GAME_BRANCH_MAX] {
        assert!(len > 1, "a {len}-byte buffer cannot hold a value and a nul");
        let buffer = vec![0 as c_char; len as usize];
        assert_eq!(buffer.len(), len as usize);
    }
    let tag_buffer = vec![0 as c_char; sys::k_cchTagListMax as usize];
    assert_eq!(tag_buffer.len(), 1025);
}

#[test]
fn preview_kinds_round_trip_and_the_unnamed_one_is_refused() {
    // Going to Steam and back must land on the same kind, or a preview uploaded
    // as a video would come out of the next query as something else.
    let kinds = [
        PreviewKind::Image,
        PreviewKind::YouTubeVideo,
        PreviewKind::Sketchfab,
        PreviewKind::EnvironmentMapHorizontalCross,
        PreviewKind::EnvironmentMapLatLong,
        PreviewKind::Clip,
    ];
    let mut codes = Vec::new();
    for kind in kinds {
        let raw = preview_kind_to_sys(kind).expect("a named kind always maps");
        let round_trip = preview_kind_from(raw as i32);
        assert_eq!(round_trip, kind, "{kind:?} did not survive the round trip");
        codes.push(raw as i32);
    }
    for i in 0..codes.len() {
        for j in (i + 1)..codes.len() {
            assert_ne!(
                codes[i], codes[j],
                "two kinds would reach Steam as one code"
            );
        }
    }
    assert_eq!(
        preview_kind_to_sys(PreviewKind::Unknown),
        Err(SteamError::InvalidParameter),
        "there is no integer that means 'a code this wrapper cannot name'"
    );
}

#[test]
fn a_missing_preview_file_is_an_answer_not_a_panic() {
    // The inherited `preview_path` does `canonicalize().unwrap()`; this is the
    // same question with an answer.
    let missing = std::path::Path::new("no-such-preview-file-9f3a2b.png");
    assert!(preview_file_path(missing).is_err());
    // The error must be the "cannot find it" one, not a generic refusal: a
    // caller branching on FileNotFound would otherwise branch on nothing.
    assert_eq!(
        preview_file_path(missing).err(),
        Some(SteamError::FileNotFound)
    );

    let real = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let resolved = preview_file_path(&real).expect("the manifest is readable");
    assert!(!resolved.as_bytes().is_empty());
    assert!(
        !resolved.as_bytes().contains(&0),
        "a C string body must not hold a nul"
    );
}

#[test]
fn steam_reported_counts_are_clamped_to_the_buffer() {
    assert_eq!(clamped_entries(3, 10), 3);
    assert_eq!(clamped_entries(10, 10), 10);
    // Steam answering more than the buffer holds must not slice past it, which
    // is what `content_descriptor` used to do.
    assert_eq!(clamped_entries(11, 10), 10);
    assert_eq!(clamped_entries(usize::MAX, 10), 10);
    assert_eq!(clamped_entries(0, 0), 0);
}

#[test]
fn descriptor_preferences_are_read_through_the_clamp() {
    let buffer = [sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_NudityOrSexualContent;
        UGC_CONTENT_DESCRIPTORS_MAX];
    // A count above the buffer must not slice past it.
    assert_eq!(
        clamped_entries(UGC_CONTENT_DESCRIPTORS_MAX + 7, buffer.len()),
        UGC_CONTENT_DESCRIPTORS_MAX
    );
    // And the slice the method takes has to stay inside the array.
    let taken = &buffer[..clamped_entries(UGC_CONTENT_DESCRIPTORS_MAX + 7, buffer.len())];
    assert_eq!(taken.len(), UGC_CONTENT_DESCRIPTORS_MAX);
}

#[test]
fn a_tag_group_with_an_interior_nul_is_refused_as_a_whole() {
    let good = tag_group_strings(&["env", "pvp"]);
    assert!(good.is_ok());
    assert_eq!(good.unwrap().len(), 2);

    // One unusable tag must fail the group rather than quietly send the part
    // that survived: Steam would then filter on something nobody asked for.
    let bad = tag_group_strings(&["env", "bad\0tag"]);
    assert_eq!(bad.err(), Some(SteamError::InvalidParameter));

    let empty: [&str; 0] = [];
    assert_eq!(tag_group_strings(&empty).map(|v| v.len()), Ok(0));
}
