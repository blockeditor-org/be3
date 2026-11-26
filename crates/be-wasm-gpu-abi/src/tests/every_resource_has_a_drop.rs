use super::*;

#[test]
fn every_resource_has_a_drop() {
    let names: Vec<&str> = imports().iter().map(|(name, _, _)| *name).collect();
    for resource in [
        "gpu-buffer",
        "gpu-texture",
        "gpu-texture-view",
        "gpu-sampler",
        "gpu-bind-group-layout",
        "gpu-bind-group",
        "gpu-pipeline-layout",
        "gpu-shader-module",
        "gpu-render-pipeline",
        "gpu-command-encoder",
        "gpu-command-buffer",
        "gpu-render-pass-encoder",
        "gpu-compute-pipeline",
        "gpu-compute-pass-encoder",
        "gpu-query-set",
        "gpu-render-bundle-encoder",
        "gpu-render-bundle",
        "gpu-canvas-context",
    ] {
        let drop = format!("[resource-drop]{resource}");
        assert!(names.contains(&drop.as_str()), "{drop} is not imported");
    }
}
