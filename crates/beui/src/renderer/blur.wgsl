struct Blur {
    texel: vec4<f32>,
    bounds: vec4<f32>,
};

@group(0) @binding(0) var<uniform> blur: Blur;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct Full {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn full(@builtin(vertex_index) index: u32) -> Full {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var output: Full;
    output.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    output.uv = vec2<f32>(x, y);
    return output;
}

fn tap(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(source, source_sampler, clamp(uv, blur.bounds.xy, blur.bounds.zw));
}

@fragment
fn downsample(input: Full) -> @location(0) vec4<f32> {
    let step = blur.texel.xy * 0.5 * blur.texel.z;
    var sum = tap(input.uv) * 4.0;
    sum += tap(input.uv - step);
    sum += tap(input.uv + step);
    sum += tap(input.uv + vec2<f32>(step.x, -step.y));
    sum += tap(input.uv - vec2<f32>(step.x, -step.y));
    return sum / 8.0;
}

@fragment
fn upsample(input: Full) -> @location(0) vec4<f32> {
    let step = blur.texel.xy * 0.5 * blur.texel.z;
    var sum = tap(input.uv + vec2<f32>(-step.x * 2.0, 0.0));
    sum += tap(input.uv + vec2<f32>(-step.x, step.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(0.0, step.y * 2.0));
    sum += tap(input.uv + vec2<f32>(step.x, step.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(step.x * 2.0, 0.0));
    sum += tap(input.uv + vec2<f32>(step.x, -step.y)) * 2.0;
    sum += tap(input.uv + vec2<f32>(0.0, -step.y * 2.0));
    sum += tap(input.uv + vec2<f32>(-step.x, -step.y)) * 2.0;
    return sum / 12.0;
}
