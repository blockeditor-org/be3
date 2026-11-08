//! Links run on a worker (remote_linking_cxx_toolchain in
//! buck/toolchains/defs.bzl), which nothing else would notice stopped: the
//! prelude decides where from more than the toolchain's fields. buck2's event
//! log says, for every action the build ran or found in a cache, whether it
//! preferred or required this machine; a Rust link that did is the prelude
//! having moved links here.

/// Each Rust link in the event log, written as JSON lines, that would run on
/// this machine, as "TARGET (IDENTIFIER)".
pub fn local_links(events: &str) -> Vec<String> {
    let mut local = Vec::new();
    for line in events.lines() {
        if !line.contains("\"ActionExecution\"") || !line.contains("\"rustc\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let action = &event["Event"]["data"]["SpanEnd"]["data"]["ActionExecution"];
        let name = &action["name"];
        let identifier = name["identifier"].as_str().unwrap_or_default();
        if name["category"] != "rustc" || !identifier.starts_with("link") {
            continue;
        }
        if action["prefers_local"] != true && action["requires_local"] != true {
            continue;
        }
        let label = &action["key"]["owner"]["TargetLabel"]["label"];
        local.push(format!(
            "{}:{} ({identifier})",
            label["package"].as_str().unwrap_or("?"),
            label["name"].as_str().unwrap_or("?"),
        ));
    }
    local
}
