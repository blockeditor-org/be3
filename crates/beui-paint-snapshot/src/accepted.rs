use std::path::{Path, PathBuf};

use crate::{BrowserSnapshot, Snapshot};

const REVIEW: &str =
    "commit it, and review it among the paintings on the pull request's summary comment";

pub const HOW_TO_ACCEPT: &str =
    "./scripts/verify, or ./scripts/test with the test's name and --update";

pub fn assert_painting(name: &str, painting: beui_paint_format::Painting) {
    let snapshot = Snapshot::of_painting(painting).expect("the painting could not be encoded");
    assert_accepted(name, &snapshot, HOW_TO_ACCEPT);
}

pub fn assert_accepted(name: &str, snapshot: &Snapshot, how_to_accept: &str) {
    let made = snapshot
        .encode()
        .expect("the painting could not be encoded");
    assert_bytes(name, how_to_accept, REVIEW, &made);
}

/// Compares what a test made with its accepted painting, or, under
/// MADE_PAINTINGS, only writes what it made into that directory, reading
/// nothing: ./scripts/verify and ./scripts/test --update compare afterwards,
/// in an action of their own (crates/be-tool-paint-compare), so that a test's
/// cached result does not depend on the painting it is compared with.
pub(crate) fn assert_bytes(name: &str, how_to_accept: &str, review: &str, made: &[u8]) {
    let accepted = accepted_path(name);
    if let Some(directory) = std::env::var_os("MADE_PAINTINGS") {
        write(&PathBuf::from(directory).join(file_name(&accepted)), made);
        return;
    }
    let updating = std::env::var_os("UPDATE_SNAPSHOTS").is_some();

    if !accepted.exists() {
        if !updating {
            panic!(
                "there is no accepted painting at {}, {review}",
                accepted.display()
            );
        }
        write(&accepted, made);
        return;
    }

    let Some(description) = changed(&read(&accepted), made) else {
        return;
    };
    if updating {
        write(&accepted, made);
        return;
    }

    panic!("the painting changed: {description}\nto accept it:\n  {how_to_accept}\nthen {review}");
}

/// What changed from the accepted painting to the one a test made, both as
/// their files hold them, or None when they are the same painting: the same
/// encoded painting, however it was compressed, or for a browser painting the
/// same bytes.
pub fn changed(accepted: &[u8], made: &[u8]) -> Option<String> {
    if BrowserSnapshot::is_browser(made) {
        return (accepted != made).then(|| crate::browser::describe(accepted, made));
    }
    let made_body = match Snapshot::body_of(made) {
        Ok(body) => body,
        Err(error) => return Some(format!("the painting made is unreadable: {error}")),
    };
    match Snapshot::body_of(accepted) {
        Ok(body) if body == made_body => return None,
        Ok(_) => {}
        Err(error) => return Some(format!("the accepted painting is unreadable: {error}")),
    }
    let (previous, current) = match (Snapshot::decode(accepted), Snapshot::decode(made)) {
        (Ok(previous), Ok(current)) => (previous, current),
        (Err(error), _) => return Some(format!("the accepted painting is unreadable: {error}")),
        (_, Err(error)) => return Some(format!("the painting made is unreadable: {error}")),
    };
    Some(
        crate::difference(&previous, &current)
            .map(|difference| difference.description)
            .unwrap_or_else(|| "the encoded painting changed, though it looks the same".to_owned()),
    )
}

fn file_name(accepted: &Path) -> &std::ffi::OsStr {
    accepted
        .file_name()
        .unwrap_or_else(|| panic!("{} names no painting", accepted.display()))
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
