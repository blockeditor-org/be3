use super::*;

#[test]
fn the_extensions_a_shader_enables_are_read_past_comments() {
    let shader = "// a header\n/* a /* nested */ block */\nrequires readonly_and_readwrite_storage_textures;\n\
                  enable f16, clip_distances ;\ndiagnostic(off, derivative_uniformity);\nenable dual_source_blending;\n\
                  fn main() {}\nenable primitive_index;";
    assert_eq!(
        enabled(shader),
        ["f16", "clip_distances", "dual_source_blending"],
        "directives are read until the first declaration"
    );
}
