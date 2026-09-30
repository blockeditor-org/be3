use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use image::RgbaImage;
use paint_snapshot::{Frame, Snapshot};

const FOLDER: &str = "snapshots";

struct Change {
    name: String,
    before: Option<Snapshot>,
    after: Option<Snapshot>,
}

fn main() {
    let root = PathBuf::from(
        String::from_utf8_lossy(&git(Path::new("."), &["rev-parse", "--show-toplevel"])).trim(),
    );
    let mut base = "HEAD".to_owned();
    let mut images = None;
    let mut arguments = std::env::args().skip(1).peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--base" => base = arguments.next().unwrap_or_else(|| usage()),
            "--images" => {
                images = Some(match arguments.next_if(|next| !next.starts_with("--")) {
                    Some(folder) => PathBuf::from(folder),
                    None => root.join("target/changed-paintings"),
                })
            }
            _ => usage(),
        }
    }

    let changes = changes(&root, &base);
    if let Some(folder) = &images {
        clear(folder);
        println!("the images are in {}", folder.display());
    }
    if changes.is_empty() {
        println!("no painting in {FOLDER}/ differs from {base}");
        return;
    }
    for change in &changes {
        for reason in reasons(change) {
            println!("{}: {reason}", change.name);
        }
        if let Some(folder) = &images {
            write_frames(change, folder);
        }
    }
}

fn changes(root: &Path, base: &str) -> Vec<Change> {
    let tracked = git(
        root,
        &["diff", "--no-renames", "--name-only", base, "--", FOLDER],
    );
    let untracked = git(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", FOLDER],
    );
    let paths: BTreeSet<String> = [tracked, untracked]
        .iter()
        .flat_map(|output| {
            String::from_utf8_lossy(output)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|path| path.ends_with(".paint"))
        .collect();

    paths
        .into_iter()
        .map(|path| {
            let revision = format!("{base}:{path}");
            let committed = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["cat-file", "-e", &revision])
                .status()
                .is_ok_and(|status| status.success());
            let before = committed.then(|| decode(&revision, &git(root, &["show", &revision])));
            let after = match std::fs::read(root.join(&path)) {
                Ok(bytes) => Some(decode(&path, &bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => fail(&format!("could not read {path}: {error}")),
            };
            let name = Path::new(&path)
                .file_stem()
                .map_or(path.clone(), |stem| stem.to_string_lossy().into_owned());
            Change {
                name,
                before,
                after,
            }
        })
        .collect()
}

fn reasons(change: &Change) -> Vec<String> {
    let (before, after) = match (&change.before, &change.after) {
        (None, None) => return Vec::new(),
        (None, Some(after)) => return vec![format!("was added, {}", frames(after.frames.len()))],
        (Some(_), None) => return vec!["was removed".to_owned()],
        (Some(before), Some(after)) => (before, after),
    };
    let Some(whole) = paint_snapshot::difference(before, after) else {
        return vec!["the encoded painting changed, though it looks the same".to_owned()];
    };
    if whole.frame.is_none() {
        return vec![whole.description];
    }
    let count = after.frames.len();
    before
        .frames
        .iter()
        .zip(&after.frames)
        .enumerate()
        .filter(|(_, (before, after))| before != after)
        .filter_map(|(index, (before, after))| {
            let description =
                paint_snapshot::difference(&alone(before), &alone(after))?.description;
            Some(match count {
                1 => description,
                count => format!("frame {} of {count} changed: {description}", index + 1),
            })
        })
        .collect()
}

fn alone(frame: &Frame) -> Snapshot {
    Snapshot {
        frames: vec![frame.clone()],
        textures: BTreeMap::new(),
    }
}

fn frames(count: usize) -> String {
    match count {
        1 => "one frame".to_owned(),
        count => format!("{count} frames"),
    }
}

fn write_frames(change: &Change, folder: &Path) {
    let count = |snapshot: Option<&Snapshot>| snapshot.map_or(0, |snapshot| snapshot.frames.len());
    let (before, after) = (change.before.as_ref(), change.after.as_ref());
    for index in 0..count(before).max(count(after)) {
        let before_frame = before.and_then(|snapshot| snapshot.frames.get(index));
        let after_frame = after.and_then(|snapshot| snapshot.frames.get(index));
        if before_frame.is_some() && before_frame == after_frame {
            continue;
        }
        let stem = format!("{}.{:02}", change.name, index + 1);
        let before_image = before_frame.and_then(|_| render(before?, index));
        let after_image = after_frame.and_then(|_| render(after?, index));
        let mut line = format!("  frame {}:", index + 1);
        if let Some(image) = &before_image {
            line += &format!(" {}", save(image, folder, &format!("{stem}.before.png")));
        }
        if let Some(image) = &after_image {
            line += &format!(" {}", save(image, folder, &format!("{stem}.after.png")));
        }
        if let (Some(before), Some(after)) = (&before_image, &after_image) {
            let highlight = paint_snapshot::highlight(before, after);
            let file = save(&highlight.image, folder, &format!("{stem}.changes.png"));
            line += &format!(" {file} ({} pixels differ)", highlight.changed);
        }
        println!("{line}");
    }
}

fn render(snapshot: &Snapshot, index: usize) -> Option<RgbaImage> {
    paint_snapshot::render(snapshot, index)
        .inspect_err(|error| eprintln!("frame {} did not render: {error}", index + 1))
        .ok()
}

fn save(image: &RgbaImage, folder: &Path, file: &str) -> String {
    let path = folder.join(file);
    image
        .save(&path)
        .unwrap_or_else(|error| fail(&format!("could not write {}: {error}", path.display())));
    file.to_owned()
}

fn clear(folder: &Path) {
    if folder.exists() {
        std::fs::remove_dir_all(folder).unwrap_or_else(|error| {
            fail(&format!("could not clear {}: {error}", folder.display()))
        });
    }
    std::fs::create_dir_all(folder)
        .unwrap_or_else(|error| fail(&format!("could not create {}: {error}", folder.display())));
}

fn decode(name: &str, bytes: &[u8]) -> Snapshot {
    Snapshot::decode(bytes).unwrap_or_else(|error| fail(&format!("{name} is unreadable: {error}")))
}

fn git(root: &Path, arguments: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| fail(&format!("could not run git: {error}")));
    if !output.status.success() {
        fail(&format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    output.stdout
}

fn usage() -> ! {
    eprintln!("usage: changed [--base REVISION] [--images [FOLDER]]");
    std::process::exit(2)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
