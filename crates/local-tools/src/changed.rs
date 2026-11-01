use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::git;
use crate::previews::{PAINTINGS, generate, stage};

const USAGE: &str = "usage: local-tools changed [--base REVISION] [--images FOLDER]";

pub fn main(arguments: Vec<String>) -> ExitCode {
    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let root = git(&["rev-parse", "--show-toplevel"]).ok_or("not inside a git checkout")?;
    std::env::set_current_dir(root.trim()).map_err(|error| error.to_string())?;
    let mut base = "HEAD".to_owned();
    let mut images = PathBuf::from("target/changed-paintings");
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or(USAGE)?;
        match argument.as_str() {
            "--base" => base = value,
            "--images" => images = value.into(),
            _ => return Err(USAGE.to_owned()),
        }
    }
    let tracked = git(&[
        "diff",
        "--no-renames",
        "--name-only",
        &base,
        "--",
        PAINTINGS,
    ])
    .ok_or_else(|| format!("could not compare the paintings with {base}"))?;
    let untracked = git(&[
        "ls-files",
        "--others",
        "--exclude-standard",
        "--",
        PAINTINGS,
    ])
    .ok_or("could not list the new paintings")?;
    let paths = tracked
        .lines()
        .chain(untracked.lines())
        .filter(|path| !path.ends_with(".browser.paint"));
    let (before, after) = stage(paths, &base, None)?;
    let _ = fs::remove_dir_all(&images);
    generate(&before, &after, &images, None, None)?;
    println!("the images are in {}", images.display());
    Ok(())
}
