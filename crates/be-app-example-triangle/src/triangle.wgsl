struct Corner {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn corner(@builtin(vertex_index) index: u32) -> Corner {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.8),
        vec2<f32>(-0.8, -0.6),
        vec2<f32>(0.8, -0.6),
    );
    var colors = array<vec3<f32>, 3>(
        vec3<f32>(0.90, 0.30, 0.30),
        vec3<f32>(0.30, 0.80, 0.40),
        vec3<f32>(0.30, 0.45, 0.90),
    );
    var out: Corner;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    out.color = colors[index];
    return out;
}

@fragment
fn fill(corner: Corner) -> @location(0) vec4<f32> {
    return vec4<f32>(corner.color, 1.0);
}
