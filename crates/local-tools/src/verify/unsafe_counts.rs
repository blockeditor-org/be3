use std::collections::BTreeMap;

const CATEGORIES: [&str; 4] = ["ffi", "memory", "env", "send-sync"];
const UNTAGGED: &str = "untagged";

pub type Counts = BTreeMap<String, usize>;

pub fn count<'a>(sources: impl IntoIterator<Item = &'a str>) -> Counts {
    let mut counts = Counts::new();
    for source in sources {
        for site in fix_rust_source::unsafe_sites(source) {
            let category = site.category.unwrap_or_else(|| UNTAGGED.to_owned());
            *counts.entry(category).or_default() += 1;
        }
    }
    counts
}

pub fn summary(before: &Counts, after: &Counts) -> String {
    let mut names: Vec<&str> = CATEGORIES.to_vec();
    for name in before.keys().chain(after.keys()) {
        if !names.contains(&name.as_str()) {
            names.push(name);
        }
    }
    let parts: Vec<String> = names
        .into_iter()
        .filter_map(|name| {
            let old = before.get(name).copied().unwrap_or(0);
            let new = after.get(name).copied().unwrap_or(0);
            if old == 0 && new == 0 && !CATEGORIES.contains(&name) {
                return None;
            }
            Some(match new as i64 - old as i64 {
                0 => format!("{name} {new}"),
                change => format!("{name} {new} (**{change:+}**)"),
            })
        })
        .collect();
    format!(
        "## Unsafe\n\nUnsafe sites by category: {}. Only ffi is unavoidable.\n\n",
        parts.join(", ")
    )
}
