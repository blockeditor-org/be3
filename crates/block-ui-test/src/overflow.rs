use beui::native::Document;

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
        "the painting {name} shows {} element(s) out of bounds, cut off where nothing scrolls to them, drawn outside their own box, or with an outline cut while their box shows (give a ring room: padding inside the scroll around its content, or `bleed` on a scroll inside a padded surface):\n  {}",
        found.len(),
        found.join("\n  ")
    );
}
