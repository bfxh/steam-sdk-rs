use super::*;
#[cfg(test)]
use serial_test::serial;

mod api;
mod cached_ugc;
mod changes;
mod download;
mod files;
mod platforms;
mod visibility;
mod workshop_update;

pub use cached_ugc::*;
pub use changes::*;
pub use download::*;
pub use files::*;
pub use platforms::*;
pub use visibility::*;
pub use workshop_update::*;

/// Access to the steam remote storage interface
pub struct RemoteStorage {
    pub(crate) rs: *mut sys::ISteamRemoteStorage,
    pub(crate) util: *mut sys::ISteamUtils,
    pub(crate) inner: Arc<Inner>,
}

impl Clone for RemoteStorage {
    fn clone(&self) -> Self {
        RemoteStorage {
            inner: self.inner.clone(),
            rs: self.rs,
            util: self.util,
        }
    }
}

/// A name is passed to Steam as a C string, so an embedded NUL is a bad
/// argument rather than a panic.
fn cloud_file_name(name: &str) -> Result<CString, SteamError> {
    CString::new(name).map_err(|_| SteamError::InvalidParameter)
}

/// Steam sizes these calls with an int32; a length it cannot express is a bad
/// argument, not something to clip.
fn cloud_chunk_len(len: usize) -> Result<i32, SteamError> {
    i32::try_from(len).map_err(|_| SteamError::InvalidParameter)
}

/// Offline guards for the one-shot cloud file calls. These need no Steam
/// client: they pin the argument checks that run before any FFI call.
#[cfg(test)]
mod offline_tests {
    use super::*;

    #[test]
    fn embedded_nul_in_a_cloud_file_name_is_rejected_not_panicked() {
        assert!(cloud_file_name("saves/slot1").is_ok());
        assert!(matches!(
            cloud_file_name("slot1\0second"),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn lengths_steam_cannot_express_are_rejected_not_clipped() {
        assert_eq!(cloud_chunk_len(0).unwrap(), 0);
        assert_eq!(cloud_chunk_len(4096).unwrap(), 4096);
        assert_eq!(cloud_chunk_len(i32::MAX as usize).unwrap(), i32::MAX);
        for too_big in [i32::MAX as usize + 1, usize::MAX] {
            assert!(
                matches!(cloud_chunk_len(too_big), Err(SteamError::InvalidParameter)),
                "{too_big} bytes should not be accepted as an int32 length"
            );
        }
    }

    #[test]
    fn local_change_kinds_map_onto_the_variants_steam_defines() {
        use sys::ERemoteStorageLocalFileChange as C;
        let mapped: Vec<_> = [
            C::k_ERemoteStorageLocalFileChange_Invalid,
            C::k_ERemoteStorageLocalFileChange_FileUpdated,
            C::k_ERemoteStorageLocalFileChange_FileDeleted,
        ]
        .iter()
        .map(|k| LocalFileChangeKind::from(*k))
        .collect();
        assert_eq!(
            mapped,
            vec![
                LocalFileChangeKind::Invalid,
                LocalFileChangeKind::Updated,
                LocalFileChangeKind::Deleted
            ]
        );
        // Every Steam variant has to land on a distinct safe variant, or two
        // different states would read back as one.
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
    }

    #[test]
    fn local_path_kinds_map_onto_the_variants_steam_defines() {
        use sys::ERemoteStorageFilePathType as P;
        let mapped: Vec<_> = [
            P::k_ERemoteStorageFilePathType_Invalid,
            P::k_ERemoteStorageFilePathType_Absolute,
            P::k_ERemoteStorageFilePathType_APIFilename,
        ]
        .iter()
        .map(|p| LocalFilePathKind::from(*p))
        .collect();
        assert_eq!(
            mapped,
            vec![
                LocalFilePathKind::Invalid,
                LocalFilePathKind::Absolute,
                LocalFilePathKind::ApiFilename
            ]
        );
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
    }

    #[test]
    fn the_four_visibility_codes_keep_their_distinct_steam_numbers() {
        use sys::ERemoteStoragePublishedFileVisibility as V;
        let codes = [
            (
                PublishedFileVisibility::Public,
                V::k_ERemoteStoragePublishedFileVisibilityPublic,
            ),
            (
                PublishedFileVisibility::FriendsOnly,
                V::k_ERemoteStoragePublishedFileVisibilityFriendsOnly,
            ),
            (
                PublishedFileVisibility::Private,
                V::k_ERemoteStoragePublishedFileVisibilityPrivate,
            ),
            (
                PublishedFileVisibility::Unlisted,
                V::k_ERemoteStoragePublishedFileVisibilityUnlisted,
            ),
        ];
        for (ours, theirs) in codes {
            assert_eq!(V::try_from(ours).unwrap(), theirs);
        }
        // Distinctness is the point, not the names: two visibilities sharing a code would let a
        // caller set one and have Steam store the other.
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(
                    V::try_from(codes[i].0).unwrap(),
                    V::try_from(codes[j].0).unwrap()
                );
            }
        }
    }

    #[test]
    fn a_visibility_steam_was_never_told_about_has_no_code_to_send() {
        // There is no out-of-range variant here to defer to (the bindings declare exactly 0..3),
        // so the only alternative to refusing is picking a real visibility for the caller. A
        // private item written back as Public is a publish, and no test would show it.
        assert!(matches!(
            sys::ERemoteStoragePublishedFileVisibility::try_from(PublishedFileVisibility::Unknown),
            Err(SteamError::InvalidParameter)
        ));
    }
}

/// Round-trips the one-shot file APIs. Needs a client, so it is ignored by
/// default; run it with `cargo test -- --ignored`.
#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_one_shot_cloud_io() {
    let client = Client::init().unwrap();
    let rs = client.remote_storage();

