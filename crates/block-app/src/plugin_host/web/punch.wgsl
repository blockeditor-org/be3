struct Region {
    offset: vec2<f32>,
    scale: vec2<f32>,
    corner_0: vec2<f32>,
    corner_1: vec2<f32>,
    corner_2: vec2<f32>,
    corner_3: vec2<f32>,
    opacity: f32,
    padding_0: f32,
    padding_1: f32,
    padding_2: f32,
};

@group(0) @binding(0)
var<uniform> region: Region;

@vertex
fn punch_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    var corners = array<vec2<f32>, 4>(
        region.corner_0,
        region.corner_1,
        region.corner_2,
        region.corner_3,
    );
    return vec4<f32>(corners[order[index]], 0.0, 1.0);
}

@fragment
fn punch_fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
