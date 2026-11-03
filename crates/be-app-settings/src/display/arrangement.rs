use be_sdk_beui::HostDisplay;
use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::display_settings::{Arrangement, ScreenPlace};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, PointerPress, Pos2, Rect, Vec2, pos2, vec2};
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

struct Held {
    id: String,
    grab: Vec2,
    view: View,
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
    let dragged = create_signal(None::<(String, Rect)>);
    let frozen = create_signal(None::<View>);
    let shown = create_memo(move || {
        let mut tiles = laid.get();
        if let Some((id, rect)) = dragged.get() {
            for tile in &mut tiles {
                if tile.id == id {
                    tile.rect = rect;
                }
            }
        }
        tiles
    });
    let view = create_memo(move || {
        frozen.get().unwrap_or_else(|| {
            laid.with(|tiles| View::of(&tiles.iter().map(|tile| tile.rect).collect::<Vec<_>>()))
        })
    });
    let keys = create_memo(move || {
        laid.with(|tiles| tiles.iter().map(|tile| tile.id.clone()).collect::<Vec<_>>())
    });
    let main = create_memo(move || laid.with(|tiles| tiles.first().map(|tile| tile.id.clone())));
    let save = settings.setter(&DisplaySettings::arrangement_setting());
    let held = store(std::cell::RefCell::new(None::<Held>));
    let on_press = move |press: PointerPress| {
        if read_only.get_untracked() {
            return;
        }
        let view = view.get_untracked();
        let point = view.to_desktop(press.local);
        let hit = laid.with_untracked(|tiles| {
            tiles
                .iter()
                .find(|tile| tile.rect.contains(point))
                .map(|tile| (tile.id.clone(), point - tile.rect.min))
        });
        *held.value().borrow_mut() = hit.map(|(id, grab)| Held { id, grab, view });
    };
    let on_drag = move |press: PointerPress| {
        let moved = held.value().borrow().as_ref().map(|held| {
            let size = laid.with_untracked(|tiles| {
                tiles
                    .iter()
                    .find(|tile| tile.id == held.id)
                    .map_or(Vec2::ZERO, |tile| tile.rect.size())
            });
            let at = held.view.to_desktop(press.local) - held.grab;
            (held.id.clone(), Rect::from_min_size(at, size), held.view)
        });
        if let Some((id, rect, view)) = moved {
            frozen.set(Some(view));
            dragged.set(Some((id, rect)));
        }
    };
    let finish = move || {
        held.value().borrow_mut().take();
        frozen.set(None);
        let Some((id, rect)) = dragged.get_untracked() else {
            return;
        };
        dragged.set(None);
        let arrangement = laid.with_untracked(|tiles| {
            dropped(
                tiles,
                &id,
                rect,
                merged.with_untracked(DisplaySettings::main_screen),
            )
        });
        save.call(Some(arrangement));
    };
    view! {
        <SettingsRow
            label="Arrangement"
            description="Drag a display to where it stands. Displays meet edge to edge, and the pointer crosses where they meet."
        >
            <Frame
                @test_id={"display.arrangement"}
                width={Length::Size(Size::Custom(AREA.x))}
                height={Length::Size(Size::Custom(AREA.y))}
                fill=ColorRole::Background
                border={Border::DEFAULT}
                radius=Radius::Sm
            >
                <Interactive
                    on_press={on_press}
                    on_drag={on_drag}
                    on_click_at={move |_| finish()}
                    on_active_change={move |active: bool| if !active { finish() }}
                >
                    <Layers>
                        <ForEach keys={keys}>|id: String|
                            <Frame align=Align2::START>
                                <ArrangedTile id shown view main />
                            </Frame>
                        </ForEach>
                    </Layers>
                </Interactive>
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
) -> NodeId {
    let key = id.clone();
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
    let fill = create_memo(move || match is_main.get() {
        true => ColorRole::AccentSoft,
        false => ColorRole::Surface,
    });
    view! {
        <Frame
            @test_id={format!("display.arrangement.{connector}")}
            width={width}
            height={height}
            translate={translate}
            fill={fill}
            border={Border::DEFAULT}
            radius=Radius::Xs
            align=Align2::CENTER
            padding={Padding::all(Space::Xs)}
        >
            <List spacing=Space::None align=Align::Center>
                <Text size=TextSize::Xs align=TextAlign::Center>{name}</Text>
                <Show condition={is_main}>
                    <Text size=TextSize::Xs color=TextColor::Accent>"Main"</Text>
                </Show>
            </List>
        </Frame>
    }
}
