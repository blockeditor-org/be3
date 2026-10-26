struct Uniforms {
    screen: vec2<f32>,
    origin: vec2<f32>,
};

struct Space {
    translation: vec2<f32>,
    opacity: f32,
    padding: f32,
    clip: vec4<f32>,
    fade: vec4<f32>,
    widths: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var colors: texture_2d<f32>;
@group(1) @binding(0) var<uniform> space: Space;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) turn: vec4<f32>,
};

struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rect: vec4<f32>,
    @location(4) clip: vec4<f32>,
    @location(5) params: vec4<f32>,
    @location(6) @interpolate(flat) segment: vec4<f32>,
    @location(7) @interpolate(flat) turn: vec4<f32>,
};

fn corner(index: u32) -> vec2<f32> {
    let right = index == 1u || index == 4u || index == 5u;
    let bottom = index == 2u || index == 3u || index == 5u;
    return vec2<f32>(select(0.0, 1.0, right), select(0.0, 1.0, bottom));
}

fn segment_distance(point: vec2<f32>, segment_start: vec2<f32>, segment_end: vec2<f32>) -> f32 {
    let span = segment_end - segment_start;
    let length = dot(span, span);
    if length <= 0.0 {
        return distance(point, segment_start);
    }
    let t = clamp(dot(point - segment_start, span) / length, 0.0, 1.0);
    return distance(point, segment_start + span * t);
}

fn turned(point: vec2<f32>, turn: vec4<f32>, direction: f32) -> vec2<f32> {
    let offset = point - turn.xy;
    let cosine = turn.z;
    let sine = turn.w * direction;
    return turn.xy + vec2<f32>(
        offset.x * cosine - offset.y * sine,
        offset.x * sine + offset.y * cosine,
    );
}

fn rounded_distance(point: vec2<f32>, extent: vec2<f32>, radius: f32) -> f32 {
    let limit = min(radius, min(extent.x, extent.y));
    let offset = abs(point) - extent + vec2<f32>(limit);
    return min(max(offset.x, offset.y), 0.0) + length(max(offset, vec2<f32>(0.0))) - limit;
}

fn corner_radius(offset: vec2<f32>, radii: vec4<f32>) -> f32 {
    if offset.y < 0.0 {
        return select(radii.y, radii.x, offset.x < 0.0);
    }
    return select(radii.z, radii.w, offset.x < 0.0);
}

fn ramp(distance: vec2<f32>, width: vec2<f32>) -> vec2<f32> {
    let ramped = clamp(distance / max(width, vec2<f32>(1.0e-6)), vec2<f32>(0.0), vec2<f32>(1.0));
    return select(vec2<f32>(1.0), ramped, width > vec2<f32>(0.0));
}

fn faded(point: vec2<f32>) -> f32 {
    let near = ramp(point - space.fade.xy, space.widths.xy);
    let far = ramp(space.fade.zw - point, space.widths.zw);
    return near.x * near.y * far.x * far.y * space.opacity;
}

fn cross2(left: vec2<f32>, right: vec2<f32>) -> f32 {
    return left.x * right.y - left.y * right.x;
}

fn outward(start: vec2<f32>, end: vec2<f32>, winding: f32) -> vec2<f32> {
    let span = end - start;
    let length = max(length(span), 1.0e-6);
    return vec2<f32>(span.y, -span.x) / length * winding;
}

fn dilated(corner: vec2<f32>, before: vec2<f32>, after: vec2<f32>, winding: f32) -> vec2<f32> {
    let incoming = outward(before, corner, winding);
    let outgoing = outward(corner, after, winding);
    var shift = (incoming + outgoing) / max(1.0 + dot(incoming, outgoing), 1.0e-3);
    let reach = length(shift);
    if reach > 3.0 {
        shift = shift * (3.0 / reach);
    }
    return corner + shift;
}

fn edge_coverage(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>, winding: f32) -> f32 {
    let span = end - start;
    let inside = winding * cross2(span, point - start) / max(length(span), 1.0e-6);
    return clamp(inside + 0.5, 0.0, 1.0);
}

fn owns(pixel: vec2<f32>, start: vec2<f32>, end: vec2<f32>, winding: f32) -> bool {
    let swap = start.x > end.x || (start.x == end.x && start.y > end.y);
    let low = select(start, end, swap);
    let high = select(end, start, swap);
    let side = cross2(high - low, pixel - low);
    let facing = select(winding, -winding, swap);
    return side * facing > 0.0 || (side == 0.0 && facing > 0.0);
}

