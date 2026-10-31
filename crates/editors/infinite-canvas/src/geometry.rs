use block_editor_beui::beui::{Pos2, Rect, Vec2, pos2};
use block_editor_beui::block_ui::{
    EMBEDDED_EDITOR_PADDING, EMBEDDED_EDITOR_TITLE_GAP, EMBEDDED_EDITOR_TITLE_HEIGHT,
};
use block_editor_beui::{ResizeMode, block_ui};
use infinite_canvas_types::{
    CanvasArrowhead, CanvasEntity, CanvasPathPoint, CanvasPoint, CanvasShape, CanvasStyle,
    CanvasTextAlign, CanvasTextStyle, CanvasTextVerticalAlign, CanvasTransform,
};
use uuid::Uuid;

pub(crate) const MIN_SIZE: f32 = 4.0;
pub(crate) const TEXT_INSET: f32 = 4.0;
pub(crate) const HIT_RADIUS: f32 = 7.0;
pub(crate) const HANDLE_RADIUS: f32 = 5.0;
pub(crate) const HANDLE_REACH: f32 = HANDLE_RADIUS + 3.0;
pub(crate) const TOUCH_HANDLE_REACH: f32 = 22.0;
pub(crate) const ROTATE_OFFSET: f32 = 28.0;
pub(crate) const IMPORT_CASCADE_OFFSET: f32 = 24.0;
pub(crate) const FRAME_LABEL_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WorldRect {
    pub(crate) min: CanvasPoint,
    pub(crate) max: CanvasPoint,
}

