use std::path::{Path, PathBuf};

use crate::Snapshot;

const REVIEW: &str =
    "commit it, and review it among the paintings on the pull request's summary comment";

pub const HOW_TO_ACCEPT: &str =
    "./scripts/verify, or ./scripts/test with the test's name and --update";

pub fn assert_painting(name: &str, painting: paint_format::Painting) {
    let snapshot = Snapshot::of_painting(painting).expect("the painting could not be encoded");
    assert_accepted(name, &snapshot, HOW_TO_ACCEPT);
}

pub fn assert_accepted(name: &str, snapshot: &Snapshot, how_to_accept: &str) {
    let body = snapshot.body().expect("the painting could not be encoded");
    assert_bytes(
        name,
        how_to_accept,
        REVIEW,
        |previous| Snapshot::body_of(previous).is_ok_and(|previous| previous == body),
        || Snapshot::compress(&body).expect("the painting could not be compressed"),
        |previous| {
            let previous = match Snapshot::decode(previous) {
                Ok(previous) => previous,
                Err(error) => return format!("the accepted painting is unreadable: {error}"),
            };
            crate::difference(&previous, snapshot)
                .map(|difference| difference.description)
                .unwrap_or_else(|| {
                    "the encoded painting changed, though it looks the same".to_owned()
                })
        },
    );
}

pub(crate) fn assert_bytes(
    name: &str,
    how_to_accept: &str,
    review: &str,
    unchanged: impl FnOnce(&[u8]) -> bool,
    bytes: impl FnOnce() -> Vec<u8>,
    describe: impl FnOnce(&[u8]) -> String,
) {
    let accepted = accepted_path(name);
    record_use(&accepted);
    let updating = std::env::var_os("UPDATE_SNAPSHOTS").is_some();

    if !accepted.exists() {
        if !updating {
            panic!(
                "there is no accepted painting at {}, {review}",
                accepted.display()
            );
        }
        accept(&accepted, &bytes(), "there is no accepted painting");
        return;
    }

    let previous_bytes = read(&accepted);
    if unchanged(&previous_bytes) {
        return;
    }
    let description = describe(&previous_bytes);
    if updating {
        accept(&accepted, &bytes(), &description);
        return;
    }

    panic!("the painting changed: {description}\nto accept it:\n  {how_to_accept}\nthen {review}");
}

fn accept(accepted: &Path, bytes: &[u8], why: &str) {
    let Some(changed) = std::env::var_os("CHANGED_PAINTINGS") else {
        write(accepted, bytes);
        return;
    };
    let painting = PathBuf::from(changed).join(file_name(accepted));
    write(&painting, bytes);
    write(&painting.with_extension("paint.why"), why.as_bytes());
}

fn file_name(accepted: &Path) -> &std::ffi::OsStr {
    accepted
        .file_name()
        .unwrap_or_else(|| panic!("{} names no painting", accepted.display()))
}

fn record_use(accepted: &Path) {
    let Some(used) = std::env::var_os("USED_PAINTINGS") else {
        return;
    };
    use sha2::{Digest as _, Sha256};
    let compared = std::fs::read(accepted).unwrap_or_default();
    let hash: String = Sha256::digest(&compared)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    write(
        &PathBuf::from(used).join(file_name(accepted)),
        hash.as_bytes(),
    );
}

fn accepted_path(name: &str) -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is not set, run these tests through cargo");
    PathBuf::from(manifest)
        .join("snapshots")
        .join(format!("{name}.paint"))
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("could not create {}: {error}", parent.display()));
    }
    std::fs::write(path, bytes)
        .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}
