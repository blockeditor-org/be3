use super::*;

#[test]
fn the_calls_made_per_draw_take_only_wasm_values() {
    let signature = |wanted: &str| {
        imports()
            .iter()
            .find(|(name, _, _)| *name == wanted)
            .map(|(_, arguments, result)| (*arguments, *result))
            .unwrap_or_else(|| panic!("{wanted} is not imported"))
    };
    assert_eq!(
        signature("[method]render-pass.set-pipeline"),
        ("u32, u32", "")
    );
    assert_eq!(
        signature("[method]render-pass.set-viewport"),
        ("u32, f32, f32, f32, f32, f32, f32", "")
    );
    assert_eq!(
        signature("[method]render-pass.set-vertex-buffer"),
        ("u32, u32, u32, u64, u32, u64", "")
    );
    assert_eq!(
        signature("[method]render-pass.draw-indexed"),
        ("u32, u32, u32, u32, i32, u32", "")
    );
    assert_eq!(
        signature("[method]compute-pass.set-pipeline"),
        ("u32, u32", "")
    );
    assert_eq!(
        signature("[method]compute-pass.dispatch-workgroups"),
        ("u32, u32, u32, u32", "")
    );
    assert_eq!(
        signature("[method]compute-pass.dispatch-workgroups-indirect"),
        ("u32, u32, u64", "")
    );
    assert_eq!(
        signature("[method]render-pass.draw-indirect"),
        ("u32, u32, u64", "")
    );
    assert_eq!(
        signature("[method]render-pass.begin-occlusion-query"),
        ("u32, u32", "")
    );
    assert_eq!(
        signature("[method]render-bundle-encoder.draw"),
        ("u32, u32, u32, u32, u32", "")
    );
    assert_eq!(
        signature("[method]render-bundle-encoder.set-vertex-buffer"),
        ("u32, u32, u32, u64, u32, u64", "")
    );
    assert_eq!(signature("create-buffer"), ("u32, u32", "u32"));
    assert_eq!(signature("take-error"), ("u32, u32", "u32"));
}
