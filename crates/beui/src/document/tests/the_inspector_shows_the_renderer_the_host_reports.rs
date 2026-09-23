use super::*;
use crate::RendererInfo;

#[test]
fn the_inspector_shows_the_renderer_the_host_reports() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.context.set_renderer_info(RendererInfo {
        adapter: wgpu::AdapterInfo {
            name: "Test adapter".to_owned(),
            vendor: 0x10de,
            device: 0x2684,
            device_type: wgpu::DeviceType::DiscreteGpu,
            device_pci_bus_id: String::new(),
            driver: "Test driver".to_owned(),
            driver_info: "1.2.3".to_owned(),
            backend: wgpu::Backend::Vulkan,
            subgroup_min_size: 32,
            subgroup_max_size: 32,
            transient_saves_memory: false,
        },
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
    });
    harness.toggle_inspector();

    harness.click(harness.performance_tab_center());
    let output = harness.frame(Vec::new());

    let tree = output.accessibility_tree("Test", TALL_VIEWPORT);
    let texts: Vec<String> = tree
        .nodes
        .iter()
        .filter_map(|(_, node)| node.value().or(node.label()).map(str::to_owned))
        .collect();
    for expected in [
        "Vulkan",
        "Test adapter",
        "Test driver",
        "1.2.3",
        "Bgra8UnormSrgb",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected:?} is missing from {texts:?}"
        );
    }
}
