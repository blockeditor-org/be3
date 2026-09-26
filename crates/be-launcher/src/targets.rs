use serde_json::Value;

pub(crate) const QUERY: &str = "//... - //third-party/... - //buck/... - //external/...";
const HIDDEN: [&str; 4] = [
    "export_file",
    "prebuilt_cxx_library",
    "rust_proc_macro_alias",
    "configured_alias",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Run,
    Test,
    Build,
}

impl Action {
    pub(crate) fn verb(self) -> &'static str {
        match self {
            Action::Run => "run",
            Action::Test => "test",
            Action::Build => "build",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Target {
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) action: Action,
}

pub(crate) fn parse_targets(listed: &Value) -> Vec<Target> {
    let mut targets: Vec<Target> = listed
        .as_object()
        .map(|listed| {
            listed
                .iter()
                .filter_map(|(label, attributes)| {
                    let kind = attributes["buck.type"].as_str()?;
                    if HIDDEN.contains(&kind) {
                        return None;
                    }
                    let label = label.strip_prefix("root").unwrap_or(label).to_owned();
                    let runnable_app = kind == "app" && !attributes["binary"].is_null();
                    let action = match kind {
                        "rust_test" | "wasi_test" => Action::Test,
                        "tool" => Action::Run,
                        "rust_binary" if !label.ends_with(":test_module") => Action::Run,
                        "app" if runnable_app => Action::Run,
                        _ => Action::Build,
                    };
                    Some(Target {
                        label,
                        kind: kind.to_owned(),
                        action,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    targets.sort_by(|left, right| left.label.cmp(&right.label));
    targets
}
