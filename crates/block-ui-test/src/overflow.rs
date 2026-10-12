use beui::Document;

pub(crate) fn overflows(document: &Document) -> Vec<String> {
    document
        .overflows()
        .iter()
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn assert_fits(name: &str, mut found: Vec<String>) {
    found.sort();
    found.dedup();
    if found.is_empty() {
        return;
    }
    panic!(
        "the painting {name} shows {} element(s) out of bounds, cut off where nothing scrolls to them or drawn outside their own box:\n  {}",
        found.len(),
        found.join("\n  ")
    );
}
