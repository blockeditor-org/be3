use crate::DemoShell;

#[test]
fn a_sample_carries_the_source_it_was_written_with() {
    let file = include_str!("../lib.rs");
    assert!(
        DemoShell::SOURCE.starts_with("#[component]\nfn DemoShell() -> NodeId {\n    let theme"),
        "the source starts at the attributes after #[sample]: {}",
        DemoShell::SOURCE
    );
    assert!(
        DemoShell::SOURCE.ends_with("    }\n}"),
        "the source ends with the function's closing brace"
    );
    assert!(
        file.contains(DemoShell::SOURCE),
        "the source is the function exactly as it is written in the file"
    );
}