impl WorldRect {
    pub(crate) fn from_points(a: CanvasPoint, b: CanvasPoint) -> Self {
        Self {
            min: CanvasPoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: CanvasPoint::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub(crate) fn center(self) -> CanvasPoint {
        CanvasPoint::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    pub(crate) fn size(self) -> CanvasPoint {
        CanvasPoint::new(self.max.x - self.min.x, self.max.y - self.min.y)
    }

    pub(crate) fn expanded(self, by: f32) -> Self {
        Self {
            min: CanvasPoint::new(self.min.x - by, self.min.y - by),
            max: CanvasPoint::new(self.max.x + by, self.max.y + by),
        }
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            min: CanvasPoint::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: CanvasPoint::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub(crate) fn contains_rect(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.max.x >= other.max.x
            && self.min.y <= other.min.y
            && self.max.y >= other.max.y
    }

    pub(crate) fn contains(self, point: CanvasPoint) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub(crate) fn rect(self) -> Rect {
        Rect::from_min_max(
            pos2(self.min.x, self.min.y),
            pos2(self.max.x.max(self.min.x), self.max.y.max(self.min.y)),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SelectionFrame {
    pub(crate) center: CanvasPoint,
    pub(crate) size: CanvasPoint,
    pub(crate) rotation: f32,
}

impl SelectionFrame {
    pub(crate) fn from_world_rect(bounds: WorldRect) -> Self {
        Self {
            center: bounds.center(),
            size: bounds.size(),
            rotation: 0.0,
        }
    }

    pub(crate) fn point(self, local: CanvasPoint) -> CanvasPoint {
        CanvasTransform::new(self.center, self.size, self.rotation).to_world(local)
    }

    pub(crate) fn contains(self, point: CanvasPoint) -> bool {
        let local = CanvasTransform::new(self.center, self.size, self.rotation).to_local(point);
        local.x.abs() <= 0.5 && local.y.abs() <= 0.5
    }

    pub(crate) fn local_bounds(self) -> WorldRect {
        WorldRect {
            min: CanvasPoint::new(-self.size.x * 0.5, -self.size.y * 0.5),
            max: CanvasPoint::new(self.size.x * 0.5, self.size.y * 0.5),
        }
    }

    pub(crate) fn corners(self) -> [CanvasPoint; 4] {
        [
            CanvasPoint::new(-0.5, -0.5),
            CanvasPoint::new(0.5, -0.5),
            CanvasPoint::new(0.5, 0.5),
            CanvasPoint::new(-0.5, 0.5),
        ]
        .map(|corner| self.point(corner))
    }

    pub(crate) fn rotate_handle(self) -> CanvasPoint {
        let top = self.point(CanvasPoint::new(0.0, -0.5));
        let angle = self.rotation - std::f32::consts::FRAC_PI_2;
        CanvasPoint::new(
            top.x + angle.cos() * ROTATE_OFFSET,
            top.y + angle.sin() * ROTATE_OFFSET,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResizeHandle {
    pub(crate) x: i8,
    pub(crate) y: i8,
}

#[derive(Clone, Copy)]
pub(crate) struct DirectEditorLayout {
    pub(crate) title_bar: WorldRect,
    pub(crate) content: WorldRect,
}

pub(crate) fn point(point: CanvasPoint) -> Pos2 {
    pos2(point.x, point.y)
}

pub(crate) fn gesture_rect(
    start: CanvasPoint,
    current: CanvasPoint,
    from_center: bool,
) -> WorldRect {
    let opposite = if from_center {
        CanvasPoint::new(start.x * 2.0 - current.x, start.y * 2.0 - current.y)
    } else {
        start
    };
    WorldRect::from_points(opposite, current)
}

pub(crate) fn duplicate_entities(
    entities: Vec<CanvasEntity>,
    offset: CanvasPoint,
) -> Vec<CanvasEntity> {
    let mut duplicate_groups = std::collections::HashMap::new();
    entities
        .into_iter()
        .map(|mut entity| {
            entity.id = Uuid::new_v4();
            if let Some(group_id) = entity.group_id {
                entity.group_id = Some(
                    *duplicate_groups
                        .entry(group_id)
                        .or_insert_with(Uuid::new_v4),
                );
            }
            entity.transform.center.x += offset.x;
            entity.transform.center.y += offset.y;
            entity
        })
        .collect()
}

pub(crate) fn entity_kind_label(entity: &CanvasEntity) -> &'static str {
    match entity.shape {
        CanvasShape::Rectangle => "Rectangle",
        CanvasShape::Ellipse => "Ellipse",
        CanvasShape::Diamond => "Diamond",
        CanvasShape::Path => match entity.style.end_arrowhead != CanvasArrowhead::None
            || entity.style.start_arrowhead != CanvasArrowhead::None
        {
            true => "Arrow",
            false => "Line",
        },
        CanvasShape::Freehand => "Freehand",
        CanvasShape::Text => "Text",
        CanvasShape::Image => "Image",
        CanvasShape::Block => "Block preview",
        CanvasShape::Editor => "Direct editor",
        CanvasShape::Frame => "Frame",
        CanvasShape::Raster => "Raster layer",
    }
}

pub(crate) fn embedded_region(frame: Option<WorldRect>, extent: Option<WorldRect>) -> WorldRect {
    if let Some(frame) = frame {
        return frame;
    }
    let (center, size) = extent.map_or_else(
        || (CanvasPoint::default(), CanvasPoint::new(100.0, 100.0)),
        |bounds| {
            (
                bounds.center(),
                CanvasPoint::new(bounds.size().x.max(100.0), bounds.size().y.max(100.0)),
            )
        },
    );
    WorldRect {
        min: CanvasPoint::new(center.x - size.x * 0.5, center.y - size.y * 0.5),
        max: CanvasPoint::new(center.x + size.x * 0.5, center.y + size.y * 0.5),
    }
}

pub(crate) fn frame_label_hit(entity: &CanvasEntity, point: CanvasPoint, scale: f32) -> bool {
    if !entity.is_frame() {
        return false;
    }
    let bounds = entity_bounds(entity);
    let height = FRAME_LABEL_HEIGHT / scale.max(f32::EPSILON);
    point.x >= bounds.min.x
        && point.x <= bounds.max.x
        && point.y >= bounds.min.y - height
        && point.y <= bounds.min.y
}

pub(crate) fn distance(a: CanvasPoint, b: CanvasPoint) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

pub(crate) fn rotate(point: CanvasPoint, angle: f32) -> CanvasPoint {
    let (sin, cos) = angle.sin_cos();
    CanvasPoint::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

pub(crate) fn path_ends(entity: &CanvasEntity) -> [CanvasPoint; 2] {
    let world = entity.world_points();
    match (world.first(), world.last()) {
        (Some(first), Some(last)) => [first.at(), last.at()],
        _ => [entity.transform.center; 2],
    }
}

pub(crate) fn with_end(entity: CanvasEntity, end: usize, at: CanvasPoint) -> CanvasEntity {
    let index = match end {
        0 => 0,
        _ => entity.points.len().saturating_sub(1),
    };
    with_point(entity, index, at)
}

pub(crate) fn with_point(mut entity: CanvasEntity, index: usize, at: CanvasPoint) -> CanvasEntity {
    let mut world = entity.world_points();
    if let Some(point) = world.get_mut(index) {
        let shift = CanvasPoint::new(at.x - point.x, at.y - point.y);
        *point = point.map(|held| CanvasPoint::new(held.x + shift.x, held.y + shift.y));
    }
    entity.set_world_points(world);
    entity
}

pub(crate) fn path_neighbour(entity: &CanvasEntity, end: usize) -> CanvasPoint {
    let world = entity.world_points();
    let index = match end {
        0 => 1,
        _ => world.len().saturating_sub(2),
    };
    world
        .get(index)
        .map_or(entity.transform.center, |point| point.at())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathGrip {
    Point(usize),
    Middle(usize),
}

pub(crate) const MIDDLE_MIN_LENGTH: f32 = 4.0 * HANDLE_REACH;

fn mix(a: CanvasPoint, b: CanvasPoint) -> CanvasPoint {
    CanvasPoint::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

pub(crate) fn segment_middle(from: CanvasPathPoint, to: CanvasPathPoint) -> CanvasPoint {
    let (a, d) = (from.at(), to.at());
    let b = from.handle_out.unwrap_or(a);
    let c = to.handle_in.unwrap_or(d);
    mix(mix(mix(a, b), mix(b, c)), mix(mix(b, c), mix(c, d)))
}

pub(crate) fn path_middles(entity: &CanvasEntity, scale: f32) -> Vec<(usize, CanvasPoint)> {
    let least = MIDDLE_MIN_LENGTH / scale.max(f32::EPSILON);
    entity
        .world_points()
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| distance(pair[0].at(), pair[1].at()) >= least)
        .map(|(index, pair)| (index, segment_middle(pair[0], pair[1])))
        .collect()
}

pub(crate) fn path_grip_at(
    entity: &CanvasEntity,
    world: CanvasPoint,
    scale: f32,
    reach: f32,
) -> Option<PathGrip> {
    let reach = reach / scale.max(f32::EPSILON);
    let nearest = |grips: Vec<(PathGrip, CanvasPoint)>| {
        grips
            .into_iter()
            .map(|(grip, at)| (grip, distance(at, world)))
            .filter(|(_, away)| *away <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(grip, _)| grip)
    };
    let points = entity
        .world_points()
        .iter()
        .enumerate()
        .map(|(index, point)| (PathGrip::Point(index), point.at()))
        .collect();
    nearest(points).or_else(|| {
        nearest(
            path_middles(entity, scale)
                .into_iter()
                .map(|(index, at)| (PathGrip::Middle(index), at))
                .collect(),
        )
    })
}

pub(crate) fn split_segment(mut entity: CanvasEntity, index: usize) -> CanvasEntity {
    let mut world = entity.world_points();
    let (Some(from), Some(to)) = (world.get(index).copied(), world.get(index + 1).copied()) else {
        return entity;
    };
    let middle = match from.handle_out.is_none() && to.handle_in.is_none() {
        true => CanvasPathPoint::new(segment_middle(from, to)),
        false => {
            let (a, d) = (from.at(), to.at());
            let b = from.handle_out.unwrap_or(a);
            let c = to.handle_in.unwrap_or(d);
            let (ab, bc, cd) = (mix(a, b), mix(b, c), mix(c, d));
            let (abc, bcd) = (mix(ab, bc), mix(bc, cd));
            world[index].handle_out = from.handle_out.map(|_| ab);
            world[index + 1].handle_in = to.handle_in.map(|_| cd);
            CanvasPathPoint {
                handle_in: Some(abc),
                handle_out: Some(bcd),
                ..CanvasPathPoint::new(mix(abc, bcd))
            }
        }
    };
    world.insert(index + 1, middle);
    entity.set_world_points(world);
    entity
}

fn polygon_contains(polygon: &[CanvasPoint], point: CanvasPoint) -> bool {
    let mut inside = false;
    for (index, a) in polygon.iter().enumerate() {
        let b = polygon[(index + 1) % polygon.len()];
        if (a.y > point.y) != (b.y > point.y)
            && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x)
        {
            inside = !inside;
        }
    }
    inside
}

fn segments_cross(a: CanvasPoint, b: CanvasPoint, c: CanvasPoint, d: CanvasPoint) -> bool {
    let side = |p: CanvasPoint, q: CanvasPoint, r: CanvasPoint| {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    };
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

pub(crate) fn lasso_encloses(lasso: &[CanvasPoint], entity: &CanvasEntity) -> bool {
    if lasso.len() < 3 {
        return false;
    }
    let bounds = lasso
        .iter()
        .map(|point| WorldRect::from_points(*point, *point))
        .reduce(WorldRect::union)
        .expect("a lasso has points");
    if !bounds.contains_rect(entity_bounds(entity)) {
        return false;
    }
    let (outline, closed) = match entity.shape {
        CanvasShape::Path | CanvasShape::Freehand => (flattened(&entity.world_points()), false),
        _ => (outline_points(entity), true),
    };
    if outline.is_empty() || !outline.iter().all(|point| polygon_contains(lasso, *point)) {
        return false;
    }
    let edges = outline.len() - usize::from(!closed);
    (0..edges).all(|index| {
        let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
        (0..lasso.len())
            .all(|edge| !segments_cross(a, b, lasso[edge], lasso[(edge + 1) % lasso.len()]))
    })
}

pub(crate) fn flattened(world: &[CanvasPathPoint]) -> Vec<CanvasPoint> {
    let mut out: Vec<CanvasPoint> = world.first().map(|first| first.at()).into_iter().collect();
    for pair in world.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from.handle_out.is_none() && to.handle_in.is_none() {
            out.push(to.at());
            continue;
        }
        let a = from.at();
        let b = from.handle_out.unwrap_or(a);
        let d = to.at();
        let c = to.handle_in.unwrap_or(d);
        for step in 1..=CURVE_STEPS {
            let t = step as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            out.push(CanvasPoint::new(
                weights[0] * a.x + weights[1] * b.x + weights[2] * c.x + weights[3] * d.x,
                weights[0] * a.y + weights[1] * b.y + weights[2] * c.y + weights[3] * d.y,
            ));
        }
    }
    out
}

const CURVE_STEPS: usize = 16;

pub(crate) fn entity_corners(entity: &CanvasEntity) -> [CanvasPoint; 4] {
    [
        entity.transform.to_world(CanvasPoint::new(-0.5, -0.5)),
        entity.transform.to_world(CanvasPoint::new(0.5, -0.5)),
        entity.transform.to_world(CanvasPoint::new(0.5, 0.5)),
        entity.transform.to_world(CanvasPoint::new(-0.5, 0.5)),
    ]
}

pub(crate) fn entity_bounds(entity: &CanvasEntity) -> WorldRect {
    let points: Vec<CanvasPoint> = match entity.shape.has_points() && !entity.points.is_empty() {
        true => entity
            .world_points()
            .iter()
            .flat_map(|point| [Some(point.at()), point.handle_in, point.handle_out])
            .flatten()
            .collect(),
        false => entity_corners(entity).to_vec(),
    };
    points
        .into_iter()
        .map(|point| WorldRect::from_points(point, point))
        .reduce(WorldRect::union)
        .unwrap()
}

pub(crate) fn direct_editor_entity_size(intrinsic: Vec2, scale: f32) -> CanvasPoint {
    let (width, height) = block_ui::embedded_editor_frame(intrinsic.x, intrinsic.y, scale);
    CanvasPoint::new(width.max(MIN_SIZE), height.max(MIN_SIZE))
}

pub(crate) fn direct_editor_to_preview(
    entity: &CanvasEntity,
    block_id: uuid::Uuid,
) -> Option<CanvasEntity> {
    let content = direct_editor_layout(entity)?.content;
    let content_size = content.size();
    let mut preview = entity.clone();
    preview.shape = CanvasShape::Block;
    preview.block = Some(block_id);
    preview.transform.center = content.center();
    preview.transform.size =
        CanvasPoint::new(content_size.x.max(MIN_SIZE), content_size.y.max(MIN_SIZE));
    preview.transform.rotation = 0.0;
    Some(preview)
}

pub(crate) fn preview_to_direct_editor(
    entity: &CanvasEntity,
    block_id: uuid::Uuid,
    intrinsic: Vec2,
) -> CanvasEntity {
    let content = entity_bounds(entity);
    let content_size = content.size();
    let scale = (content_size.x / intrinsic.x)
        .min(content_size.y / intrinsic.y)
        .max(f32::EPSILON);
    let mut direct = entity.clone();
    direct.shape = CanvasShape::Editor;
    direct.block = Some(block_id);
    direct.scale = scale;
    direct.transform.center = CanvasPoint::new(
        content.center().x,
        content.center().y
            - (EMBEDDED_EDITOR_TITLE_HEIGHT + EMBEDDED_EDITOR_TITLE_GAP) * scale * 0.5,
    );
    direct.transform.size =
        direct_editor_entity_size(Vec2::new(content_size.x, content_size.y), scale);
    direct.transform.rotation = 0.0;
    direct
}

pub(crate) fn direct_editor_layout(entity: &CanvasEntity) -> Option<DirectEditorLayout> {
    if entity.shape != CanvasShape::Editor {
        return None;
    }
    let scale = entity.scale;
    let bounds = entity_bounds(entity);
    let padding = EMBEDDED_EDITOR_PADDING * scale;
    let title_height = EMBEDDED_EDITOR_TITLE_HEIGHT * scale;
    let title_gap = EMBEDDED_EDITOR_TITLE_GAP * scale;
    Some(DirectEditorLayout {
        title_bar: WorldRect {
            min: CanvasPoint::new(bounds.min.x + padding, bounds.min.y + padding),
            max: CanvasPoint::new(
                bounds.max.x - padding,
                bounds.min.y + padding + title_height,
            ),
        },
        content: WorldRect {
            min: CanvasPoint::new(
                bounds.min.x + padding,
                bounds.min.y + padding + title_height + title_gap,
            ),
            max: CanvasPoint::new(bounds.max.x - padding, bounds.max.y - padding),
        },
    })
}

pub(crate) fn hit_entity(entity: &CanvasEntity, point: CanvasPoint, radius: f32) -> bool {
    let reach = radius + entity.style.stroke_width.max(0.0) * 0.5;
    match entity.shape {
        CanvasShape::Path | CanvasShape::Freehand => {
            let line = flattened(&entity.world_points());
            match line.as_slice() {
                [] => false,
                [dot] => distance(point, *dot) <= reach,
                _ => line
                    .windows(2)
                    .any(|segment| point_segment_distance(point, segment[0], segment[1]) <= reach),
            }
        }
        CanvasShape::Rectangle
        | CanvasShape::Ellipse
        | CanvasShape::Diamond
        | CanvasShape::Frame
            if entity.style.filled().is_none() || entity.is_frame() =>
        {
            let outline = outline_points(entity);
            outline
                .iter()
                .zip(outline.iter().cycle().skip(1))
                .any(|(a, b)| point_segment_distance(point, *a, *b) <= reach)
        }
        CanvasShape::Ellipse | CanvasShape::Diamond => {
            let local = entity.transform.to_local(point);
            let grown = CanvasPoint::new(
                local.x * entity.transform.size.x / (entity.transform.size.x + radius * 2.0),
                local.y * entity.transform.size.y / (entity.transform.size.y + radius * 2.0),
            );
            entity.shape.contains_local(grown)
        }
        _ => {
            let local = entity.transform.to_local(point);
            local.x.abs() <= 0.5 + radius / entity.transform.size.x.max(MIN_SIZE)
                && local.y.abs() <= 0.5 + radius / entity.transform.size.y.max(MIN_SIZE)
        }
    }
}

pub(crate) const ELLIPSE_STEPS: usize = 64;

pub(crate) fn outline_points(entity: &CanvasEntity) -> Vec<CanvasPoint> {
    let local: Vec<CanvasPoint> = match entity.shape {
        CanvasShape::Ellipse => (0..ELLIPSE_STEPS)
            .map(|step| {
                let angle = step as f32 / ELLIPSE_STEPS as f32 * std::f32::consts::TAU;
                CanvasPoint::new(angle.cos() * 0.5, angle.sin() * 0.5)
            })
            .collect(),
        CanvasShape::Diamond => vec![
            CanvasPoint::new(0.0, -0.5),
            CanvasPoint::new(0.5, 0.0),
            CanvasPoint::new(0.0, 0.5),
            CanvasPoint::new(-0.5, 0.0),
        ],
        _ => vec![
            CanvasPoint::new(-0.5, -0.5),
            CanvasPoint::new(0.5, -0.5),
            CanvasPoint::new(0.5, 0.5),
            CanvasPoint::new(-0.5, 0.5),
        ],
    };
    local
        .into_iter()
        .map(|point| entity.transform.to_world(point))
        .collect()
}

pub(crate) fn point_segment_distance(point: CanvasPoint, a: CanvasPoint, b: CanvasPoint) -> f32 {
    let ab = CanvasPoint::new(b.x - a.x, b.y - a.y);
    let length_squared = ab.x * ab.x + ab.y * ab.y;
    if length_squared <= f32::EPSILON {
        return distance(point, a);
    }
    let projection =
        (((point.x - a.x) * ab.x + (point.y - a.y) * ab.y) / length_squared).clamp(0.0, 1.0);
    distance(
        point,
        CanvasPoint::new(a.x + ab.x * projection, a.y + ab.y * projection),
    )
}

pub(crate) fn resize_handle_at(
    frame: SelectionFrame,
    world: CanvasPoint,
    scale: f32,
    resize: ResizeMode,
    reach: f32,
) -> Option<(ResizeHandle, f32)> {
    let reach = reach / scale.max(f32::EPSILON);
    resize_handles(frame)
        .into_iter()
        .filter(|(handle, _)| resize_handle_allowed(*handle, resize))
        .map(|(handle, at)| (handle, distance(at, world)))
        .filter(|(_, away)| *away <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

pub(crate) fn rotate_handle_at(
    frame: SelectionFrame,
    world: CanvasPoint,
    scale: f32,
    reach: f32,
) -> Option<f32> {
    let away = distance(frame.rotate_handle(), world);
    (away <= reach / scale.max(f32::EPSILON)).then_some(away)
}

pub(crate) fn handle_reach(frame: SelectionFrame, scale: f32, touch: bool) -> f32 {
    match touch {
        true => (frame.size.x.abs().min(frame.size.y.abs()) * scale / 3.0)
            .clamp(HANDLE_REACH, TOUCH_HANDLE_REACH),
        false => HANDLE_REACH,
    }
}

pub(crate) fn resize_handle_allowed(handle: ResizeHandle, resize: ResizeMode) -> bool {
    (handle.x == 0 || resize.horizontal()) && (handle.y == 0 || resize.vertical())
}

pub(crate) fn resize_handles(frame: SelectionFrame) -> [(ResizeHandle, CanvasPoint); 8] {
    [
        (-1, -1),
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
    ]
    .map(|(x, y)| {
        (
            ResizeHandle { x, y },
            frame.point(CanvasPoint::new(x as f32 * 0.5, y as f32 * 0.5)),
        )
    })
}

pub(crate) fn constrain_point_angle(
    start: CanvasPoint,
    current: CanvasPoint,
    step: f32,
) -> CanvasPoint {
    let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
    let length = delta.x.hypot(delta.y);
    let angle = (delta.y.atan2(delta.x) / step).round() * step;
    CanvasPoint::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

pub(crate) fn rotate_entities(
    frame: SelectionFrame,
    start_angle: f32,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    snap_angle: bool,
) -> Vec<CanvasEntity> {
    let center = frame.center;
    let current_angle = (current.y - center.y).atan2(current.x - center.x);
    let mut delta = current_angle - start_angle;
    if snap_angle {
        let step = 15.0_f32.to_radians();
        delta = (delta / step).round() * step;
    }
    originals
        .iter()
        .cloned()
        .map(|mut entity| {
            let relative = CanvasPoint::new(
                entity.transform.center.x - center.x,
                entity.transform.center.y - center.y,
            );
            let rotated = rotate(relative, delta);
            entity.transform.center = CanvasPoint::new(center.x + rotated.x, center.y + rotated.y);
            entity.transform.rotation += delta;
            entity
        })
        .collect()
}

pub(crate) fn resize_entities(
    handle: ResizeHandle,
    frame: SelectionFrame,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
    scale_text: bool,
    scale_editors: bool,
) -> Vec<CanvasEntity> {
    let current = rotate(
        CanvasPoint::new(current.x - frame.center.x, current.y - frame.center.y),
        -frame.rotation,
    );
    let local_originals = originals
        .iter()
        .cloned()
        .map(|mut entity| {
            entity.transform.center = rotate(
                CanvasPoint::new(
                    entity.transform.center.x - frame.center.x,
                    entity.transform.center.y - frame.center.y,
                ),
                -frame.rotation,
            );
            entity.transform.rotation -= frame.rotation;
            entity
        })
        .collect::<Vec<_>>();
    resize_entities_axis(
        handle,
        frame.local_bounds(),
        current,
        &local_originals,
        preserve_aspect_ratio,
        scale_text,
        scale_editors,
    )
    .into_iter()
    .map(|mut entity| {
        let center = rotate(entity.transform.center, frame.rotation);
        entity.transform.center =
            CanvasPoint::new(frame.center.x + center.x, frame.center.y + center.y);
        entity.transform.rotation += frame.rotation;
        entity
    })
    .collect()
}

pub(crate) fn resize_entities_axis(
    handle: ResizeHandle,
    bounds: WorldRect,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
    scale_text: bool,
    scale_editors: bool,
) -> Vec<CanvasEntity> {
    let original_size = bounds.size();
    let mut resized = bounds;
    if handle.x < 0 {
        resized.min.x = current.x.min(resized.max.x - MIN_SIZE);
    } else if handle.x > 0 {
        resized.max.x = current.x.max(resized.min.x + MIN_SIZE);
    }
    if handle.y < 0 {
        resized.min.y = current.y.min(resized.max.y - MIN_SIZE);
    } else if handle.y > 0 {
        resized.max.y = current.y.max(resized.min.y + MIN_SIZE);
    }
    if preserve_aspect_ratio {
        resized = proportional_resize_bounds(handle, bounds, resized);
    }
    let resized_size = resized.size();
    let scale_x = if handle.x == 0 && !preserve_aspect_ratio {
        1.0
    } else {
        resized_size.x / original_size.x.max(MIN_SIZE)
    };
    let scale_y = if handle.y == 0 && !preserve_aspect_ratio {
        1.0
    } else {
        resized_size.y / original_size.y.max(MIN_SIZE)
    };

    originals
        .iter()
        .cloned()
        .map(|mut entity| {
            let unit_x = (entity.transform.center.x - bounds.min.x) / original_size.x.max(MIN_SIZE);
            let unit_y = (entity.transform.center.y - bounds.min.y) / original_size.y.max(MIN_SIZE);
            entity.transform.center = CanvasPoint::new(
                if handle.x == 0 && !preserve_aspect_ratio {
                    entity.transform.center.x
                } else {
                    resized.min.x + unit_x * resized_size.x
                },
                if handle.y == 0 && !preserve_aspect_ratio {
                    entity.transform.center.y
                } else {
                    resized.min.y + unit_y * resized_size.y
                },
            );
            entity.transform.size.x = (entity.transform.size.x * scale_x).max(MIN_SIZE);
            entity.transform.size.y = (entity.transform.size.y * scale_y).max(MIN_SIZE);
            let factor = if handle.x == 0 { scale_y } else { scale_x };
            if entity.shape == CanvasShape::Editor && scale_editors {
                entity.scale = (entity.scale * factor).max(f32::EPSILON);
            }
            if entity.shape == CanvasShape::Text {
                if scale_text {
                    entity.style.text_size = entity.style.text_size.scaled(factor);
                } else {
                    entity.text_style.wrap = true;
                }
            }
            entity
        })
        .collect()
}

pub(crate) fn proportional_resize_bounds(
    handle: ResizeHandle,
    bounds: WorldRect,
    resized: WorldRect,
) -> WorldRect {
    let original_width = bounds.size().x.max(MIN_SIZE);
    let original_height = bounds.size().y.max(MIN_SIZE);
    let resized_width = resized.size().x.max(MIN_SIZE);
    let resized_height = resized.size().y.max(MIN_SIZE);
    let scale = match (handle.x, handle.y) {
        (0, _) => resized_height / original_height,
        (_, 0) => resized_width / original_width,
        _ => {
            let horizontal = resized_width / original_width;
            let vertical = resized_height / original_height;
            if (horizontal - 1.0).abs() >= (vertical - 1.0).abs() {
                horizontal
            } else {
                vertical
            }
        }
    }
    .max(MIN_SIZE / original_width)
    .max(MIN_SIZE / original_height);
    let width = original_width * scale;
    let height = original_height * scale;
    let center = bounds.center();
    let (min_x, max_x) = match handle.x {
        -1 => (bounds.max.x - width, bounds.max.x),
        1 => (bounds.min.x, bounds.min.x + width),
        _ => (center.x - width * 0.5, center.x + width * 0.5),
    };
    let (min_y, max_y) = match handle.y {
        -1 => (bounds.max.y - height, bounds.max.y),
        1 => (bounds.min.y, bounds.min.y + height),
        _ => (center.y - height * 0.5, center.y + height * 0.5),
    };
    WorldRect {
        min: CanvasPoint::new(min_x, min_y),
        max: CanvasPoint::new(max_x, max_y),
    }
}

pub(crate) const PEN_PRESSURE: f32 = 0.5;
pub(crate) const MOST_PRESSED_WIDTH: f32 = 1.75;

pub(crate) fn pen_entity(
    points: Vec<CanvasPoint>,
    pressures: Vec<f32>,
    style: CanvasStyle,
) -> CanvasEntity {
    let mut entity = CanvasEntity::new(CanvasShape::Freehand, CanvasTransform::default());
    entity.style = CanvasStyle {
        seed: entity.style.seed,
        ..style
    };
    entity.set_drawn_points(points.into_iter().map(CanvasPathPoint::new).collect());
    entity.pressures = pressures;
    entity
}

pub(crate) fn drawn_seed(shape: CanvasShape, at: CanvasPoint) -> u32 {
    let mut hash = 0x811c_9dc5 ^ shape as u32;
    for byte in [at.x, at.y]
        .into_iter()
        .flat_map(|number| number.to_bits().to_le_bytes())
    {
        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
    }
    hash
}

pub(crate) fn text_box_size(
    text_style: &CanvasTextStyle,
    font_size: f32,
    content: Vec2,
) -> CanvasPoint {
    CanvasPoint::new(
        content.x.max(16.0),
        content.y.max(font_size * text_style.line_height),
    )
}

pub(crate) fn text_grown(
    transform: CanvasTransform,
    text_style: &CanvasTextStyle,
    size: CanvasPoint,
) -> CanvasTransform {
    let grown = CanvasPoint::new(size.x - transform.size.x, size.y - transform.size.y);
    let x = match text_style.alignment {
        CanvasTextAlign::Left => grown.x / 2.0,
        CanvasTextAlign::Center => 0.0,
        CanvasTextAlign::Right => -grown.x / 2.0,
    };
    let y = match text_style.vertical_alignment {
        CanvasTextVerticalAlign::Top => grown.y / 2.0,
        CanvasTextVerticalAlign::Middle => 0.0,
        CanvasTextVerticalAlign::Bottom => -grown.y / 2.0,
    };
    let shift = rotate(CanvasPoint::new(x, y), transform.rotation);
    CanvasTransform::new(
        CanvasPoint::new(transform.center.x + shift.x, transform.center.y + shift.y),
        size,
        transform.rotation,
    )
}
