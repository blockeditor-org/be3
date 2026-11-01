use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use beui_html::{render_fragment, view};
use paint_review_manifest::{Frame, Manifest, Painting, Status};
use paint_snapshot::{BrowserSnapshot, Snapshot};
use sha2::{Digest as _, Sha256};

use crate::{git_bytes, hex};

pub const PAINTINGS: &str = ":(glob)crates/**/snapshots/*.paint";
const ROOT: &str = "PREVIEW_ROOT";
const MAX_FRAMES: usize = 500;
const MAX_LISTED: usize = 100;
const MAX_INLINE_FRAMES: usize = 30;

pub fn stage<'a>(
    paths: impl IntoIterator<Item = &'a str>,
    base: &str,
    head: Option<&str>,
) -> Result<(PathBuf, PathBuf), String> {
    let previews = Path::new("target/previews");
    let (before, after) = (previews.join("before"), previews.join("after"));
    for directory in [&before, &after] {
        let _ = fs::remove_dir_all(directory);
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    for path in paths.into_iter().filter(|line| !line.is_empty()) {
        let name = flat_name(path).ok_or("a painting with no name")?;
        let show = |commit: &str| git_bytes(&["show", &format!("{commit}:{path}")]);
        let new = match head {
            Some(head) => show(head),
            None => fs::read(path).ok(),
        };
        for (contents, directory) in [(show(base), &before), (new, &after)] {
            if let Some(contents) = contents {
                fs::write(directory.join(&name), contents).map_err(|error| error.to_string())?;
            }
        }
    }
    Ok((before, after))
}

fn flat_name(path: &str) -> Option<String> {
    let mut parts = path.rsplit('/');
    let name = parts.next()?;
    let krate = parts.nth(1)?;
    Some(format!("{krate}.{name}"))
}

pub fn generate(
    before: &Path,
    after: &Path,
    output: &Path,
    browser_renderer: Option<&Path>,
    site: Option<&Path>,
) -> Result<String, String> {
    let mut comment = "## Paintings\n\n".to_owned();

    fs::create_dir_all(output).map_err(|error| error.to_string())?;

    let names: BTreeSet<String> = [before, after].into_iter().flat_map(paintings).collect();

    let mut manifest = Manifest::default();
    let mut frames = 0;
    let mut omitted = 0;
    for name in &names {
        let old = read(&before.join(format!("{name}.paint")));
        let new = read(&after.join(format!("{name}.paint")));
        let (status, detail) = match (&old, &new) {
            (None, None) => continue,
            (Some(Err(error)), _) | (_, Some(Err(error))) => (Status::Unreadable, error.clone()),
            (None, Some(Ok(_))) => (Status::Added, String::new()),
            (Some(Ok(_)), None) => (Status::Removed, String::new()),
            (Some(Ok(old)), Some(Ok(new))) => match old.difference(new) {
                None => continue,
                Some(difference) => (Status::Changed, difference),
            },
        };
        let shown = match detail.as_str() {
            "" => String::new(),
            detail => format!(": {}", detail.replace('\n', " ")),
        };
        println!("{name} {}{shown}", described(status));
        let old = old.and_then(Result::ok);
        let new = new.and_then(Result::ok);
        let rendered = differing_frames(
            old.as_ref(),
            new.as_ref(),
            output,
            &mut frames,
            &mut omitted,
            browser_renderer,
        )?;
        manifest.paintings.push(Painting {
            name: name.clone(),
            status,
            detail,
            frames: rendered,
        });
    }

    if manifest.paintings.is_empty() {
        comment.push_str("No paintings changed.\n");
        println!("0 paintings differ");
        return Ok(comment);
    }

    let json = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    let manifest_hash = write_hashed(output, "json", &json)?;
    let page = site
        .map(|site| copy_site(site, output))
        .transpose()?
        .map(|site| format!("{ROOT}/site/{site}/index.html?changes={manifest_hash}"));

    if let Some(page) = &page {
        let _ = writeln!(comment, "**[Review them]({page})**\n");
    }
    comment.push_str("<details><summary>Painting previews</summary>\n\n");
    let count = |status: Status| {
        manifest
            .paintings
            .iter()
            .filter(|painting| painting.status == status)
            .count()
    };
    let total = manifest.paintings.len();
    let _ = write!(
        comment,
        "{} in this pull request: {} changed, {} added, {} removed",
        match total {
            1 => "One painting differs".to_owned(),
            count => format!("{count} paintings differ"),
        },
        count(Status::Changed),
        count(Status::Added),
        count(Status::Removed),
    );
    match count(Status::Unreadable) {
        0 => comment.push_str(".\n\n"),
        unreadable => {
            let _ = write!(comment, ", {unreadable} unreadable.\n\n");
        }
    }
    if omitted > 0 {
        let _ = writeln!(
            comment,
            "{omitted} frames past the first {MAX_FRAMES} were not drawn.\n"
        );
    }
    let mut inline_frames = 0;
    for painting in manifest.paintings.iter().take(MAX_LISTED) {
        let name = match &page {
            Some(page) => format!("[`{0}`]({page}&painting={0})", painting.name),
            None => format!("`{}`", painting.name),
        };
        let status = described(painting.status);
        let detail = match painting.detail.as_str() {
            "" => String::new(),
            detail => format!(": {}", escaped(&detail.replace('\n', " "))),
        };
        let _ = writeln!(comment, "- {name} {status}{detail}");
        if !painting.frames.is_empty() && inline_frames < MAX_INLINE_FRAMES {
            comment.push_str(
                "\n<table><tr><th>Frame</th><th>Before</th><th>After</th><th>Changes</th></tr>\n",
            );
            for frame in &painting.frames {
                if inline_frames >= MAX_INLINE_FRAMES {
                    break;
                }
                inline_frames += 1;
                let _ = write!(comment, "<tr><td>{}</td>", frame.number);
                for (label, hash) in [
                    ("Before", &frame.before),
                    ("After", &frame.after),
                    ("Changes", &frame.changes),
                ] {
                    match hash {
                        Some(hash) => {
                            let _ = write!(
                                comment,
                                "<td><img alt=\"{label}\" src=\"{ROOT}/{hash}.png\" width=\"320\"></td>"
                            );
                        }
                        None => comment.push_str("<td></td>"),
                    }
                }
                comment.push_str("</tr>\n");
            }
            comment.push_str("</table>\n\nMagenta marks every pixel that differs.\n\n");
        }
    }
    if total > MAX_LISTED {
        let _ = writeln!(comment, "- and {} more", total - MAX_LISTED);
    }
    let available_frames: usize = manifest
        .paintings
        .iter()
        .map(|painting| painting.frames.len())
        .sum();
    if available_frames > inline_frames {
        let _ = writeln!(
            comment,
            "\n{} more frames are available on the review page; inline previews stop at {MAX_INLINE_FRAMES} frames.",
            available_frames - inline_frames
        );
    }
    comment.push_str("\n</details>\n");
    println!("{total} paintings differ");
    Ok(comment)
}

fn described(status: Status) -> &'static str {
    match status {
        Status::Changed => "changed",
        Status::Added => "was added",
        Status::Removed => "was removed",
        Status::Unreadable => "is unreadable",
    }
}

