use be_sdk_beui::HostDisplay;
use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::display_settings::{Arrangement, ScreenPlace};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::drag::{self, DragSpec};
use be_sdk_beui::beui::{NodeId, PointerPress, Pos2, Rect, Vec2, pos2, vec2};
use beui::geometry::Side;
use beui::geometry::touching;
use beui::{Align, Align2, TextAlign};

use super::ui::Settings;

pub(super) const AREA: Vec2 = vec2(272.0, 160.0);
const MARGIN: f32 = 16.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tile {
    pub(super) id: String,
    pub(super) connector: String,
    pub(super) name: String,
    pub(super) rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct View {
    origin: Pos2,
    corner: Pos2,
    fit: f32,
}

impl View {
    pub(super) fn of(rects: &[Rect]) -> Self {
        let bounds = rects
            .iter()
            .copied()
            .reduce(|a, b| a.union(b))
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)));
        let room = AREA - Vec2::splat(MARGIN * 2.0);
        let fit = (room.x / bounds.width().max(1.0)).min(room.y / bounds.height().max(1.0));
        let shown = bounds.size() * fit;
        Self {
            origin: bounds.min,
            corner: pos2((AREA.x - shown.x) / 2.0, (AREA.y - shown.y) / 2.0),
            fit,
        }
    }

    pub(super) fn to_area(self, point: Pos2) -> Pos2 {
        self.corner + (point - self.origin) * self.fit
    }

    fn to_desktop(self, point: Pos2) -> Pos2 {
        self.origin + (point - self.corner) / self.fit
    }
}

pub(super) fn tiles(displays: &[HostDisplay], arrangement: &Arrangement) -> Vec<Tile> {
    let shown: Vec<&HostDisplay> = displays
        .iter()
        .filter(|display| display.rect.width() > 0.0 && display.rect.height() > 0.0)
        .collect();
    let placing: Vec<(Vec2, Option<Pos2>)> = shown
        .iter()
        .map(|display| {
            let place = arrangement
                .place(&display.id)
                .map(|(x, y)| pos2(x as f32, y as f32));
            (display.rect.size(), place)
        })
        .collect();
    shown
        .into_iter()
        .zip(touching::arrange(&placing))
        .map(|(display, rect)| Tile {
            id: display.id.clone(),
            connector: display.connector.clone(),
            name: display.name.clone(),
            rect,
        })
        .collect()
}

pub(super) fn dropped(tiles: &[Tile], id: &str, rect: Rect, main: Option<String>) -> Arrangement {
    let others: Vec<Rect> = tiles
        .iter()
        .filter(|tile| tile.id != id)
        .map(|tile| tile.rect)
        .collect();
    let at = rect.min;
    let rounded = Rect::from_min_size(pos2(at.x.round(), at.y.round()), rect.size());
    let settled = touching::settle(rounded, &others);
    let placing: Vec<(Vec2, Option<Pos2>)> = tiles
        .iter()
        .map(|tile| match tile.id == id {
            true => (settled.size(), Some(settled.min)),
            false => (tile.rect.size(), Some(tile.rect.min)),
        })
        .collect();
    let places = tiles
        .iter()
        .zip(touching::arrange(&placing))
        .map(|(tile, rect)| ScreenPlace {
            id: tile.id.clone(),
            x: rect.min.x.round() as i32,
            y: rect.min.y.round() as i32,
        })
        .collect();
    Arrangement { main, places }
}

/// A display carried in the arrangement, by id.
#[derive(Clone, Debug, PartialEq)]
struct Carried(String);

/// How far a carried display is pulled onto a place where it lines up with another, in
/// points on screen (not desktop pixels); beyond it, it moves freely and settles against
/// the nearest edge when let go.
const SNAP: f32 = 12.0;

/// The places a carried display can be let go: every place it lines up touching another
/// display (`touching::places`), and within `SNAP` of none, anywhere in the area. Rests are
/// in the area's points; what a place means is the display's rect on the desktop.
fn arrangement_spec(tiles: &[Tile], id: &str, view: View, area: NodeRef) -> DragSpec<Rect> {
    let Some(size) = tiles
        .iter()
        .find(|tile| tile.id == id)
        .map(|tile| tile.rect.size())
    else {
        return drag::floating();
    };
    let others: Vec<&Tile> = tiles.iter().filter(|tile| tile.id != id).collect();
    let rects: Vec<Rect> = others.iter().map(|tile| tile.rect).collect();
    let in_area = |rect: Rect| Rect::from_min_size(view.to_area(rect.min), rect.size() * view.fit);
    let places = touching::places(size, &rects).into_iter().map(|place| {
        drag::Candidate::new(place.rect, in_area(place.rect), area)
            .labelled(place_label(&others[place.beside].name, place.side, place.line))
    });
    drag::closest(places)
        .when_far(drag::free(area, move |rect: Rect| {
            Rect::from_min_size(view.to_desktop(rect.min), size)
        }))
        .with_distance(SNAP)
}

