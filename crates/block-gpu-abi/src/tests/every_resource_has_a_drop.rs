use super::*;

#[test]
fn every_resource_has_a_drop() {
    let names: Vec<&str> = imports().iter().map(|(name, _, _)| *name).collect();
    for resource in [
        "buffer",
        "texture",
        "texture-view",
        "sampler",
        "bind-group-layout",
        "bind-group",
        "pipeline-layout",
        "shader-module",
        "render-pipeline",
        "command-encoder",
        "command-buffer",
        "render-pass",
        "compute-pipeline",
        "compute-pass",
        "surface",
    ] {
        let drop = format!("[resource-drop]{resource}");
        assert!(names.contains(&drop.as_str()), "{drop} is not imported");
    }
}
