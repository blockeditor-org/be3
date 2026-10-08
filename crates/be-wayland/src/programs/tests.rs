use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

mod a_desktop_entry_reads_its_localized_name_and_its_lists;
mod a_terminal_program_runs_inside_a_terminal_emulator;
mod an_entry_is_hidden_by_its_flags_and_the_desktops_it_names;
mod an_entry_whose_try_exec_is_missing_is_left_out;
mod an_exec_line_drops_file_codes_and_expands_the_rest;
mod an_exec_line_keeps_quoted_arguments_whole;
mod an_icon_is_found_at_the_size_closest_to_the_one_asked_for;
mod locales_are_tried_from_the_most_specific;
mod png_and_svg_icons_load_as_images;
mod the_data_home_shadows_the_system_entries;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "be-wayland-programs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the scratch directory is made");
        Self(path)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn write(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("a file has a folder"))
            .expect("the folder is made");
        std::fs::write(&path, contents).expect("the file is written");
        path
    }

    fn executable(&self, relative: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.write(relative, b"#!/bin/sh\n");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the file is made executable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn application(name: &str, exec: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n{extra}")
}

fn entry(exec: &str, extra: &str) -> DesktopEntry {
    DesktopEntry::parse(
        "test.desktop",
        "/usr/share/applications/test.desktop",
        &application("Test", exec, extra),
        &[],
    )
    .expect("the entry parses")
}

fn environment(scratch: &Scratch, data_dirs: &[&str]) -> Environment {
    Environment {
        data_dirs: data_dirs.iter().map(|dir| scratch.path(dir)).collect(),
        path: vec![scratch.path("bin")],
        home: Some(scratch.path("home")),
        ..Environment::default()
    }
}

fn ids(entries: &[DesktopEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.id.as_str()).collect()
}

fn ends_with(path: Option<PathBuf>, tail: &str) -> bool {
    path.as_deref()
        .is_some_and(|path: &Path| path.ends_with(tail))
}