fn differing_frames(
    old: Option<&ReviewSnapshot>,
    new: Option<&ReviewSnapshot>,
    output: &Path,
    drawn: &mut usize,
    omitted: &mut usize,
    browser_renderer: Option<&Path>,
) -> Result<Vec<Frame>, String> {
    let count =
        |snapshot: Option<&ReviewSnapshot>| snapshot.map_or(0, |snapshot| snapshot.frame_count());
    let frames = count(old).max(count(new));
    let mut listed = Vec::new();
    for index in 0..frames {
        let old_frame = old.filter(|snapshot| index < snapshot.frame_count());
        let new_frame = new.filter(|snapshot| index < snapshot.frame_count());
        if matches!((old_frame, new_frame), (Some(old), Some(new)) if old.same_frame(new, index)) {
            continue;
        }
        if *drawn >= MAX_FRAMES {
            *omitted += 1;
            continue;
        }
        *drawn += 1;
        let old_image = old_frame
            .map(|snapshot| snapshot.render(index, output, browser_renderer))
            .transpose()?;
        let new_image = new_frame
            .map(|snapshot| snapshot.render(index, output, browser_renderer))
            .transpose()?;
        let (changes, changed_pixels) = match (&old_image, &new_image) {
            (Some(old), Some(new)) => {
                let highlight = paint_snapshot::highlight(old, new);
                (
                    Some(image(&highlight.image, output)?),
                    Some(highlight.changed as u64),
                )
            }
            _ => (None, None),
        };
        let frame = Frame {
            number: index + 1,
            before: old_image
                .as_ref()
                .map(|old| image(old, output))
                .transpose()?,
            after: new_image
                .as_ref()
                .map(|new| image(new, output))
                .transpose()?,
            changes,
            changed_pixels,
        };
        let mut line = format!("  frame {}:", frame.number);
        if let (Some(ReviewSnapshot::Native(old)), Some(ReviewSnapshot::Native(new))) =
            (old_frame, new_frame)
            && old.frames.len() > 1
        {
            let what = paint_snapshot::frame_difference(&old.frames[index], &new.frames[index]);
            let _ = write!(line, " {what};");
        }
        for (label, hash) in [
            ("before", &frame.before),
            ("after", &frame.after),
            ("changes", &frame.changes),
        ] {
            if let Some(hash) = hash {
                let _ = write!(line, " {label} {hash}.png");
            }
        }
        if let Some(pixels) = frame.changed_pixels {
            let _ = write!(line, " ({pixels} pixels differ)");
        }
        println!("{line}");
        listed.push(frame);
    }
    Ok(listed)
}

