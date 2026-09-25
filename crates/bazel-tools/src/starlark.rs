pub(crate) fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serializes")
}
