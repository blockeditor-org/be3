use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use paint_snapshot::Snapshot;
use sha2::{Digest as _, Sha256};

const MARKER: &str = "<!-- paint-previews -->";
const ROOT: &str = "PREVIEW_ROOT";
const MAX_ROWS: usize = 30;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [before, after, output] = arguments.as_slice() else {
        eprintln!("usage: preview BEFORE_DIR AFTER_DIR OUTPUT_DIR");
        std::process::exit(2)
    };
    let (before, after, output) = (Path::new(before), Path::new(after), Path::new(output));

    std::fs::create_dir_all(output).unwrap_or_else(|error| fail(&error.to_string()));

    let names: BTreeSet<String> = [before, after].into_iter().flat_map(paintings).collect();

    let mut sections = Vec::new();
    let mut unshown = Vec::new();
    let mut rows = 0;
    let (mut added, mut removed, mut changed) = (0, 0, 0);
    for name in &names {
        let old = read(&before.join(format!("{name}.paint")));
        let new = read(&after.join(format!("{name}.paint")));
        let summary = match (&old, &new) {
            (None, None) => continue,
            (Some(Err(error)), _) | (_, Some(Err(error))) => format!("is unreadable: {error}"),
            (None, Some(Ok(_))) => {
                added += 1;
                "was added".to_owned()
            }
            (Some(Ok(_)), None) => {
                removed += 1;
                "was removed".to_owned()
            }
            (Some(Ok(old)), Some(Ok(new))) => match paint_snapshot::difference(old, new) {
                None => continue,
                Some(difference) => {
                    changed += 1;
                    format!("changed: {}", difference.description)
                }
            },
        };
        let old = old.and_then(Result::ok);
        let new = new.and_then(Result::ok);
        if rows >= MAX_ROWS {
            unshown.push(format!("- `{name}` {}", escaped(&summary)));
            continue;
        }
        let table = table(old.as_ref(), new.as_ref(), output, &mut rows);
        let (headline, detail) = summary.split_once(": ").unwrap_or((&summary, ""));
        sections.push(format!(
            "<details{{open}}>\n<summary><code>{name}</code> {}</summary>\n\n{}{table}\n</details>\n",
            escaped(headline),
            match detail {
                "" => String::new(),
                detail => format!("<sub>{}</sub>\n\n", escaped(detail)),
            }
        ));
    }

    let total = added + removed + changed;
    let mut comment = format!("{MARKER}\n## Paintings\n\n");
    if total == 0 {
        comment.push_str("No paintings changed.\n");
    } else {
        let _ = writeln!(
            comment,
            "{} in this pull request: {changed} changed, {added} added, {removed} removed. \
             In the changes column, magenta marks every pixel that differs.\n",
            match total {
                1 => "One painting differs".to_owned(),
                count => format!("{count} paintings differ"),
            }
        );
        let open = if sections.len() <= 5 { " open" } else { "" };
        for section in sections {
            comment.push_str(&section.replace("{open}", open));
            comment.push('\n');
        }
        if !unshown.is_empty() {
            let _ = writeln!(comment, "Not shown, to keep this comment short:\n");
            for line in unshown {
                let _ = writeln!(comment, "{line}");
            }
        }
    }
    std::fs::write(output.join("comment.md"), comment)
        .unwrap_or_else(|error| fail(&format!("could not write the comment: {error}")));
    println!("{total} paintings differ");
}

fn table(
    old: Option<&Snapshot>,
    new: Option<&Snapshot>,
    output: &Path,
    rows: &mut usize,
) -> String {
    let count = |snapshot: Option<&Snapshot>| snapshot.map_or(0, |snapshot| snapshot.frames.len());
    let frames = count(old).max(count(new));
    let mut table = "| Frame | Before | After | Changes |\n|---|---|---|---|\n".to_owned();
    for index in 0..frames {
        let old_frame = old.and_then(|snapshot| snapshot.frames.get(index));
        let new_frame = new.and_then(|snapshot| snapshot.frames.get(index));
        if old_frame.is_some() && old_frame == new_frame {
            continue;
        }
        if *rows >= MAX_ROWS {
            let _ = writeln!(table, "| {} | more frames not shown | | |", index + 1);
            break;
        }
        *rows += 1;
        let old_image = old_frame.and_then(|_| render(old?, index));
        let new_image = new_frame.and_then(|_| render(new?, index));
        let changes = match (&old_image, &new_image) {
            (Some(old), Some(new)) => {
                let highlight = paint_snapshot::highlight(old, new);
                let cell = image("changes", &highlight.image, output);
                match highlight.changed {
                    0 => format!("{cell}<br>no visible difference"),
                    1 => format!("{cell}<br>1 pixel"),
                    count => format!("{cell}<br>{count} pixels"),
                }
            }
            _ => String::new(),
        };
        let cell = |rendered: &Option<image::RgbaImage>, kind: &str| match rendered {
            Some(rendered) => image(kind, rendered, output),
            None => String::new(),
        };
        let _ = writeln!(
            table,
            "| {} | {} | {} | {changes} |",
            index + 1,
            cell(&old_image, "before"),
            cell(&new_image, "after"),
        );
    }
    table
}

fn render(snapshot: &Snapshot, index: usize) -> Option<image::RgbaImage> {
    paint_snapshot::render(snapshot, index)
        .inspect_err(|error| eprintln!("frame {index} did not render: {error}"))
        .ok()
}

fn image(kind: &str, image: &image::RgbaImage, output: &Path) -> String {
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap_or_else(|error| fail(&format!("could not encode a {kind} image: {error}")));
    let hash: String = Sha256::digest(&png)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = format!("{hash}.png");
    let path = output.join(&file);
    std::fs::write(&path, png)
        .unwrap_or_else(|error| fail(&format!("could not write {}: {error}", path.display())));
    format!("![{kind}]({ROOT}/{file})")
}

fn paintings(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            Some(name.strip_suffix(".paint")?.to_owned())
        })
        .filter(|name| {
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        })
        .collect()
}

fn read(path: &Path) -> Option<Result<Snapshot, String>> {
    let bytes = std::fs::read(path).ok()?;
    Some(Snapshot::decode(&bytes))
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
