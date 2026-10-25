struct Scene {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    sky: vec4<f32>,
    options: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene;

fn corner(index: u32) -> vec2<f32> {
    return vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
}

fn encode(color: vec3<f32>) -> vec3<f32> {
    if scene.options.x > 0.5 {
        return color;
    }
    let low = color * 12.92;
    let high = 1.055 * pow(color, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, color <= vec3<f32>(0.0031308));
}

const FLOOR_EXTENT: f32 = 60.0;

struct FloorOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn floor_vertex(@builtin(vertex_index) index: u32) -> FloorOut {
    let at = (corner(index) * 2.0 - 1.0) * FLOOR_EXTENT;
    let world = vec3<f32>(scene.eye.x + at.x, 0.0, scene.eye.z - at.y);
    var out: FloorOut;
    out.position = scene.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    return out;
}

@fragment
fn floor_fragment(input: FloorOut) -> @location(0) vec4<f32> {
    let coord = input.world.xz;
    let width = fwidth(coord);
    let cell = abs(fract(coord - 0.5) - 0.5) / max(width, vec2<f32>(1e-4));
    let line = 1.0 - min(min(cell.x, cell.y), 1.0);
    let axis_cell = abs(coord) / max(width, vec2<f32>(1e-4));
    let axis = 1.0 - min(min(axis_cell.x, axis_cell.y), 1.0);
    let distance = length(input.world.xz - scene.eye.xz);
    let fade = 1.0 - smoothstep(8.0, FLOOR_EXTENT * 0.9, distance);
    let detail = 1.0 - smoothstep(0.3, 1.0, max(width.x, width.y));
    let ground = vec3<f32>(0.012, 0.014, 0.018);
    let grid = vec3<f32>(0.10, 0.12, 0.15);
    var color = mix(ground, grid, line * detail);
    color = mix(color, vec3<f32>(0.16, 0.20, 0.30), axis * detail);
    color = mix(scene.sky.rgb, color, fade);
    return vec4<f32>(encode(color), 1.0);
}

const SHARPEN: f32 = -0.5;

struct Panel {
    model: mat4x4<f32>,
    shape: vec4<f32>,
    span: vec4<f32>,
    extra: vec4<f32>,
};

@group(1) @binding(0) var<uniform> panel: Panel;
@group(1) @binding(1) var panel_texture: texture_2d<f32>;
@group(1) @binding(2) var panel_sampler: sampler;

struct PanelOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

const SEGMENTS: u32 = 48u;

@vertex
fn panel_vertex(@builtin(vertex_index) index: u32) -> PanelOut {
    let weight = vec2<f32>(f32(index / 2u) / f32(SEGMENTS), f32(index & 1u));
    let at = mix(panel.span.xy, panel.span.zw, weight);
    let width = panel.shape.x;
    let bend = panel.shape.z;
    let across = (at.x - 0.5) * width;
    let up = (0.5 - at.y) * panel.shape.y;
    var local = vec3<f32>(across, up, panel.shape.w);
    if bend > 1e-4 {
        let radius = width / bend;
        let angle = across / radius;
        let normal = vec3<f32>(-sin(angle), 0.0, cos(angle));
        local = vec3<f32>(radius * sin(angle), up, radius * (1.0 - cos(angle))) + normal * panel.shape.w;
    }
    var out: PanelOut;
    out.position = scene.view_projection * panel.model * vec4<f32>(local, 1.0);
    out.uv = weight;
    return out;
}

@fragment
fn panel_fragment(input: PanelOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let color = textureSampleBias(panel_texture, panel_sampler, input.uv, SHARPEN);
    if !front {
        return vec4<f32>(encode(vec3<f32>(0.05, 0.055, 0.065)), 1.0);
    }
    return vec4<f32>(color.rgb, 1.0);
}

@fragment
fn chrome_fragment(input: PanelOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if !front {
        discard;
    }
    return textureSample(panel_texture, panel_sampler, input.uv) * panel.extra.x;
}

@fragment
fn cursor_fragment(input: PanelOut) -> @location(0) vec4<f32> {
    return textureSample(panel_texture, panel_sampler, input.uv);
}

struct Blit {
    rect: vec4<f32>,
    screen: vec4<f32>,
};

@group(0) @binding(0) var<uniform> blit: Blit;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct BlitOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn blit_vertex(@builtin(vertex_index) index: u32) -> BlitOut {
    let weight = corner(index);
    let point = mix(blit.rect.xy, blit.rect.zw, weight);
    var out: BlitOut;
    out.position = vec4<f32>(point / blit.screen.xy * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = weight;
    return out;
}

@fragment
fn blit_fragment(input: BlitOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source)) - 1;
    let texel = clamp(vec2<i32>(floor(input.position.xy - blit.rect.xy)), vec2<i32>(0), size);
    return textureLoad(source, texel, 0);
}

@vertex
fn mip_vertex(@builtin(vertex_index) index: u32) -> BlitOut {
    let weight = corner(index);
    var out: BlitOut;
    out.position = vec4<f32>(weight * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = weight;
    return out;
}

@fragment
fn mip_fragment(input: BlitOut) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, input.uv);
}
