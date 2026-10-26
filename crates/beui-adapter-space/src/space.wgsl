struct Scene {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    grid: vec4<f32>,
    axis: vec4<f32>,
    floor: vec4<f32>,
    inverse: mat4x4<f32>,
    options: vec4<f32>,
    beams: array<vec4<f32>, 6>,
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

struct BackgroundOut {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn background_vertex(@builtin(vertex_index) index: u32) -> BackgroundOut {
    let ndc = corner(index) * 2.0 - 1.0;
    var out: BackgroundOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

fn fade(distance: f32) -> f32 {
    return 1.0 - smoothstep(scene.floor.y, scene.floor.z, distance);
}

@fragment
fn background_fragment(input: BackgroundOut) -> @location(0) vec4<f32> {
    let far = scene.inverse * vec4<f32>(input.ndc, 0.5, 1.0);
    let direction = far.xyz / far.w - scene.eye.xyz;
    let down = -direction.y;
    let along = length(direction.xz);
    var share = 0.0;
    if down > 0.0 {
        share = fade(scene.floor.w * along / down);
    }
    let color = mix(scene.sky.rgb, scene.ground.rgb, share);
    return vec4<f32>(encode(color), 1.0);
}

struct GridOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn grid_vertex(@builtin(vertex_index) index: u32) -> GridOut {
    let at = (corner(index) * 2.0 - 1.0) * scene.floor.x;
    let world = vec3<f32>(scene.eye.x + at.x, 0.0, scene.eye.z - at.y);
    var out: GridOut;
    out.position = scene.view_projection * vec4<f32>(world, 1.0);
    out.world = world;
    return out;
}

@fragment
fn grid_fragment(input: GridOut) -> @location(0) vec4<f32> {
    let coord = input.world.xz;
    let width = fwidth(coord);
    let cell = abs(fract(coord - 0.5) - 0.5) / max(width, vec2<f32>(1e-4));
    let line = 1.0 - min(min(cell.x, cell.y), 1.0);
    let axis_cell = abs(coord) / max(width, vec2<f32>(1e-4));
    let axis = 1.0 - min(min(axis_cell.x, axis_cell.y), 1.0);
    let detail = 1.0 - smoothstep(0.3, 1.0, max(width.x, width.y));
    let shown = fade(length(input.world.xz - scene.eye.xz)) * detail;
    let weight = max(line, axis);
    let color = mix(scene.grid.rgb, scene.axis.rgb, axis / max(weight, 1e-4));
    let alpha = weight * shown;
    return vec4<f32>(encode(color) * alpha, alpha);
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

struct BeamOut {
    @builtin(position) position: vec4<f32>,
    @location(0) at: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn beam_vertex(@builtin(vertex_index) index: u32, @builtin(instance_index) instance: u32) -> BeamOut {
    let beam = instance % 2u;
    let dot = instance >= 2u;
    let start = scene.beams[beam * 3u];
    let end = scene.beams[beam * 3u + 1u];
    var out: BeamOut;
    out.color = scene.beams[beam * 3u + 2u];
    let c = corner(index);
    let across = c.y * 2.0 - 1.0;
    var world: vec3<f32>;
    if dot {
        let toward = normalize(scene.eye.xyz - end.xyz);
        let side = c.x * 2.0 - 1.0;
        world = end.xyz + toward * 0.002 + (scene.right.xyz * side + scene.up.xyz * across) * end.w;
        out.at = vec3<f32>(side, across, 1.0);
        if end.w <= 0.0 {
            out.color = vec4<f32>(0.0);
        }
    } else {
        let point = mix(start.xyz, end.xyz, c.x);
        let along = end.xyz - start.xyz;
        let side = normalize(cross(along, scene.eye.xyz - point));
        world = point + side * across * start.w / 2.0;
        out.at = vec3<f32>(c.x, across, 0.0);
        if start.w <= 0.0 {
            out.color = vec4<f32>(0.0);
        }
    }
    out.position = scene.view_projection * vec4<f32>(world, 1.0);
    return out;
}

@fragment
fn beam_fragment(input: BeamOut) -> @location(0) vec4<f32> {
    var shape: f32;
    if input.at.z > 0.5 {
        shape = 1.0 - smoothstep(0.7, 1.0, length(input.at.xy));
    } else {
        shape = (1.0 - smoothstep(0.4, 1.0, abs(input.at.y))) * mix(0.15, 1.0, input.at.x);
    }
    let alpha = input.color.a * shape;
    return vec4<f32>(encode(input.color.rgb) * alpha, alpha);
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
