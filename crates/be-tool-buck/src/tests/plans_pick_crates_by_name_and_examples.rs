use crate::plan::{Member, PLANS, cargo_arguments};

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
    let wasm32 = plan("wasm32");
    assert!(wasm32.starts_with(
        "build --target TRIPLE -p tabletop-rules-chess -p be-type-text -p be-wasm-gpu-shim"
    ));
    assert!(!wasm32.contains("be-type-bundle"));
    assert!(
        plan("wasi-html")
            .ends_with("-p beui-demo-html --lib --unit-graph -Z unstable-options --locked")
    );
}
