use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use beui_html::{Node, fragment, text};
use paint_snapshot::Snapshot;
use sha2::{Digest as _, Sha256};

const ROOT: &str = "PREVIEW_ROOT";
const MAX_ROWS: usize = 30;

pub fn generate(before: &Path, after: &Path, output: &Path) -> Result<String, String> {
    let mut comment = "## Paintings\n\n".to_owned();

    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;

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
        let table = table(old.as_ref(), new.as_ref(), output, &mut rows)?;
        let (headline, detail) = summary.split_once(": ").unwrap_or((&summary, ""));
        sections.push(
            Node::element("details")
                .child(
                    Node::element("summary")
                        .child(Node::element("code").child(name.as_str()))
                        .child(" ")
                        .child(headline),
                )
                .child(if detail.is_empty() {
                    fragment([])
                } else {
                    Node::element("sub").child(detail)
                })
                .child(table),
        );
    }

    let total = added + removed + changed;
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
        let open = sections.len() <= 5;
        for section in sections {
            comment.push_str(&section.flag("open", open).to_html());
            comment.push('\n');
        }
        if !unshown.is_empty() {
            let _ = writeln!(comment, "Not shown, to keep this comment short:\n");
            for line in unshown {
                let _ = writeln!(comment, "{line}");
            }
        }
    }
    println!("{total} paintings differ");
    Ok(comment)
}

fn table(
    old: Option<&Snapshot>,
    new: Option<&Snapshot>,
    output: &Path,
    rows: &mut usize,
) -> Result<Node, String> {
    let count = |snapshot: Option<&Snapshot>| snapshot.map_or(0, |snapshot| snapshot.frames.len());
    let frames = count(old).max(count(new));
    let mut table = Node::element("table").child(Node::element("tr").child(fragment(
        ["Frame", "Before", "After", "Changes"].map(|title| Node::element("th").child(title)),
    )));
    for index in 0..frames {
        let old_frame = old.and_then(|snapshot| snapshot.frames.get(index));
        let new_frame = new.and_then(|snapshot| snapshot.frames.get(index));
        if old_frame.is_some() && old_frame == new_frame {
            continue;
        }
        if *rows >= MAX_ROWS {
            table = table.child(
                Node::element("tr")
                    .child(Node::element("td").child((index + 1).to_string()))
                    .child(
                        Node::element("td")
                            .attr("colspan", "3")
                            .child("more frames not shown"),
                    ),
            );
            break;
        }
        *rows += 1;
        let old_image = old_frame.and_then(|_| render(old?, index));
        let new_image = new_frame.and_then(|_| render(new?, index));
        let changes = match (&old_image, &new_image) {
            (Some(old), Some(new)) => {
                let highlight = paint_snapshot::highlight(old, new);
                let cell = image("changes", &highlight.image, output)?;
                let label = match highlight.changed {
                    0 => "no visible difference".to_string(),
                    1 => "1 pixel".to_string(),
                    count => format!("{count} pixels"),
                };
                fragment([cell, Node::element("br"), text(label)])
            }
            _ => fragment([]),
        };
        let cell = |rendered: &Option<image::RgbaImage>, kind: &str| match rendered {
            Some(rendered) => image(kind, rendered, output),
            None => Ok(fragment([])),
        };
        table = table.child(
            Node::element("tr")
                .child(Node::element("td").child((index + 1).to_string()))
                .child(Node::element("td").child(cell(&old_image, "before")?))
                .child(Node::element("td").child(cell(&new_image, "after")?))
                .child(Node::element("td").child(changes)),
        );
    }
    Ok(table)
}

fn render(snapshot: &Snapshot, index: usize) -> Option<image::RgbaImage> {
    paint_snapshot::render(snapshot, index)
        .inspect_err(|error| eprintln!("frame {index} did not render: {error}"))
        .ok()
}

fn image(kind: &str, image: &image::RgbaImage, output: &Path) -> Result<Node, String> {
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| format!("could not encode a {kind} image: {error}"))?;
    let hash: String = Sha256::digest(&png)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = format!("{hash}.png");
    let path = output.join(&file);
    std::fs::write(&path, png)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(Node::element("img")
        .attr("alt", kind)
        .attr("src", format!("{ROOT}/{file}")))
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
    beui_html::text(text).to_html()
}

#[cfg(test)]
mod tests;
