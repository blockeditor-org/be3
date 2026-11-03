use crate::known_window::KnownWindow;

pub fn readable_id(program_id: &str) -> String {
    let id = program_id.trim();
    let file = id.rsplit(['/', '\\']).next().unwrap_or(id);
    let file = match file.len().checked_sub(4).and_then(|cut| file.get(cut..)) {
        Some(extension) if extension.eq_ignore_ascii_case(".exe") => &file[..file.len() - 4],
        _ => file,
    };
    let package = match file.split_once('!') {
        Some((family, _)) => family.split('_').next().unwrap_or(family),
        None => file,
    };
    let last = package.rsplit('.').next().unwrap_or(package);
    let words = last.replace(['-', '_'], " ");
    let mut letters = words.trim().chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

fn without_name<'a>(title: &'a str, name: &str) -> &'a str {
    if name.is_empty() {
        return title;
    }
    for separator in [" - ", " \u{2014} ", " \u{2013} ", " | "] {
        let Some(cut) = title.len().checked_sub(separator.len() + name.len()) else {
            continue;
        };
        let (Some(kept), Some(tail)) = (title.get(..cut), title.get(cut..)) else {
            continue;
        };
        if tail.starts_with(separator) && tail[separator.len()..].eq_ignore_ascii_case(name) {
            return kept.trim();
        }
    }
    title
}

pub fn label(name: &str, title: &str, program_id: &str) -> String {
    let title = without_name(title.trim(), name);
    let titled =
        !title.is_empty() && !title.eq_ignore_ascii_case(name) && title != program_id.trim();
    match (name.is_empty(), titled) {
        (true, false) => "Window".to_owned(),
        (true, true) => title.to_owned(),
        (false, false) => name.to_owned(),
        (false, true) => format!("{name} \u{2014} {title}"),
    }
}

pub fn program_name(window: &KnownWindow, parent: Option<&KnownWindow>) -> String {
    let named =
        |window: &KnownWindow| Some(window.name.trim().to_owned()).filter(|name| !name.is_empty());
    let read =
        |window: &KnownWindow| Some(readable_id(&window.program_id)).filter(|name| !name.is_empty());
    named(window)
        .or_else(|| parent.and_then(named))
        .or_else(|| read(window))
        .or_else(|| parent.and_then(read))
        .unwrap_or_default()
}

pub fn window_name(window: &KnownWindow, parent: Option<&KnownWindow>) -> String {
    label(
        &program_name(window, parent),
        &window.title,
        &window.program_id,
    )
}

#[cfg(test)]
mod tests;
