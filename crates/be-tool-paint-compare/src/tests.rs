use std::path::{Path, PathBuf};

use beui_paint_snapshot::BrowserSnapshot;

use crate::{Options, run};

mod a_record_without_update_fails_on_a_changed_painting;
mod a_second_run_that_agrees_but_fails_is_reported_as_flaky;
mod a_second_run_that_paints_differently_is_reported_as_flaky;
mod changed_and_new_paintings_are_kept_with_why_and_every_one_is_used;
mod changed_names_the_new_and_differing_paintings;

/// An empty directory of the test's own.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("paint-compare-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// A browser painting whose document is `text`.
fn painting(text: &str) -> Vec<u8> {
    BrowserSnapshot::new([8, 8], text).encode().unwrap()
}

fn put(directory: &Path, name: &str, bytes: &[u8]) {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(directory.join(name), bytes).unwrap();
}

fn options(directory: &Path) -> Options {
    Options {
        made: directory.join("made"),
        accepted: directory.join("accepted"),
        out: directory.join("out"),
        record: None,
        update: true,
        changed: None,
        again: false,
    }
}

fn ran(options: &Options) {
    run(options).unwrap();
}