fn copy_site(site: &Path, output: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let entries = fs::read_dir(site.join(&relative))
            .map_err(|error| format!("could not read {}: {error}", site.display()))?;
        for entry in entries.flatten() {
            let path = relative.join(entry.file_name());
            if site.join(&path).is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut hasher = Sha256::new();
    let mut contents = Vec::new();
    for file in files {
        let bytes = fs::read(site.join(&file))
            .map_err(|error| format!("could not read {}: {error}", file.display()))?;
        let name = file.to_string_lossy().replace('\\', "/");
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(Sha256::digest(&bytes));
        contents.push((file, bytes));
    }
    let hash = hex(&hasher.finalize());
    let root = output.join("site").join(&hash);
    for (file, bytes) in contents {
        let path = root.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&path, bytes)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    }
    Ok(hash)
}

enum ReviewSnapshot {
    Native(Snapshot),
    Browser(BrowserSnapshot, PathBuf),
}

impl ReviewSnapshot {
    fn frame_count(&self) -> usize {
        match self {
            Self::Native(snapshot) => snapshot.frames.len(),
            Self::Browser(..) => 1,
        }
    }

    fn same_frame(&self, other: &Self, index: usize) -> bool {
        match (self, other) {
            (Self::Native(old), Self::Native(new)) => {
                old.frames.get(index) == new.frames.get(index)
            }
            (Self::Browser(old, _), Self::Browser(new, _)) => old == new,
            _ => false,
        }
    }

    fn difference(&self, other: &Self) -> Option<String> {
        match (self, other) {
            (Self::Native(old), Self::Native(new)) => {
                paint_snapshot::difference(old, new).map(|difference| difference.description)
            }
            (Self::Browser(old, _), Self::Browser(new, _)) if old == new => None,
            (Self::Browser(..), Self::Browser(..)) => {
                Some("browser document, viewport or embedded files changed".into())
            }
            _ => Some("painting format changed".into()),
        }
    }

    fn render(
        &self,
        index: usize,
        output: &Path,
        renderer: Option<&Path>,
    ) -> Result<image::RgbaImage, String> {
        match self {
            Self::Native(snapshot) => paint_snapshot::render(snapshot, index),
            Self::Browser(_, path) => {
                let png = output.join("browser-render.png");
                let renderer = renderer.ok_or("the browser preview rasterizer was not built")?;
                crate::browser::rasterize(path, &png, Some(renderer))?;
                let result = image::open(&png)
                    .map(|image| image.to_rgba8())
                    .map_err(|error| error.to_string());
                let _ = std::fs::remove_file(png);
                result
            }
        }
    }
}

fn image(image: &image::RgbaImage, output: &Path) -> Result<String, String> {
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| format!("could not encode an image: {error}"))?;
    write_hashed(output, "png", &png)
}

fn write_hashed(output: &Path, extension: &str, bytes: &[u8]) -> Result<String, String> {
    let hash = hex(&Sha256::digest(bytes));
    let path = output.join(format!("{hash}.{extension}"));
    fs::write(&path, bytes)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(hash)
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

fn read(path: &Path) -> Option<Result<ReviewSnapshot, String>> {
    let bytes = std::fs::read(path).ok()?;
    Some(if BrowserSnapshot::is_browser(&bytes) {
        BrowserSnapshot::decode(&bytes)
            .map(|snapshot| ReviewSnapshot::Browser(snapshot, path.to_owned()))
    } else {
        Snapshot::decode(&bytes).map(ReviewSnapshot::Native)
    })
}

fn escaped(text: &str) -> String {
    render_fragment(|| {
        view! {
            {text}
        }
    })
}

#[cfg(test)]
mod tests;
