use super::*;

#[test]
fn only_the_calls_that_record_into_a_pass_go_untimed() {
    for recording in [
        "[method]gpu-render-pass-encoder.draw",
        "[method]gpu-render-pass-encoder.set-pipeline",
        "[method]gpu-render-pass-encoder.execute-bundles",
        "[method]gpu-compute-pass-encoder.dispatch-workgroups",
        "[method]gpu-render-bundle-encoder.draw-indexed",
    ] {
        assert!(only_records(recording), "{recording} only records");
    }
    for working in [
        "[method]gpu-render-pass-encoder.end",
        "[method]gpu-compute-pass-encoder.end",
        "[method]gpu-render-bundle-encoder.finish",
        "[method]gpu-command-encoder.begin-render-pass",
        "[resource-drop]gpu-render-pass-encoder",
        "[method]gpu-queue.submit",
        "[method]gpu-device.create-texture",
    ] {
        assert!(!only_records(working), "{working} is timed");
    }
    assert!(submits("[method]gpu-queue.submit"));
}
