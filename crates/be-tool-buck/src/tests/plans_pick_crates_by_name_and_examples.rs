use crate::plan::{Member, PLANS, builds_programs, cargo_arguments};

#[test]
fn plans_pick_crates_by_name_and_examples() {
    let member = |name: &str, examples: bool| Member {
        name: name.to_owned(),
        examples,
    };
    let members = [
        member("be-app-canvas", true),
        member("be-app-text", false),
        member("be-type-bundle", false),
        member("be-type-text", false),
        member("tabletop-rules-chess", false),
    ];
    let plan = |name: &str| {
        let plan = PLANS.iter().find(|plan| plan.name == name).unwrap();
        cargo_arguments(plan, "TRIPLE", &members).join(" ")
    };
    // The host leaves out every app but one with examples, which run there.
    let host = plan("linux-x86_64");
    assert!(
        host.starts_with(
            "test --target TRIPLE --workspace --exclude be-app-text --exclude be-sdk "
        )
    );
    assert!(!host.contains("be-app-canvas"));
    assert!(plan("wasi-guest").contains("-p be-app-canvas -p be-app-text "));
    assert!(
        !builds_programs("wasi-guest"),
        "the guests' binaries and examples run on the host, not as guests"
    );
    assert!(builds_programs("linux-x86_64"));
    assert!(
        builds_programs("windows-x86_64"),
        "a cross platform is the host's plan"
    );
    let guests = plan("wasi-guest");
    assert!(guests.contains("-p tabletop-rules-chess "));
    assert!(guests.contains("-p be-type-text "));
    assert!(guests.contains("-p be-lib-remap-module "));
    assert!(!guests.contains("be-type-bundle"));
    assert!(
        !guests.contains("be-wasm-gpu-shim"),
        "the gpu shim, which imports JavaScript, has a plan of its own"
    );
    assert!(plan("wasm32-bindgen").starts_with("build --target TRIPLE -p be-wasm-gpu-shim "));
    assert!(
        plan("wasi-html")
            .ends_with("-p beui-host-window --lib --unit-graph -Z unstable-options --locked")
    );
}
