use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesktopEntry {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub name: String,
    pub generic_name: String,
    pub comment: String,
    pub keywords: Vec<String>,
    pub icon: Option<String>,
    pub exec: String,
    pub try_exec: Option<String>,
    pub working_dir: Option<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub hidden: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
}

impl DesktopEntry {
    pub fn parse(id: &str, path: &str, text: &str, locales: &[String]) -> Option<Self> {
        let keys = main_group(text)?;
        let value = |key: &str| localized(&keys, key, locales).map(unescape);
        let plain = |key: &str| keys.get(key).map(|raw| unescape(raw));
        let flag = |key: &str| keys.get(key).is_some_and(|raw| raw.trim() == "true");
        let list = |key: &str| keys.get(key).map(|raw| split_list(raw)).unwrap_or_default();
        Some(Self {
            id: id.to_owned(),
            path: path.to_owned(),
            kind: plain("Type").unwrap_or_default(),
            name: value("Name")?,
            generic_name: value("GenericName").unwrap_or_default(),
            comment: value("Comment").unwrap_or_default(),
            keywords: localized(&keys, "Keywords", locales)
                .map(split_list)
                .unwrap_or_default(),
            icon: plain("Icon").filter(|icon| !icon.is_empty()),
            exec: plain("Exec").unwrap_or_default(),
            try_exec: plain("TryExec").filter(|exec| !exec.is_empty()),
            working_dir: plain("Path").filter(|path| !path.is_empty()),
            terminal: flag("Terminal"),
            no_display: flag("NoDisplay"),
            hidden: flag("Hidden"),
            only_show_in: list("OnlyShowIn"),
            not_show_in: list("NotShowIn"),
        })
    }

    pub fn shown_in(&self, desktops: &[String]) -> bool {
        if self.no_display || self.hidden || self.kind != "Application" || self.exec.is_empty() {
            return false;
        }
        let ours = |listed: &[String]| listed.iter().any(|name| desktops.contains(name));
        if !self.only_show_in.is_empty() && !ours(&self.only_show_in) {
            return false;
        }
        !ours(&self.not_show_in)
    }
}

fn main_group(text: &str) -> Option<HashMap<&str, &str>> {
    let mut keys = HashMap::new();
    let mut inside = false;
    let mut seen = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            inside = &line[1..line.len() - 1] == "Desktop Entry";
            seen |= inside;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            keys.entry(key.trim()).or_insert(value.trim_start());
        }
    }
    seen.then_some(keys)
}

fn localized<'a>(keys: &HashMap<&str, &'a str>, key: &str, locales: &[String]) -> Option<&'a str> {
    locales
        .iter()
        .find_map(|locale| keys.get(format!("{key}[{locale}]").as_str()))
        .or_else(|| keys.get(key))
        .copied()
}

pub fn locales(messages: &str) -> Vec<String> {
    let (rest, modifier) = match messages.split_once('@') {
        Some((rest, modifier)) => (rest, Some(modifier)),
        None => (messages, None),
    };
    let rest = rest.split('.').next().unwrap_or_default();
    let (lang, country) = match rest.split_once('_') {
        Some((lang, country)) => (lang, Some(country)),
        None => (rest, None),
    };
    if lang.is_empty() || lang == "C" || lang == "POSIX" {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let (Some(country), Some(modifier)) = (country, modifier) {
        out.push(format!("{lang}_{country}@{modifier}"));
    }
    if let Some(country) = country {
        out.push(format!("{lang}_{country}"));
    }
    if let Some(modifier) = modifier {
        out.push(format!("{lang}@{modifier}"));
    }
    out.push(lang.to_owned());
    out
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(next) = chars.next() {
        if next != '\\' {
            out.push(next);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn split_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars();
    while let Some(next) = chars.next() {
        match next {
            '\\' => match chars.next() {
                Some(';') => current.push(';'),
                Some(other) => {
                    current.push('\\');
                    current.push(other);
                }
                None => current.push('\\'),
            },
            ';' => out.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    out.push(current);
    out.into_iter()
        .map(|item| unescape(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}
