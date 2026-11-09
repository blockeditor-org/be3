struct Opacity {
    value: vec4<f32>,
};

@group(0) @binding(0) var<uniform> opacity: Opacity;
@group(0) @binding(1) var layer: texture_2d<f32>;

@vertex
fn full(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

// The layer holds premultiplied colour, as the ui pipeline's blending leaves it over
// transparent pixels, so fading it scales every channel.
@fragment
fn compose(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(layer, vec2<i32>(position.xy), 0) * opacity.value.x;
}
