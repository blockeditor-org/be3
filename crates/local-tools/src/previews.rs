use beui_html::elements as html;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use beui_html::{Children, Node, component, render_fragment, view};
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
        sections.push(SectionData {
            name: name.clone(),
            headline: headline.to_string(),
            detail: detail.to_string(),
            rows: table,
        });
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
            comment.push_str(&render_fragment(|| {
                view! {
                    <Section data={section} open={open} />
                }
            }));
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
) -> Result<Vec<RowData>, String> {
    let count = |snapshot: Option<&Snapshot>| snapshot.map_or(0, |snapshot| snapshot.frames.len());
    let frames = count(old).max(count(new));
    let mut table_rows = Vec::new();
    for index in 0..frames {
        let old_frame = old.and_then(|snapshot| snapshot.frames.get(index));
        let new_frame = new.and_then(|snapshot| snapshot.frames.get(index));
        if old_frame.is_some() && old_frame == new_frame {
            continue;
        }
        if *rows >= MAX_ROWS {
            table_rows.push(RowData {
                number: index + 1,
                before: None,
                after: None,
                changes: None,
                truncated: true,
            });
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
                Some((cell, label))
            }
            _ => None,
        };
        let cell = |rendered: &Option<image::RgbaImage>, kind: &str| match rendered {
            Some(rendered) => image(kind, rendered, output).map(Some),
            None => Ok(None),
        };
        table_rows.push(RowData {
            number: index + 1,
            before: cell(&old_image, "before")?,
            after: cell(&new_image, "after")?,
            changes,
            truncated: false,
        });
    }
    Ok(table_rows)
}

struct SectionData {
    name: String,
    headline: String,
    detail: String,
    rows: Vec<RowData>,
}

struct RowData {
    number: usize,
    before: Option<ImageData>,
    after: Option<ImageData>,
    changes: Option<(ImageData, String)>,
    truncated: bool,
}

struct ImageData {
    alt: String,
    src: String,
}

#[component]
fn Section(data: SectionData, open: bool) -> Node {
    let detail: Children<Node> = if data.detail.is_empty() {
        view! {}
    } else {
        view! {
            <html::Sub>{data.detail}</html::Sub>
        }
        .into()
    };
    view! {
        <html::Details open={open}>
            <html::Summary>
                <html::Code>{data.name}</html::Code>
                " "
                {data.headline}
            </html::Summary>
            {detail}
            <Table rows={data.rows} />
        </html::Details>
    }
}

#[component]
fn Table(rows: Vec<RowData>) -> Node {
    let rows: Children<Node> = rows
        .into_iter()
        .map(|row| {
            view! {
                <Row data={row} />
            }
        })
        .collect();
    view! {
        <html::Table>
            <html::Tr>
                <html::Th>"Frame"</html::Th>
                <html::Th>"Before"</html::Th>
                <html::Th>"After"</html::Th>
                <html::Th>"Changes"</html::Th>
            </html::Tr>
            {rows}
        </html::Table>
    }
}

#[component]
fn Row(data: RowData) -> Node {
    if data.truncated {
        return view! {
            <html::Tr>
                <html::Td>{data.number.to_string()}</html::Td>
                <html::Td colspan=3>"more frames not shown"</html::Td>
            </html::Tr>
        };
    }
    let changes: Children<Node> = match data.changes {
        Some((image, label)) => view! {
            <Image data={image} />
            <html::Br />
            {label}
        },
        None => view! {},
    };
    view! {
        <html::Tr>
            <html::Td>{data.number.to_string()}</html::Td>
            <html::Td>
                <ImageCell image={data.before} />
            </html::Td>
            <html::Td>
                <ImageCell image={data.after} />
            </html::Td>
            <html::Td>{changes}</html::Td>
        </html::Tr>
    }
}

#[component]
fn ImageCell(image: Option<ImageData>) -> Children<Node> {
    match image {
        Some(data) => view! {
            <Image data={data} />
        }
        .into(),
        None => view! {},
    }
}

#[component]
fn Image(data: ImageData) -> Node {
    view! {
        <html::Img alt={data.alt} src={data.src} />
    }
}

fn render(snapshot: &Snapshot, index: usize) -> Option<image::RgbaImage> {
    paint_snapshot::render(snapshot, index)
        .inspect_err(|error| eprintln!("frame {index} did not render: {error}"))
        .ok()
}

fn image(kind: &str, image: &image::RgbaImage, output: &Path) -> Result<ImageData, String> {
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
    Ok(ImageData {
        alt: kind.to_string(),
        src: format!("{ROOT}/{file}"),
    })
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
    beui_html::render_fragment(|| {
        view! {
            {text}
        }
    })
}

#[cfg(test)]
mod tests;
