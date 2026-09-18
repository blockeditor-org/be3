struct Combine {
    region: vec4<f32>,
    row0: vec4<f32>,
    row1: vec4<f32>,
    row2: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> combine: Combine;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var blurred: texture_2d<f32>;
@group(0) @binding(3) var scene_sampler: sampler;

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

fn to_gamma(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = pow(linear, vec3<f32>(1.0 / 2.4)) * 1.055 - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

fn to_linear(gamma: vec3<f32>) -> vec3<f32> {
    let low = gamma / 12.92;
    let high = pow((gamma + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, gamma <= vec3<f32>(0.04045));
}

@fragment
fn compose(input: Full) -> @location(0) vec4<f32> {
    let region = combine.region;
    let untouched = textureSample(scene, scene_sampler, input.uv);
    if input.uv.x < region.x || input.uv.y < region.y
        || input.uv.x > region.z || input.uv.y > region.w {
        return untouched;
    }
    let sampled = textureSample(blurred, scene_sampler, clamp(input.uv, region.xy, region.zw));
    let stored_linear = combine.params.y > 0.5;
    var linear = clamp(sampled.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if !stored_linear {
        linear = to_linear(linear);
    }
    linear = clamp(
        vec3<f32>(
            dot(combine.row0.xyz, linear),
            dot(combine.row1.xyz, linear),
            dot(combine.row2.xyz, linear),
        ),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    var encoded = to_gamma(linear);
    encoded = clamp(
        (encoded - vec3<f32>(0.5)) * combine.params.x + vec3<f32>(0.5),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    var result = encoded;
    if stored_linear {
        result = to_linear(encoded);
    }
    return vec4<f32>(result, sampled.a);
}
