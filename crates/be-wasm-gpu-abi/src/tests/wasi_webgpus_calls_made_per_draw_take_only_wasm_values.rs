use super::*;

#[test]
fn wasi_webgpus_calls_made_per_draw_take_only_wasm_values() {
    let signature = |wanted: &str| {
        imports()
            .iter()
            .find(|(name, _, _)| *name == wanted)
            .map(|(_, arguments, result)| (*arguments, *result))
            .unwrap_or_else(|| panic!("{wanted} is not imported"))
    };
    for pass in ["gpu-render-pass-encoder", "gpu-render-bundle-encoder"] {
        assert_eq!(
            signature(&format!("[method]{pass}.set-pipeline")),
            ("u32, u32", "")
        );
        // Each option is a tag and its value.
        assert_eq!(
            signature(&format!("[method]{pass}.draw")),
            ("u32, u32, u32, u32, u32, u32, u32, u32", "")
        );
        assert_eq!(
            signature(&format!("[method]{pass}.set-vertex-buffer")),
            ("u32, u32, u32, u32, u32, u64, u32, u64", "")
        );
        // The dynamic offsets are a tag, a pointer and a length, and an error comes back
        // only when there is one, in the room the last two name.
        assert_eq!(
            signature(&format!("[method]{pass}.set-bind-group")),
            (
                "u32, u32, u32, u32, u32, u32, u32, u32, u64, u32, u32, u32, u32",
                "u32"
            )
        );
    }
    assert_eq!(
        signature("[method]gpu-render-pass-encoder.set-viewport"),
        ("u32, f32, f32, f32, f32, f32, f32", "")
    );
    assert_eq!(
        signature("[method]gpu-compute-pass-encoder.dispatch-workgroups"),
        ("u32, u32, u32, u32, u32, u32", "")
    );
    assert_eq!(signature("[poll]"), ("u32, u32, u32, u32, u32", "u32"));
}
