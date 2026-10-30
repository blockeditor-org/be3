struct Region {
    offset: vec2<f32>,
    scale: vec2<f32>,
    corner_0: vec2<f32>,
    corner_1: vec2<f32>,
    corner_2: vec2<f32>,
    corner_3: vec2<f32>,
    opacity: f32,
    size_x: f32,
    size_y: f32,
    corner_radius: f32,
};

struct PunchOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) corner_radius: f32,
};

@group(0) @binding(0)
var<uniform> region: Region;

@vertex
fn punch_vertex(@builtin(vertex_index) index: u32) -> PunchOutput {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    var corners = array<vec2<f32>, 4>(
        region.corner_0,
        region.corner_1,
        region.corner_2,
        region.corner_3,
    );
    let corner = order[index];
    var output: PunchOutput;
    output.position = vec4<f32>(corners[corner], 0.0, 1.0);
    output.uv = region.offset + uvs[corner] * region.scale;
    output.size = vec2<f32>(region.size_x, region.size_y);
    output.corner_radius = region.corner_radius;
    return output;
}

@fragment
fn punch_fragment(input: PunchOutput) -> @location(0) vec4<f32> {
    let half = input.size * 0.5;
    let corner = min(input.corner_radius, min(half.x, half.y));
    let inside = abs(input.uv * input.size - half) - (half - vec2<f32>(corner));
    let distance = length(max(inside, vec2<f32>(0.0))) + min(max(inside.x, inside.y), 0.0) - corner;
    if corner > 0.0 && distance > 0.0 {
        discard;
    }
    return vec4<f32>(0.0);
}