fn triangle_vertex(index: u32, instance: Instance) -> Fragment {
    let a = instance.rect.xy + space.translation;
    let b = instance.rect.zw + space.translation;
    let c = instance.uv.xy + space.translation;
    let winding = sign(cross2(b - a, c - a));
    var point = dilated(c, b, a, winding);
    if index == 0u {
        point = dilated(a, c, b, winding);
    } else if index == 1u {
        point = dilated(b, a, c, winding);
    }
    let clip = instance.clip + vec4<f32>(space.translation, space.translation);

    var fragment: Fragment;
    fragment.position = vec4<f32>(
        (point - uniforms.origin) / uniforms.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    fragment.point = point;
    fragment.uv = vec2<f32>(0.0);
    fragment.color = instance.color;
    fragment.rect = vec4<f32>(0.0);
    fragment.clip = vec4<f32>(max(clip.xy, space.clip.xy), min(clip.zw, space.clip.zw));
    fragment.params = instance.params;
    fragment.segment = vec4<f32>(c, winding, round(instance.params.x));
    fragment.turn = vec4<f32>(a, b);
    return fragment;
}

fn triangle_coverage(input: Fragment) -> f32 {
    let a = input.turn.xy;
    let b = input.turn.zw;
    let c = input.segment.xy;
    let winding = input.segment.z;
    let edges = u32(input.segment.w);
    let pixel = input.position.xy;
    var corners = array<vec2<f32>, 3>(a - uniforms.origin, b - uniforms.origin, c - uniforms.origin);
    var ends = array<vec2<f32>, 3>(a, b, c);
    let point = input.point;
    var coverage = 1.0;
    for (var side = 0u; side < 3u; side = side + 1u) {
        let next = (side + 1u) % 3u;
        if (edges & (1u << side)) != 0u {
            coverage = coverage * edge_coverage(point, ends[side], ends[next], winding);
        } else if !owns(pixel, corners[side], corners[next], winding) {
            return 0.0;
        }
    }
    let inside = winding * cross2(b - a, point - a) >= 0.0
        && winding * cross2(c - b, point - b) >= 0.0
        && winding * cross2(a - c, point - c) >= 0.0;
    if inside {
        return coverage;
    }
    let gap = min(
        segment_distance(point, a, b),
        min(segment_distance(point, b, c), segment_distance(point, c, a)),
    );
    return min(coverage, clamp(0.5 - gap, 0.0, 1.0));
}

@vertex
fn vertex(@builtin(vertex_index) index: u32, instance: Instance) -> Fragment {
    if instance.params.z > 3.5 {
        return triangle_vertex(index, instance);
    }
    let shift = vec4<f32>(space.translation, space.translation);
    let rect = instance.rect + shift;
    let clip = instance.clip + shift;
    let turn = vec4<f32>(instance.turn.xy + space.translation, instance.turn.zw);
    let line = instance.params.z > 2.5;
    let samples = instance.params.z > 0.5;
    let bleed = select(1.0, 0.0, samples);
    let low = rect.xy - vec2<f32>(bleed);
    let high = rect.zw + vec2<f32>(bleed);
    let weight = corner(index);
    let point = turned(mix(low, high, weight), turn, 1.0);

    var fragment: Fragment;
    fragment.position = vec4<f32>(
        (point - uniforms.origin) / uniforms.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    fragment.point = point;
    fragment.uv = mix(instance.uv.xy, instance.uv.zw, weight);
    fragment.color = instance.color;
    fragment.rect = rect;
    fragment.clip = vec4<f32>(max(clip.xy, space.clip.xy), min(clip.zw, space.clip.zw));
    fragment.params = instance.params;
    fragment.segment = select(instance.uv, instance.uv + shift, line);
    fragment.turn = turn;
    return fragment;
}

@fragment
fn fragment(input: Fragment) -> @location(0) vec4<f32> {
    if input.point.x < input.clip.x
        || input.point.y < input.clip.y
        || input.point.x > input.clip.z
        || input.point.y > input.clip.w {
        discard;
    }

    let fade = faded(input.point);
    if input.params.z > 3.5 {
        return vec4<f32>(input.color.rgb, input.color.a * triangle_coverage(input) * fade);
    }
    let local = turned(input.point, input.turn, -1.0);
    var coverage = 1.0;
    if input.params.z > 2.5 {
        let reach = segment_distance(input.point, input.segment.xy, input.segment.zw);
        let edge = clamp(input.params.x + 0.5 - reach, 0.0, 1.0);
        return vec4<f32>(input.color.rgb, input.color.a * edge * fade);
    }
    if input.params.z > 1.5 {
        let texel = textureSampleLevel(atlas, atlas_sampler, input.uv, 0.0);
        let center = (input.rect.xy + input.rect.zw) * 0.5;
        let extent = (input.rect.zw - input.rect.xy) * 0.5;
        let distance = rounded_distance(local - center, extent, input.params.x);
        let edge = clamp(0.5 - distance, 0.0, 1.0);
        return vec4<f32>(texel.rgb * input.color.rgb, texel.a * input.color.a * edge * fade);
    }
    if input.params.z > 0.5 && input.params.w > 0.5 {
        let texel = textureSampleLevel(colors, atlas_sampler, input.uv, 0.0);
        return vec4<f32>(texel.rgb, texel.a * input.color.a * fade);
    }
    if input.params.z > 0.5 {
        coverage = textureSampleLevel(atlas, atlas_sampler, input.uv, 0.0).r;
    } else {
        let center = (input.rect.xy + input.rect.zw) * 0.5;
        let extent = (input.rect.zw - input.rect.xy) * 0.5;
        let offset = local - center;
        let radius = corner_radius(offset, input.segment);
        var distance = rounded_distance(offset, extent, radius);
        let width = input.params.y;
        if width > 0.0 {
            distance = abs(distance + width * 0.5) - width * 0.5;
        }
        coverage = clamp(0.5 - distance, 0.0, 1.0);
    }

    return vec4<f32>(input.color.rgb, input.color.a * coverage * fade);
}
