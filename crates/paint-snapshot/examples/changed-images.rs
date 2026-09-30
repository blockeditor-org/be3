use std::path::{Path, PathBuf};

use image::RgbaImage;
use paint_snapshot::{Change, Snapshot};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let root = paint_snapshot::root().unwrap_or_else(|error| fail(&error));
    let (base, output) = match arguments.as_slice() {
        [] => ("HEAD", root.join("target/changed-paintings")),
        [base] => (base.as_str(), root.join("target/changed-paintings")),
        [base, output] => (base.as_str(), PathBuf::from(output)),
        _ => usage(),
    };
    let changes = paint_snapshot::changes(&root, base).unwrap_or_else(|error| fail(&error));

    if output.exists() {
        std::fs::remove_dir_all(&output).unwrap_or_else(|error| {
            fail(&format!("could not clear {}: {error}", output.display()))
        });
    }
    std::fs::create_dir_all(&output)
        .unwrap_or_else(|error| fail(&format!("could not create {}: {error}", output.display())));

    if changes.is_empty() {
        println!("no painting in snapshots/ differs from {base}");
        return;
    }
    println!("the images are in {}", output.display());
    for change in &changes {
        for reason in change.reasons() {
            println!("{}: {reason}", change.name);
        }
        write_frames(change, &output);
    }
}

fn write_frames(change: &Change, output: &Path) {
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
            line += &format!(" {}", save(image, output, &format!("{stem}.before.png")));
        }
        if let Some(image) = &after_image {
            line += &format!(" {}", save(image, output, &format!("{stem}.after.png")));
        }
        if let (Some(before), Some(after)) = (&before_image, &after_image) {
            let highlight = paint_snapshot::highlight(before, after);
            let path = save(&highlight.image, output, &format!("{stem}.changes.png"));
            line += &format!(" {path} ({} pixels differ)", highlight.changed);
        }
        println!("{line}");
    }
}

fn render(snapshot: &Snapshot, index: usize) -> Option<RgbaImage> {
    paint_snapshot::render(snapshot, index)
        .inspect_err(|error| eprintln!("frame {} did not render: {error}", index + 1))
        .ok()
}

fn save(image: &RgbaImage, output: &Path, file: &str) -> String {
    let path = output.join(file);
    image
        .save(&path)
        .unwrap_or_else(|error| fail(&format!("could not write {}: {error}", path.display())));
    file.to_owned()
}

fn usage() -> ! {
    eprintln!("usage: changed-images [BASE [OUTPUT_DIR]]");
    std::process::exit(2)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