    let payload = b"steamworks-rs one-shot write".to_vec();
    rs.write_file("qoder-one-shot.bin", &payload).unwrap();
    // A second, empty write must not be sent as a null payload.
    rs.write_file("qoder-empty.bin", &[]).unwrap();

    assert_eq!(rs.read_file("qoder-one-shot.bin", 4096).unwrap(), payload);
    assert_eq!(rs.read_file("qoder-empty.bin", 4096).unwrap(), Vec::new());
    // Asking for fewer bytes than the file holds truncates, it does not fail.
    assert_eq!(rs.read_file("qoder-one-shot.bin", 4).unwrap().len(), 4);
    assert!(matches!(
        rs.read_file("qoder-does-not-exist.bin", 16),
        Err(SteamError::FileNotFound)
    ));

    let _batch = rs.begin_write_batch().unwrap();
    rs.write_file("qoder-in-batch.bin", b"batched").unwrap();
    drop(_batch);

    for change in rs.local_file_changes() {
        println!("{change:?}");
    }
    if let Some((total, available)) = rs.quota() {
        println!("quota: {available} of {total} bytes free");
    }
    for name in [
        "qoder-one-shot.bin",
        "qoder-empty.bin",
        "qoder-in-batch.bin",
    ] {
        rs.file(name).delete();
    }
}

#[test]
#[serial]
#[ignore = "requires a running Steam client"]
fn test_cloud() {
    use std::io::{Read, Write};
    let client = Client::init().unwrap();

    let rs = client.remote_storage();
    println!("Listing files:");
    for f in rs.files() {
        println!("{:?}", f);
    }

    {
        let test = rs.file("test.txt");
        let mut w = test.write();
        write!(w, "Testing").unwrap();
    }

    println!("Listing files:");
    for f in rs.files() {
        println!("{:?}", f);
    }

    let mut output = String::new();
    let test = rs.file("test.txt");
    test.read().read_to_string(&mut output).unwrap();
    println!("Got: {:?}", output);

    assert_eq!(output, "Testing");
}