/// What a screen reader says of a place: "Right of Laptop, at its top".
fn place_label(name: &str, side: Side, line: touching::Line) -> String {
    let beside = match side {
        Side::Left => "Left of",
        Side::Right => "Right of",
        Side::Above => "Above",
        Side::Below => "Below",
    };
    let across = matches!(side, Side::Left | Side::Right);
    let at = match (line, across) {
        (touching::Line::Middle, _) => "at its middle",
        (touching::Line::Start, true) => "at its top",
        (touching::Line::End, true) => "at its bottom",
        (touching::Line::Start, false) => "at its left",
        (touching::Line::End, false) => "at its right",
    };
    format!("{beside} {name}, {at}")
}

#[component]
pub(super) fn ArrangementRow(
    settings: Settings,
    displays: Memo<Vec<HostDisplay>>,
    read_only: Memo<bool>,
) -> NodeId {
    let merged = settings.effective();
    let laid = create_memo(move || {
        let arrangement = merged.with(DisplaySettings::arrangement);
        displays.with(|displays| tiles(displays, &arrangement))
    });
    let view = create_memo(move || {
        laid.with(|tiles| View::of(&tiles.iter().map(|tile| tile.rect).collect::<Vec<_>>()))
    });
    let keys = create_memo(move || {
        laid.with(|tiles| tiles.iter().map(|tile| tile.id.clone()).collect::<Vec<_>>())
    });
    let main = create_memo(move || laid.with(|tiles| tiles.first().map(|tile| tile.id.clone())));
    let save = settings.setter(&DisplaySettings::arrangement_setting());
    let area = NodeRef::new();
    use_drop_site(
        Func::new(|_: Carried| true),
        Func::new(move |Carried(id): Carried| {
            laid.with_untracked(|tiles| {
                arrangement_spec(tiles, &id, view.get_untracked(), area)
            })
        }),
        Callback::new(move |(Carried(id), rect): (Carried, Rect)| {
            let arrangement = laid.with_untracked(|tiles| {
                dropped(
                    tiles,
                    &id,
                    rect,
                    merged.with_untracked(DisplaySettings::main_screen),
                )
            });
            save.call(Some(arrangement));
        }),
    );
    view! {
        <SettingsRow
            label="Arrangement"
            description="Drag a display to where it stands. Displays meet edge to edge, and the pointer crosses where they meet."
        >
            <Frame
                @node_ref=&area
                @test_id={"display.arrangement"}
                width={Length::Size(Size::Custom(AREA.x))}
                height={Length::Size(Size::Custom(AREA.y))}
                fill=ColorRole::Background
                border={Border::DEFAULT}
                radius=Radius::Sm
            >
                // A scroll that never overflows, for its clip: a display carried past the
                // area never draws over the rows around it.
                <Scroll>
                    <Frame
                        width={Length::Size(Size::Custom(AREA.x))}
                        height={Length::Size(Size::Custom(AREA.y))}
                    >
                        <Layers>
                            <ForEach value={keys}>|id: String|
                                <Frame align=Align2::START>
                                    <ArrangedTile id shown={laid} view main read_only />
                                </Frame>
                            </ForEach>
                        </Layers>
                    </Frame>
                </Scroll>
            </Frame>
        </SettingsRow>
    }
}

#[component]
fn ArrangedTile(
    id: String,
    shown: Memo<Vec<Tile>>,
    view: Memo<View>,
    main: Memo<Option<String>>,
    read_only: Memo<bool>,
) -> NodeId {
    let key = id.clone();
    let carried = id.clone();
    let drag = Func::new(move |_: PointerPress| {
        (!read_only.get_untracked()).then(|| drag::Carry::new(Carried(carried.clone())))
    });
    let tile =
        create_memo(move || shown.with(|tiles| tiles.iter().find(|tile| tile.id == key).cloned()));
    let area = create_memo(move || {
        let view = view.get();
        tile.with(|tile| {
            tile.as_ref().map_or(Rect::ZERO, |tile| {
                Rect::from_min_size(view.to_area(tile.rect.min), tile.rect.size() * view.fit)
            })
        })
    });
    let width = create_memo(move || Length::Size(Size::Custom(area.get().width())));
    let height = create_memo(move || Length::Size(Size::Custom(area.get().height())));
    let translate = create_memo(move || area.get().min.to_vec2());
    let name = create_memo(move || {
        tile.with(|tile| {
            tile.as_ref()
                .map(|tile| tile.name.clone())
                .unwrap_or_default()
        })
    });
    let connector = tile.with_untracked(|tile| {
        tile.as_ref()
            .map(|tile| tile.connector.clone())
            .unwrap_or_default()
    });
    let is_main = create_memo(move || main.get().as_deref() == Some(id.as_str()));
    view! {
        <Frame translate={translate}>
            <Interactive drag={drag} focusable=true>
                <Frame
                    @test_id={format!("display.arrangement.{connector}")}
                    width={width}
                    height={height}
                    fill={move || match is_main.get() {
                        true => ColorRole::AccentSoft,
                        false => ColorRole::Surface,
                    }}
                    border={Border::DEFAULT}
                    radius=Radius::Xs
                    align=Align2::CENTER
                    padding={Padding::all(Space::Xs)}
                >
                    <List spacing=Space::None align=Align::Center>
                        <Text size=TextSize::Xs align=TextAlign::Center>{name}</Text>
                        <Show value={is_main}>
                            <Text size=TextSize::Xs color=TextColor::Accent>"Main"</Text>
                        </Show>
                    </List>
                </Frame>
            </Interactive>
        </Frame>
    }
}
