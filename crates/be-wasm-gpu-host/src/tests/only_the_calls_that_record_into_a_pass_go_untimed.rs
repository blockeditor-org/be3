use crate::only_records;

#[test]
fn only_the_calls_that_record_into_a_pass_go_untimed() {
    for recording in [
        "[method]render-pass.draw",
        "[method]render-pass.set-pipeline",
        "[method]render-pass.execute-bundles",
        "[method]compute-pass.dispatch-workgroups",
        "[method]render-bundle-encoder.draw-indexed",
    ] {
        assert!(only_records(recording), "{recording} only records");
    }
    for working in [
        "[method]render-pass.end",
        "[method]compute-pass.end",
        "[method]render-bundle-encoder.finish",
        "[method]command-encoder.begin-render-pass",
        "[resource-drop]render-pass",
        "submit",
        "create-texture",
    ] {
        assert!(!only_records(working), "{working} is timed");
    }
}
