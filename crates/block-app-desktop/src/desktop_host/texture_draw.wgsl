struct Placed {
    rect: vec4<f32>,
    uv: vec4<f32>,
    screen: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> placed: Placed;
@group(0) @binding(1)
var shown_texture: texture_2d<f32>;
@group(0) @binding(2)
var shown_sampler: sampler;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Output {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let point = mix(placed.rect.xy, placed.rect.zw, corner);
    var output: Output;
    output.position = vec4<f32>(
        point.x / placed.screen.x * 2.0 - 1.0,
        1.0 - point.y / placed.screen.y * 2.0,
        0.0,
        1.0,
    );
    output.uv = mix(placed.uv.xy, placed.uv.zw, corner);
    return output;
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    return textureSampleLevel(shown_texture, shown_sampler, input.uv, 0.0);
}
