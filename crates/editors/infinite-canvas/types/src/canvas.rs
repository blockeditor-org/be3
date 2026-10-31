use std::collections::{BTreeMap, HashMap, HashSet};

use be_model::{
    Anchor, BlockRef, Bounds, Change, Document, Edit, Grid, List, Map, Model, ObjectId, RichText,
    Series, marks::Rich,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use be_block::{ChildChange, Root};
use database_types::database::DatabaseValue;

use crate::raster::{
    self, CanvasPixel, CanvasPixelUpdate, CanvasRaster, CanvasRasterAnchor, default_raster_palette,
    normalized_palette, raster_dimensions, valid_dimension,
};

pub const BINDING_GAP: f32 = 4.0;

mod rich_text {
    use be_model::RichText;
    use be_model::marks::Seed;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize, Serialize)]
    struct Held {
        bytes: Vec<u8>,
        seeds: Vec<Seed>,
    }

    pub fn serialize<S: Serializer>(text: &RichText, serializer: S) -> Result<S::Ok, S::Error> {
        Held {
            bytes: text.bytes().to_vec(),
            seeds: text.seeds().to_vec(),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<RichText, D::Error> {
        let held = Held::deserialize(deserializer)?;
        Ok(RichText::new(held.bytes, held.seeds))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

impl CanvasPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasTransform {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
    pub rotation: f32,
}

impl CanvasTransform {
    pub const fn new(center: CanvasPoint, size: CanvasPoint, rotation: f32) -> Self {
        Self {
            center,
            size,
            rotation,
        }
    }

    pub fn to_world(self, local: CanvasPoint) -> CanvasPoint {
        let scaled = CanvasPoint::new(local.x * self.size.x, local.y * self.size.y);
        let turned = rotated(scaled, self.rotation);
        CanvasPoint::new(self.center.x + turned.x, self.center.y + turned.y)
    }

    pub fn to_local(self, world: CanvasPoint) -> CanvasPoint {
        let relative = CanvasPoint::new(world.x - self.center.x, world.y - self.center.y);
        let turned = rotated(relative, -self.rotation);
        CanvasPoint::new(
            turned.x / self.size.x.max(0.001),
            turned.y / self.size.y.max(0.001),
        )
    }
}

fn rotated(point: CanvasPoint, angle: f32) -> CanvasPoint {
    let (sin, cos) = angle.sin_cos();
    CanvasPoint::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasShape {
    #[default]
    Rectangle,
    Ellipse,
    Diamond,
    Path,
    Freehand,
    Text,
    Image,
    Block,
    Editor,
    Frame,
    Raster,
}

impl CanvasShape {
    pub fn embeds_block(self) -> bool {
        matches!(self, Self::Image | Self::Block | Self::Editor)
    }

    pub fn has_points(self) -> bool {
        matches!(self, Self::Path | Self::Freehand)
    }

    pub fn bindable(self) -> bool {
        matches!(
            self,
            Self::Rectangle
                | Self::Ellipse
                | Self::Diamond
                | Self::Text
                | Self::Image
                | Self::Block
                | Self::Editor
        )
    }

    pub fn within(self, local: CanvasPoint) -> CanvasPoint {
        if self.contains_local(local) {
            return local;
        }
        let (mut inside, mut outside) = (0.0, 1.0);
        for _ in 0..24 {
            let middle = (inside + outside) * 0.5;
            match self.contains_local(CanvasPoint::new(local.x * middle, local.y * middle)) {
                true => inside = middle,
                false => outside = middle,
            }
        }
        CanvasPoint::new(local.x * inside, local.y * inside)
    }

    pub fn contains_local(self, local: CanvasPoint) -> bool {
        match self {
            Self::Ellipse => (local.x * 2.0).powi(2) + (local.y * 2.0).powi(2) <= 1.0,
            Self::Diamond => local.x.abs() + local.y.abs() <= 0.5,
            _ => local.x.abs() <= 0.5 && local.y.abs() <= 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasPathPoint {
    pub x: f32,
    pub y: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle_in: Option<CanvasPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle_out: Option<CanvasPoint>,
}

impl CanvasPathPoint {
    pub const fn new(at: CanvasPoint) -> Self {
        Self {
            x: at.x,
            y: at.y,
            handle_in: None,
            handle_out: None,
        }
    }

    pub const fn at(self) -> CanvasPoint {
        CanvasPoint::new(self.x, self.y)
    }

    pub fn map(self, mut place: impl FnMut(CanvasPoint) -> CanvasPoint) -> Self {
        let at = place(self.at());
        Self {
            x: at.x,
            y: at.y,
            handle_in: self.handle_in.map(&mut place),
            handle_out: self.handle_out.map(&mut place),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasBinding {
    pub entity: Uuid,
    pub anchor: CanvasPoint,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasArrowhead {
    #[default]
    None,
    Arrow,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextWeight {
    #[default]
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextVerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextSize {
    Small,
    #[default]
    Medium,
    Large,
    ExtraLarge,
    Custom(f32),
}

impl CanvasTextSize {
    pub const TOKENS: [Self; 4] = [Self::Small, Self::Medium, Self::Large, Self::ExtraLarge];

    pub fn px(self) -> f32 {
        match self {
            Self::Small => 16.0,
            Self::Medium => 20.0,
            Self::Large => 28.0,
            Self::ExtraLarge => 36.0,
            Self::Custom(px) => match px.is_finite() {
                true => px.clamp(4.0, 256.0),
                false => 20.0,
            },
        }
    }

    pub fn scaled(self, factor: f32) -> Self {
        Self::Custom((self.px() * factor).max(4.0))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasTextStyle {
    pub weight: CanvasTextWeight,
    pub alignment: CanvasTextAlign,
    pub vertical_alignment: CanvasTextVerticalAlign,
    pub line_height: f32,
    pub wrap: bool,
}

impl Default for CanvasTextStyle {
    fn default() -> Self {
        Self {
            weight: CanvasTextWeight::Regular,
            alignment: CanvasTextAlign::Left,
            vertical_alignment: CanvasTextVerticalAlign::Top,
            line_height: 1.2,
            wrap: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasProfile {
    #[default]
    Prototyping,
    Vector,
    Raster,
}

impl CanvasProfile {
    pub const ALL: [Self; 3] = [Self::Prototyping, Self::Vector, Self::Raster];

    pub fn name(self) -> &'static str {
        match self {
            Self::Prototyping => "Prototyping",
            Self::Vector => "Vector",
            Self::Raster => "Raster",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasHue {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTone {
    Strong,
    Soft,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasColor {
    #[default]
    Auto,
    Palette {
        hue: CanvasHue,
        tone: CanvasTone,
    },
    Rgba {
        #[serde(default)]
        red: u8,
        #[serde(default)]
        green: u8,
        #[serde(default)]
        blue: u8,
        #[serde(default)]
        alpha: u8,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasFill {
    #[default]
    None,
    Hatched,
    Solid,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasStrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasLook {
    #[default]
    Clean,
    HandDrawn,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasStyle {
    pub stroke_color: CanvasColor,
    pub fill_color: CanvasColor,
    pub fill: CanvasFill,
    pub stroke_width: f32,
    pub stroke_style: CanvasStrokeStyle,
    pub look: CanvasLook,
    pub roughness: f32,
    pub opacity: f32,
    pub text_size: CanvasTextSize,
    pub corner_radius: f32,
    pub start_arrowhead: CanvasArrowhead,
    pub end_arrowhead: CanvasArrowhead,
    pub seed: u32,
}

impl Default for CanvasStyle {
    fn default() -> Self {
        Self {
            stroke_color: CanvasColor::Auto,
            fill_color: CanvasColor::Auto,
            fill: CanvasFill::None,
            stroke_width: 2.0,
            stroke_style: CanvasStrokeStyle::Solid,
            look: CanvasLook::Clean,
            roughness: 1.0,
            opacity: 1.0,
            text_size: CanvasTextSize::Medium,
            corner_radius: 0.0,
            start_arrowhead: CanvasArrowhead::None,
            end_arrowhead: CanvasArrowhead::None,
            seed: 0,
        }
    }
}

impl CanvasStyle {
    pub fn filled(&self) -> Option<CanvasColor> {
        (self.fill != CanvasFill::None).then_some(self.fill_color)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct CanvasPointsFrame {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
}

impl CanvasPointsFrame {
    fn around(points: &[CanvasPathPoint]) -> Option<Self> {
        let mut low = CanvasPoint::new(f32::INFINITY, f32::INFINITY);
        let mut high = CanvasPoint::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for point in points {
            for at in [Some(point.at()), point.handle_in, point.handle_out]
                .into_iter()
                .flatten()
            {
                low = CanvasPoint::new(low.x.min(at.x), low.y.min(at.y));
                high = CanvasPoint::new(high.x.max(at.x), high.y.max(at.y));
            }
        }
        low.x.is_finite().then(|| Self {
            center: CanvasPoint::new((low.x + high.x) * 0.5, (low.y + high.y) * 0.5),
            size: CanvasPoint::new((high.x - low.x).max(1.0), (high.y - low.y).max(1.0)),
        })
    }

    fn normalized(self, at: CanvasPoint) -> CanvasPoint {
        CanvasPoint::new(
            (at.x - self.center.x) / self.size.x,
            (at.y - self.center.y) / self.size.y,
        )
    }

    fn fills(self, transform: CanvasTransform) -> bool {
        transform == CanvasTransform::new(self.center, self.size, 0.0)
    }

    fn place(self, transform: CanvasTransform, world: CanvasPoint) -> CanvasPoint {
        if self.fills(transform) {
            return world;
        }
        let local = transform.to_local(world);
        CanvasPoint::new(
            self.center.x + local.x * self.size.x,
            self.center.y + local.y * self.size.y,
        )
    }

    fn grown(self, points: &[CanvasPathPoint]) -> Self {
        let corner = |sign: f32| {
            CanvasPathPoint::new(CanvasPoint::new(
                self.center.x + sign * self.size.x * 0.5,
                self.center.y + sign * self.size.y * 0.5,
            ))
        };
        let mut around = vec![corner(-1.0), corner(1.0)];
        around.extend_from_slice(points);
        Self::around(&around).unwrap_or(self)
    }

    fn transform_for(self, transform: CanvasTransform, grown: Self) -> CanvasTransform {
        if self.fills(transform) {
            return CanvasTransform::new(grown.center, grown.size, 0.0);
        }
        let scale = CanvasPoint::new(
            transform.size.x / self.size.x,
            transform.size.y / self.size.y,
        );
        let shift = rotated(
            CanvasPoint::new(
                (grown.center.x - self.center.x) * scale.x,
                (grown.center.y - self.center.y) * scale.y,
            ),
            transform.rotation,
        );
        CanvasTransform::new(
            CanvasPoint::new(transform.center.x + shift.x, transform.center.y + shift.y),
            CanvasPoint::new(grown.size.x * scale.x, grown.size.y * scale.y),
            transform.rotation,
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasComponent {
    pub schema_id: Uuid,
    pub values: BTreeMap<Uuid, DatabaseValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntity {
    pub id: Uuid,
    pub transform: CanvasTransform,
    pub shape: CanvasShape,
    pub block: Option<Uuid>,
    pub scale: f32,
    pub points: Vec<CanvasPathPoint>,
    pub pressures: Vec<f32>,
    #[serde(default)]
    pub points_frame: Option<CanvasPointsFrame>,
    pub bindings: [Option<CanvasBinding>; 2],
    #[serde(with = "rich_text")]
    pub text: RichText,
    pub placeholder: String,
    pub text_style: CanvasTextStyle,
    pub style: CanvasStyle,
    pub hidden: bool,
    pub group_id: Option<Uuid>,
    pub locked: bool,
    pub components: Vec<CanvasComponent>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasLayerMove {
    BringToFront,
    ForwardOne,
    BackOne,
    SendToBack,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum InfiniteCanvasOperation {
    Add {
        entity: Box<CanvasEntity>,
    },
    Update {
        entities: Vec<CanvasEntity>,
    },
    Remove {
        ids: Vec<Uuid>,
    },
    Reorder {
        ids: Vec<Uuid>,
        movement: CanvasLayerMove,
    },
    ExactOrder {
        ids: Vec<Uuid>,
    },
    Extend {
        id: Uuid,
        points: Vec<CanvasPoint>,
        pressures: Vec<f32>,
    },
    Paint {
        id: Uuid,
        pixels: Vec<CanvasPixelUpdate>,
    },
    Fill {
        id: Uuid,
        x: u16,
        y: u16,
        color: CanvasPixel,
    },
    ReplaceColor {
        id: Uuid,
        from: CanvasPixel,
        to: CanvasPixel,
    },
    Clear {
        id: Uuid,
    },
    Resize {
        id: Uuid,
        width: u16,
        height: u16,
        anchor: CanvasRasterAnchor,
    },
    SetPalette {
        colors: Vec<CanvasPixel>,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasGridSpacing {
    Small,
    #[default]
    Medium,
    Large,
}

impl CanvasGridSpacing {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default)]
pub struct CanvasGrid {
    pub hidden: bool,
    pub spacing: CanvasGridSpacing,
}

impl CanvasGrid {
    pub fn shown(self) -> Option<CanvasGridSpacing> {
        (!self.hidden).then_some(self.spacing)
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "canvas")]
pub struct Canvas {
    pub entities: List<Entity>,
    pub grid: CanvasGrid,
    pub profile: CanvasProfile,
    pub palette: Option<Vec<CanvasPixel>>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "canvas.entity")]
pub struct Entity {
    pub shape: Option<CanvasShape>,
    pub center: CanvasPoint,
    pub size: CanvasPoint,
    pub rotation: f32,
    pub block: Option<BlockRef>,
    pub scale: f32,
    pub points: Series<CanvasPathPoint>,
    pub pressures: Series<f32>,
    pub start_binding: Option<CanvasBinding>,
    pub end_binding: Option<CanvasBinding>,
    pub text: RichText,
    pub placeholder: String,
    pub text_style: CanvasTextStyle,
    pub stroke_color: CanvasColor,
    pub fill_color: CanvasColor,
    pub fill: CanvasFill,
    pub stroke_width: f32,
    pub stroke_style: CanvasStrokeStyle,
    pub look: CanvasLook,
    pub roughness: f32,
    pub opacity: f32,
    pub text_size: CanvasTextSize,
    pub corner_radius: f32,
    pub start_arrowhead: CanvasArrowhead,
    pub end_arrowhead: CanvasArrowhead,
    pub seed: u32,
    pub hidden: bool,
    pub group_id: Option<Uuid>,
    pub locked: bool,
    pub components: List<Component>,
    pub points_frame: Option<CanvasPointsFrame>,
    pub pixels: Grid<[u8; 4]>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "canvas.component")]
pub struct Component {
    pub schema: Option<BlockRef>,
    pub values: Map<Uuid, DatabaseValue>,
}

impl Entity {
    fn of(entity: &CanvasEntity) -> Self {
        let style = entity.style;
        Self {
            shape: Some(entity.shape),
            center: entity.transform.center,
            size: entity.transform.size,
            rotation: entity.transform.rotation,
            block: entity.block.map(BlockRef),
            scale: entity.scale,
            points: entity.points.clone().into(),
            pressures: entity.pressures.clone().into(),
            start_binding: entity.bindings[0],
            end_binding: entity.bindings[1],
            text: entity.text.clone(),
            placeholder: entity.placeholder.clone(),
            text_style: entity.text_style,
            stroke_color: style.stroke_color,
            fill_color: style.fill_color,
            fill: style.fill,
            stroke_width: style.stroke_width,
            stroke_style: style.stroke_style,
            look: style.look,
            roughness: style.roughness,
            opacity: style.opacity,
            text_size: style.text_size,
            corner_radius: style.corner_radius,
            start_arrowhead: style.start_arrowhead,
            end_arrowhead: style.end_arrowhead,
            seed: style.seed,
            hidden: entity.hidden,
            group_id: entity.group_id,
            locked: entity.locked,
            components: entity.components.iter().map(Component::of).collect(),
            points_frame: entity.points_frame,
            pixels: Grid::default(),
        }
    }

    fn placed(entity: &CanvasEntity) -> Self {
        let mut held = Self::of(entity);
        if entity.shape == CanvasShape::Raster {
            let (width, height) = raster_dimensions(entity.transform.size);
            let transform = CanvasTransform::raster(entity.transform.center, width, height);
            held.center = transform.center;
            held.size = transform.size;
            held.rotation = 0.0;
            held.pixels = Grid::new(Bounds::new(0, 0, u32::from(width), u32::from(height)));
        }
        held
    }

    pub fn raster(&self) -> Option<CanvasRaster> {
        match self.shape {
            Some(CanvasShape::Raster) => CanvasRaster::of(&self.pixels),
            _ => None,
        }
    }

    pub fn view(&self, id: ObjectId) -> Option<CanvasEntity> {
        Some(CanvasEntity {
            id: id.as_uuid(),
            transform: CanvasTransform::new(self.center, self.size, self.rotation),
            shape: self.shape?,
            block: self.block.map(|block| block.0),
            scale: self.scale,
            points: self.points.to_vec(),
            pressures: self.pressures.to_vec(),
            points_frame: self.points_frame,
            bindings: [self.start_binding, self.end_binding],
            text: self.text.clone(),
            placeholder: self.placeholder.clone(),
            text_style: self.text_style,
            style: CanvasStyle {
                stroke_color: self.stroke_color,
                fill_color: self.fill_color,
                fill: self.fill,
                stroke_width: self.stroke_width,
                stroke_style: self.stroke_style,
                look: self.look,
                roughness: self.roughness,
                opacity: self.opacity,
                text_size: self.text_size,
                corner_radius: self.corner_radius,
                start_arrowhead: self.start_arrowhead,
                end_arrowhead: self.end_arrowhead,
                seed: self.seed,
            },
            hidden: self.hidden,
            group_id: self.group_id,
            locked: self.locked,
            components: self
                .components
                .iter()
                .filter_map(|component| component.view())
                .collect(),
        })
    }
}

impl Component {
    fn of(component: &CanvasComponent) -> Self {
        Self {
            schema: Some(BlockRef(component.schema_id)),
            values: component
                .values
                .iter()
                .map(|(field, value)| (*field, value.clone()))
                .collect(),
        }
    }

    fn view(&self) -> Option<CanvasComponent> {
        Some(CanvasComponent {
            schema_id: self.schema?.0,
            values: self
                .values
                .iter()
                .map(|(field, value)| (*field, value.clone()))
                .collect(),
        })
    }
}

impl Canvas {
    pub fn set_grid(grid: CanvasGrid) -> Edit {
        Self::GRID.set(ObjectId::ROOT, &grid).into()
    }

    pub fn resolved_palette(&self) -> Vec<CanvasPixel> {
        self.palette.clone().unwrap_or_else(default_raster_palette)
    }

    pub fn palette_in(content: &CanvasContent) -> Vec<CanvasPixel> {
        content
            .field(ObjectId::ROOT, Self::PALETTE)
            .unwrap_or_else(default_raster_palette)
    }

    pub fn raster(content: &CanvasContent, id: Uuid) -> Option<CanvasRaster> {
        let object = ObjectId::from_uuid(id);
        if !content.tree().contains(object)
            || content.field(object, Entity::SHAPE) != Some(CanvasShape::Raster)
        {
            return None;
        }
        CanvasRaster::of(&content.field(object, Entity::PIXELS))
    }

    pub fn with_entities(entities: impl IntoIterator<Item = CanvasEntity>) -> Self {
        let mut seen = HashSet::new();
        Self {
            entities: List::with_ids(
                entities
                    .into_iter()
                    .filter(|entity| !entity.id.is_nil() && seen.insert(entity.id))
                    .map(|entity| {
                        (
                            ObjectId::from_uuid(entity.id),
                            Entity::placed(&normalized_entity(entity)),
                        )
                    }),
            ),
            ..Self::default()
        }
    }

    pub fn set_profile(&self, profile: CanvasProfile) -> Edit {
        match self.profile == profile {
            true => Edit::default(),
            false => Self::PROFILE.set(ObjectId::ROOT, &profile).into(),
        }
    }

    pub fn entities(&self) -> Vec<CanvasEntity> {
        let unbound = self.unbound();
        let by_id: HashMap<Uuid, &CanvasEntity> =
            unbound.iter().map(|entity| (entity.id, entity)).collect();
        unbound
            .iter()
            .map(|entity| entity.followed(|id| by_id.get(&id).copied()))
            .collect()
    }

    fn unbound(&self) -> Vec<CanvasEntity> {
        self.entities
            .iter()
            .filter_map(|held| held.view(held.id))
            .collect()
    }

    pub fn edit_in(content: &CanvasContent, operation: &InfiniteCanvasOperation) -> Edit {
        let held = |id: Uuid| {
            let object = ObjectId::from_uuid(id);
            Some(be_model::Item {
                id: object,
                value: content.read::<Entity>(object)?,
            })
        };
        let unbound = |id: Uuid| held(id)?.view(ObjectId::from_uuid(id));
        match operation {
            InfiniteCanvasOperation::Add { entity } => {
                if entity.id.is_nil() || content.tree().contains(ObjectId::from_uuid(entity.id)) {
                    return Edit::default();
                }
                Self::ENTITIES
                    .insert_as(
                        ObjectId::from_uuid(entity.id),
                        ObjectId::ROOT,
                        Anchor::End,
                        &Entity::placed(&normalized_entity(entity.as_ref().clone())),
                    )
                    .into()
            }
            InfiniteCanvasOperation::Update { entities } => entities
                .iter()
                .flat_map(|update| {
                    let mut update = normalized_entity(update.clone());
                    let Some(held) = held(update.id) else {
                        return Vec::new();
                    };
                    let Some(mut shown) = held.view(held.id) else {
                        return Vec::new();
                    };
                    shown.follow(unbound);
                    if shown.shape == CanvasShape::Raster || update.shape == CanvasShape::Raster {
                        update.shape = shown.shape;
                    }
                    update_entity(content, &held, &shown, &update)
                })
                .collect(),
            InfiniteCanvasOperation::Remove { ids } => {
                let removed: HashSet<Uuid> = ids
                    .iter()
                    .copied()
                    .filter(|id| content.tree().contains(ObjectId::from_uuid(*id)))
                    .collect();
                let mut changes: Vec<Change> = removed
                    .iter()
                    .map(|id| Change::remove(ObjectId::from_uuid(*id)))
                    .collect();
                let bound_to_removed = |binding: &Option<CanvasBinding>| {
                    binding.is_some_and(|binding| removed.contains(&binding.entity))
                };
                for object in content.ids(ObjectId::ROOT, Self::ENTITIES) {
                    let ends = [
                        content.field(object, Entity::START_BINDING),
                        content.field(object, Entity::END_BINDING),
                    ];
                    if removed.contains(&object.as_uuid()) || !ends.iter().any(bound_to_removed) {
                        continue;
                    }
                    let Some(held) = held(object.as_uuid()) else {
                        continue;
                    };
                    let Some(entity) = held.view(object) else {
                        continue;
                    };
                    let mut kept = entity.clone();
                    kept.follow(unbound);
                    for binding in &mut kept.bindings {
                        if bound_to_removed(binding) {
                            *binding = None;
                        }
                    }
                    changes.extend(update_entity(content, &held, &entity, &kept));
                }
                Edit(changes)
            }
            InfiniteCanvasOperation::Reorder { ids, movement } => {
                let order = Self::order(content);
                Self::reorder_to(&order, &reordered(&order, ids, *movement), ids)
            }
            InfiniteCanvasOperation::ExactOrder { ids } => {
                let order = Self::order(content);
                Self::reorder_to(&order, &exactly_ordered(&order, ids), ids)
            }
            InfiniteCanvasOperation::Extend {
                id,
                points,
                pressures,
            } => Self::extend(content, ObjectId::from_uuid(*id), points, pressures),
            InfiniteCanvasOperation::Paint { id, pixels } => {
                Self::repaint(content, *id, |raster| raster::painted(raster, pixels))
            }
            InfiniteCanvasOperation::Fill { id, x, y, color } => {
                Self::repaint(content, *id, |raster| {
                    raster::filled(raster, *x, *y, *color)
                })
            }
            InfiniteCanvasOperation::ReplaceColor { id, from, to } => {
                Self::repaint(content, *id, |raster| match from == to {
                    true => Vec::new(),
                    false => raster::recolored(raster, |held| (held == *from).then_some(*to)),
                })
            }
            InfiniteCanvasOperation::Clear { id } => Self::repaint(content, *id, |raster| {
                raster::recolored(raster, |held| {
                    (held != CanvasPixel::TRANSPARENT).then_some(CanvasPixel::TRANSPARENT)
                })
            }),
            InfiniteCanvasOperation::Resize {
                id,
                width,
                height,
                anchor,
            } => Self::resize(content, *id, *width, *height, *anchor),
            InfiniteCanvasOperation::SetPalette { colors } => {
                let colors = normalized_palette(colors);
                match colors == Self::palette_in(content) {
                    true => Edit::default(),
                    false => Self::PALETTE.set(ObjectId::ROOT, &Some(colors)).into(),
                }
            }
        }
    }

    fn repaint(
        content: &CanvasContent,
        id: Uuid,
        cells: impl FnOnce(&CanvasRaster) -> raster::Cells,
    ) -> Edit {
        Self::raster(content, id)
            .and_then(|held| raster::paint(ObjectId::from_uuid(id), cells(&held)))
            .into_iter()
            .collect()
    }

    fn resize(
        content: &CanvasContent,
        id: Uuid,
        width: u16,
        height: u16,
        anchor: CanvasRasterAnchor,
    ) -> Edit {
        let Some(held) = Self::raster(content, id) else {
            return Edit::default();
        };
        if !valid_dimension(width)
            || !valid_dimension(height)
            || (width == held.width() && height == held.height())
        {
            return Edit::default();
        }
        let object = ObjectId::from_uuid(id);
        let (old, new) = (
            held.bounds(),
            raster::resized(held.bounds(), width, height, anchor),
        );
        let center: CanvasPoint = content.field(object, Entity::CENTER);
        let size: CanvasPoint = content.field(object, Entity::SIZE);
        let pixel = CanvasPoint::new(
            size.x / f32::from(held.width()),
            size.y / f32::from(held.height()),
        );
        let corner = CanvasPoint::new(
            center.x - size.x * 0.5 + (new.left - old.left) as f32 * pixel.x,
            center.y - size.y * 0.5 + (new.top - old.top) as f32 * pixel.y,
        );
        let size = CanvasPoint::new(f32::from(width) * pixel.x, f32::from(height) * pixel.y);
        Edit(vec![
            Entity::PIXELS.reshape(object, new),
            Entity::CENTER.set(
                object,
                &CanvasPoint::new(corner.x + size.x * 0.5, corner.y + size.y * 0.5),
            ),
            Entity::SIZE.set(object, &size),
        ])
    }

    fn extend(
        content: &CanvasContent,
        object: ObjectId,
        points: &[CanvasPoint],
        pressures: &[f32],
    ) -> Edit {
        let (Some(frame), Some(held_points), Some(held_pressures)) = (
            content.field(object, Entity::POINTS_FRAME),
            content.series(object, Entity::POINTS),
            content.series(object, Entity::PRESSURES),
        ) else {
            return Edit::default();
        };
        let transform = CanvasTransform::new(
            content.field(object, Entity::CENTER),
            content.field(object, Entity::SIZE),
            content.field(object, Entity::ROTATION),
        );
        let added: Vec<CanvasPathPoint> = points
            .iter()
            .map(|world| CanvasPathPoint::new(frame.place(transform, *world)))
            .collect();
        let Some(appended) = Entity::POINTS.append(object, held_points, &added) else {
            return Edit::default();
        };
        let mut changes = vec![appended];
        changes.extend(Entity::PRESSURES.append(object, held_pressures, pressures));
        let grown = frame.grown(&added);
        if grown != frame {
            let resized = frame.transform_for(transform, grown);
            changes.push(Entity::POINTS_FRAME.set(object, &Some(grown)));
            changes.push(Entity::CENTER.set(object, &resized.center));
            changes.push(Entity::SIZE.set(object, &resized.size));
        }
        Edit(changes)
    }

    fn order(content: &CanvasContent) -> Vec<Uuid> {
        content
            .ids(ObjectId::ROOT, Self::ENTITIES)
            .into_iter()
            .map(ObjectId::as_uuid)
            .collect()
    }

    pub fn rich_text(content: &CanvasContent, entity: Uuid) -> Option<&Rich> {
        content.rich(ObjectId::from_uuid(entity), Entity::TEXT)
    }

    pub fn retext(content: &CanvasContent, client: u64, entity: Uuid, text: &str) -> Option<Edit> {
        let held = Self::rich_text(content, entity)?.text.items();
        let wanted = text.as_bytes();
        let start = held
            .iter()
            .zip(wanted)
            .take_while(|(held, wanted)| held == wanted)
            .count();
        let end = held[start..]
            .iter()
            .rev()
            .zip(wanted[start..].iter().rev())
            .take_while(|(held, wanted)| held == wanted)
            .count();
        let op = Self::rich_text(content, entity)?.text.replace(
            client,
            start..held.len() - end,
            wanted[start..wanted.len() - end].to_vec(),
        )?;
        Some(Entity::TEXT.edit(ObjectId::from_uuid(entity), op).into())
    }

    fn reorder_to(current: &[Uuid], target: &[Uuid], moved: &[Uuid]) -> Edit {
        if current == target {
            return Edit::default();
        }
        let held: HashSet<Uuid> = current.iter().copied().collect();
        let moved: HashSet<Uuid> = moved.iter().copied().collect();
        let object_of = |entity: Uuid| held.contains(&entity).then(|| ObjectId::from_uuid(entity));
        let mut changes = Vec::new();
        for (index, entity) in target.iter().enumerate() {
            if !moved.contains(entity) {
                continue;
            }
            let Some(object) = object_of(*entity) else {
                continue;
            };
            let anchor = match index
                .checked_sub(1)
                .and_then(|previous| object_of(target[previous]))
            {
                Some(previous) => Anchor::After(previous),
                None => Anchor::Start,
            };
            changes.push(Self::ENTITIES.move_into(ObjectId::ROOT, anchor, object));
        }
        Edit(changes)
    }

    fn child_entities(&self, old: Uuid) -> impl Iterator<Item = &be_model::Item<Entity>> {
        self.entities
            .iter()
            .filter(move |held| held.block == Some(BlockRef(old)))
    }
}

fn update_entity(
    content: &CanvasContent,
    held: &be_model::Item<Entity>,
    shown: &CanvasEntity,
    update: &CanvasEntity,
) -> Vec<Change> {
    let id = held.id;
    let mut changes = Vec::new();
    macro_rules! write {
        ($field:ident, $shown:expr, $update:expr) => {
            if $shown != $update {
                changes.push(Entity::$field.set(id, &$update));
            }
        };
    }
    write!(SHAPE, Some(shown.shape), Some(update.shape));
    write!(CENTER, shown.transform.center, update.transform.center);
    write!(SIZE, shown.transform.size, update.transform.size);
    write!(
        ROTATION,
        shown.transform.rotation,
        update.transform.rotation
    );
    write!(BLOCK, shown.block.map(BlockRef), update.block.map(BlockRef));
    write!(SCALE, shown.scale, update.scale);
    if shown.points != update.points
        && let Some(change) = content
            .series(id, Entity::POINTS)
            .and_then(|points| Entity::POINTS.update(id, points, &update.points))
    {
        changes.push(change);
    }
    if shown.pressures != update.pressures
        && let Some(change) = content
            .series(id, Entity::PRESSURES)
            .and_then(|pressures| Entity::PRESSURES.update(id, pressures, &update.pressures))
    {
        changes.push(change);
    }
    write!(POINTS_FRAME, shown.points_frame, update.points_frame);
    write!(START_BINDING, shown.bindings[0], update.bindings[0]);
    write!(END_BINDING, shown.bindings[1], update.bindings[1]);
    write!(PLACEHOLDER, shown.placeholder, update.placeholder);
    write!(TEXT_STYLE, shown.text_style, update.text_style);
    let (was, style) = (shown.style, update.style);
    write!(STROKE_COLOR, was.stroke_color, style.stroke_color);
    write!(FILL_COLOR, was.fill_color, style.fill_color);
    write!(FILL, was.fill, style.fill);
    write!(STROKE_WIDTH, was.stroke_width, style.stroke_width);
    write!(STROKE_STYLE, was.stroke_style, style.stroke_style);
    write!(LOOK, was.look, style.look);
    write!(ROUGHNESS, was.roughness, style.roughness);
    write!(OPACITY, was.opacity, style.opacity);
    write!(TEXT_SIZE, was.text_size, style.text_size);
    write!(CORNER_RADIUS, was.corner_radius, style.corner_radius);
    write!(START_ARROWHEAD, was.start_arrowhead, style.start_arrowhead);
    write!(END_ARROWHEAD, was.end_arrowhead, style.end_arrowhead);
    write!(SEED, was.seed, style.seed);
    write!(HIDDEN, shown.hidden, update.hidden);
    write!(GROUP_ID, shown.group_id, update.group_id);
    write!(LOCKED, shown.locked, update.locked);
    changes.extend(update_components(held, &update.components));
    changes
}

fn update_components(held: &be_model::Item<Entity>, wanted: &[CanvasComponent]) -> Vec<Change> {
    let mut changes = Vec::new();
    for component in held.components.iter() {
        let Some(update) = wanted
            .iter()
            .find(|wanted| Some(BlockRef(wanted.schema_id)) == component.schema)
        else {
            changes.push(Change::remove(component.id));
            continue;
        };
        for (field, value) in component.values.iter() {
            if !update.values.contains_key(field) {
                changes.push(Component::VALUES.put(component.id, field, None));
            } else if update.values.get(field) != Some(value) {
                changes.push(Component::VALUES.put(component.id, field, update.values.get(field)));
            }
        }
        for (field, value) in &update.values {
            if !component.values.contains_key(field) {
                changes.push(Component::VALUES.put(component.id, field, Some(value)));
            }
        }
    }
    for component in wanted {
        if !held
            .components
            .iter()
            .any(|existing| existing.schema == Some(BlockRef(component.schema_id)))
        {
            let id = ObjectId::from_uuid(Uuid::from_u128(
                held.id.as_uuid().as_u128() ^ component.schema_id.as_u128(),
            ));
            changes.push(Entity::COMPONENTS.insert_as(
                id,
                held.id,
                Anchor::End,
                &Component::of(component),
            ));
        }
    }
    changes
}

fn reordered(order: &[Uuid], ids: &[Uuid], movement: CanvasLayerMove) -> Vec<Uuid> {
    let selected: HashSet<_> = ids.iter().copied().collect();
    let mut order = order.to_vec();
    match movement {
        CanvasLayerMove::BringToFront => {
            let (mut back, front): (Vec<_>, Vec<_>) =
                order.into_iter().partition(|id| !selected.contains(id));
            back.extend(front);
            order = back;
        }
        CanvasLayerMove::ForwardOne => {
            for index in (0..order.len().saturating_sub(1)).rev() {
                if selected.contains(&order[index]) && !selected.contains(&order[index + 1]) {
                    order.swap(index, index + 1);
                }
            }
        }
        CanvasLayerMove::BackOne => {
            for index in 1..order.len() {
                if selected.contains(&order[index]) && !selected.contains(&order[index - 1]) {
                    order.swap(index - 1, index);
                }
            }
        }
        CanvasLayerMove::SendToBack => {
            let (mut back, front): (Vec<_>, Vec<_>) =
                order.into_iter().partition(|id| selected.contains(id));
            back.extend(front);
            order = back;
        }
    }
    order
}

fn exactly_ordered(order: &[Uuid], ids: &[Uuid]) -> Vec<Uuid> {
    let mut rank: HashMap<Uuid, usize> = HashMap::new();
    for (index, id) in ids.iter().enumerate() {
        rank.entry(*id).or_insert(index);
    }
    let mut ranked: Vec<Uuid> = order
        .iter()
        .filter(|id| rank.contains_key(id))
        .copied()
        .collect();
    ranked.sort_by_key(|entity| rank.get(entity).copied().unwrap_or(usize::MAX));
    let mut ranked = ranked.into_iter();
    order
        .iter()
        .map(|id| match rank.contains_key(id) {
            true => ranked.next().unwrap_or(*id),
            false => *id,
        })
        .collect()
}

impl Root for Canvas {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_6669_6e69_7465_2d63_616e_7661_7301);

    fn name_in(_: &be_model::Document<Self>) -> Option<String> {
        None
    }

    fn references_of(_: &be_model::Document<Self>, _: Uuid) -> Vec<Uuid> {
        Vec::new()
    }

    fn describe_in(_: &be_model::Document<Self>, _: &mut be_block::BlockMetadata) {}

    fn undoable(change: &Change) -> bool {
        !matches!(
            change,
            Change::Set { object, field, .. }
                if *object == ObjectId::ROOT && *field == Self::PROFILE.index()
        )
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => {
                let mut changes: Vec<Change> = self
                    .child_entities(old)
                    .map(|held| Change::remove(held.id))
                    .collect();
                for held in self.entities.iter() {
                    for component in held.components.iter() {
                        if component.schema == Some(BlockRef(old)) {
                            changes.push(Change::remove(component.id));
                            continue;
                        }
                        for (field, value) in component.values.iter() {
                            if *value == DatabaseValue::Block(old) {
                                changes.push(Component::VALUES.put(component.id, field, None));
                            }
                        }
                    }
                }
                Some(Edit(changes))
            }
            ChildChange::Replace { old, new } => {
                let mut changes = Vec::new();
                for held in self.child_entities(old) {
                    changes.push(Entity::BLOCK.set(held.id, &Some(BlockRef(new))));
                }
                for held in self.entities.iter() {
                    let existing = held
                        .components
                        .iter()
                        .find(|component| component.schema == Some(BlockRef(new)));
                    for component in held.components.iter() {
                        if component.schema == Some(BlockRef(old)) {
                            match existing {
                                Some(existing) => {
                                    for (field, value) in component.values.iter() {
                                        if !existing.values.contains_key(field) {
                                            changes.push(Component::VALUES.put(
                                                existing.id,
                                                field,
                                                Some(value),
                                            ));
                                        }
                                    }
                                    changes.push(Change::remove(component.id));
                                }
                                None => {
                                    changes.push(
                                        Component::SCHEMA.set(component.id, &Some(BlockRef(new))),
                                    );
                                }
                            }
                        }
                        for (field, value) in component.values.iter() {
                            if *value == DatabaseValue::Block(old) {
                                changes.push(Component::VALUES.put(
                                    component.id,
                                    field,
                                    Some(&DatabaseValue::Block(new)),
                                ));
                            }
                        }
                    }
                }
                Some(Edit(changes))
            }
        }
    }
}

pub fn normalized_entity(mut entity: CanvasEntity) -> CanvasEntity {
    match entity.shape {
        CanvasShape::Editor => {
            entity.transform.rotation = 0.0;
            if !entity.scale.is_finite() || entity.scale <= 0.0 {
                entity.scale = 1.0;
            }
        }
        CanvasShape::Raster => entity.transform.rotation = 0.0,
        CanvasShape::Frame => {
            entity.transform.rotation = 0.0;
            let size = &mut entity.transform.size;
            size.x = match size.x.is_finite() {
                true => size.x.abs().max(1.0),
                false => 100.0,
            };
            size.y = match size.y.is_finite() {
                true => size.y.abs().max(1.0),
                false => 100.0,
            };
        }
        _ => {}
    }
    entity
}

impl CanvasEntity {
    pub fn new(shape: CanvasShape, transform: CanvasTransform) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            transform,
            shape,
            block: None,
            scale: 1.0,
            points: Vec::new(),
            pressures: Vec::new(),
            points_frame: None,
            bindings: [None, None],
            text: RichText::default(),
            placeholder: String::new(),
            text_style: CanvasTextStyle::default(),
            style: CanvasStyle {
                seed: id.as_u128() as u32,
                ..CanvasStyle::default()
            },
            hidden: false,
            group_id: None,
            locked: false,
            components: Vec::new(),
        }
    }

    pub fn raster(center: CanvasPoint, width: u16, height: u16) -> Self {
        Self::new(
            CanvasShape::Raster,
            CanvasTransform::raster(center, width, height),
        )
    }

    pub fn path(world: &[CanvasPoint]) -> Self {
        let mut path = Self::new(CanvasShape::Path, CanvasTransform::default());
        path.set_world_points(world.iter().copied().map(CanvasPathPoint::new).collect());
        path
    }

    pub fn is_frame(&self) -> bool {
        self.shape == CanvasShape::Frame
    }

    pub fn world_points(&self) -> Vec<CanvasPathPoint> {
        if self
            .points_frame
            .is_some_and(|frame| frame.fills(self.transform))
        {
            return self.points.clone();
        }
        self.points
            .iter()
            .map(|point| point.map(|at| self.transform.to_world(self.local_point(at))))
            .collect()
    }

    fn local_point(&self, at: CanvasPoint) -> CanvasPoint {
        self.points_frame.map_or(at, |frame| frame.normalized(at))
    }

    pub fn set_world_points(&mut self, world: Vec<CanvasPathPoint>) {
        let rotation = self.transform.rotation;
        let turned: Vec<CanvasPathPoint> = world
            .iter()
            .map(|point| point.map(|at| rotated(at, -rotation)))
            .collect();
        self.points_frame = None;
        let Some(frame) = CanvasPointsFrame::around(&turned) else {
            self.points.clear();
            return;
        };
        self.transform =
            CanvasTransform::new(rotated(frame.center, rotation), frame.size, rotation);
        self.points = turned
            .into_iter()
            .map(|point| point.map(|at| frame.normalized(at)))
            .collect();
    }

    pub fn set_drawn_points(&mut self, world: Vec<CanvasPathPoint>) {
        let Some(frame) = CanvasPointsFrame::around(&world) else {
            self.points.clear();
            self.points_frame = None;
            return;
        };
        self.transform = CanvasTransform::new(frame.center, frame.size, 0.0);
        self.points_frame = Some(frame);
        self.points = world;
    }

    pub fn followed<'a>(&self, target: impl Fn(Uuid) -> Option<&'a CanvasEntity>) -> Self {
        let mut followed = self.clone();
        followed.follow(|id| target(id).cloned());
        followed
    }

    pub fn follow(&mut self, target: impl Fn(Uuid) -> Option<CanvasEntity>) {
        if self.shape != CanvasShape::Path || self.points.len() < 2 {
            return;
        }
        let targets = self.bindings.map(|binding| {
            let binding = binding?;
            let target = target(binding.entity)
                .filter(|target| target.id != self.id && target.shape.bindable())?;
            let anchor = target.transform.to_world(binding.anchor);
            Some((target, anchor))
        });
        if targets.iter().all(Option::is_none) {
            return;
        }
        let mut world = self.world_points();
        let last = world.len() - 1;
        let ends = [0, last];
        let neighbours = [1, last - 1];
        for end in 0..2 {
            let Some((target, anchor)) = &targets[end] else {
                continue;
            };
            let neighbour = match (world.len(), &targets[1 - end]) {
                (2, Some((_, other))) => *other,
                _ => world[neighbours[end]].at(),
            };
            let reached = target.outline_toward(neighbour, *anchor);
            let point = &mut world[ends[end]];
            let shift = CanvasPoint::new(reached.x - point.x, reached.y - point.y);
            *point = point.map(|at| CanvasPoint::new(at.x + shift.x, at.y + shift.y));
        }
        self.set_world_points(world);
    }

    pub fn outline_toward(&self, from: CanvasPoint, to: CanvasPoint) -> CanvasPoint {
        let inside = |world: CanvasPoint| self.shape.contains_local(self.transform.to_local(world));
        if inside(from) || !inside(to) {
            return to;
        }
        let (mut outside, mut within) = (from, to);
        for _ in 0..24 {
            let middle =
                CanvasPoint::new((outside.x + within.x) * 0.5, (outside.y + within.y) * 0.5);
            match inside(middle) {
                true => within = middle,
                false => outside = middle,
            }
        }
        let delta = CanvasPoint::new(to.x - from.x, to.y - from.y);
        let length = delta.x.hypot(delta.y).max(f32::EPSILON);
        let gap = BINDING_GAP.min((from.x - outside.x).hypot(from.y - outside.y));
        CanvasPoint::new(
            outside.x - delta.x / length * gap,
            outside.y - delta.y / length * gap,
        )
    }
}

pub type CanvasContent = Document<Canvas>;
