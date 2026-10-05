struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opaque: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opaque: f32,
};

@vertex
fn surface_vertex(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.opaque = input.opaque;
    return output;
}

@group(0) @binding(0)
var surface_texture: texture_2d<f32>;
@group(0) @binding(1)
var surface_sampler: sampler;

@fragment
fn surface_fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSampleLevel(surface_texture, surface_sampler, input.uv, 0.0);
    if input.opaque > 0.5 {
        return vec4<f32>(color.rgb, 1.0);
    }
    return color;
}
