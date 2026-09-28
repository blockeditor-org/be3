struct BlitUniform {
    rect: vec4<f32>,
    clip: vec4<f32>,
    screen: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> blit: BlitUniform;
@group(0) @binding(1) var scene_texture: texture_2d<f32>;
@group(0) @binding(2) var scene_sampler: sampler;

struct BlitOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) point: vec2<f32>,
};

fn corner(index: u32) -> vec2<f32> {
    let right = index == 1u || index == 4u || index == 5u;
    let bottom = index == 2u || index == 3u || index == 5u;
    return vec2<f32>(select(0.0, 1.0, right), select(0.0, 1.0, bottom));
}

@vertex
fn blit_vertex(@builtin(vertex_index) index: u32) -> BlitOutput {
    let weight = corner(index);
    let point = mix(blit.rect.xy, blit.rect.zw, weight);
    var output: BlitOutput;
    output.position = vec4<f32>(
        point / blit.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    output.uv = weight;
    output.point = point;
    return output;
}

@fragment
fn blit_fragment(input: BlitOutput) -> @location(0) vec4<f32> {
    if input.point.x < blit.clip.x || input.point.y < blit.clip.y
        || input.point.x > blit.clip.z || input.point.y > blit.clip.w {
        discard;
    }
    return textureSample(scene_texture, scene_sampler, input.uv);
}
