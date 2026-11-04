use be_sdk_beui::beui::accesskit::{Node, Role, Toggled};
use be_sdk_beui::beui::dock::TabId;
use be_sdk_beui::beui::dock_host::unstyled::{DockArea, holding_area};
use be_sdk_beui::beui::icons::{
    ICON_APP_WINDOW, ICON_ARCHIVE_RESTORE, ICON_EXTERNAL_LINK, ICON_EYE, ICON_EYE_OFF,
    ICON_MONITOR, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::button::ButtonFace;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::unstyled;
use be_sdk_beui::beui::{Align, Direction, Extent, NodeId, Pos2, Rect};
use be_sdk_beui::{DisplaysWatch, HostWindowId};
use be_sdk_shell::Workspace;
use std::collections::HashMap;

use super::bar::bar_bounds;

const CHIP_SPACING: Space = Space::Xs;
/// The row asks for this much per chip, so a band gives it room as if every title were long; the
/// row then measures the room it got, never its content, so switching to icon-only chips
/// (below `CHIP_LEAST` each) does not shrink what it asks for.
const CHIP_WIDTH: Size = Size::Menu;
const CHIP_LEAST: Size = Size::Custom(2.0 * Size::Xl2.px());
const BELL_FLASH: std::time::Duration = std::time::Duration::from_millis(400);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ChipKey {
    Window(TabId, HostWindowId),
    Placeholder(TabId),
    Block(TabId),
}

impl ChipKey {
    fn test_id(self) -> String {
        match self {
            ChipKey::Window(_, id) => format!("desktop.windows.{}", id.0),
            ChipKey::Placeholder(tab) => format!("desktop.windows.placeholder.{}", tab.value()),
            ChipKey::Block(tab) => format!("desktop.windows.block.{}", tab.value()),
        }
    }

    fn tab(self) -> TabId {
        match self {
            ChipKey::Window(tab, _) | ChipKey::Placeholder(tab) | ChipKey::Block(tab) => tab,
        }
    }
}

fn program_chips(workspace: Workspace, tabs: &[TabId]) -> Vec<ChipKey> {
    let mut displays = Vec::new();
    let mut windows = Vec::new();
    let mut waiting = Vec::new();
    for tab in tabs {
        match workspace.program_window(*tab) {
            Some(window) if workspace.is_display(*tab) => {
                displays.push(ChipKey::Window(*tab, window));
            }
            Some(window) => windows.push(ChipKey::Window(*tab, window)),
            None => waiting.push(ChipKey::Placeholder(*tab)),
        }
    }
    displays.into_iter().chain(windows).chain(waiting).collect()
}

pub(crate) fn holder(window: Option<Rect>, displays: &[Rect]) -> u32 {
    let (Some(window), Some(first)) = (window, displays.first()) else {
        return 0;
    };
    let window = window.translate(first.min.to_vec2());
    holding_area(window, displays).unwrap_or(0) as u32
}

#[derive(Clone, Copy)]
pub(crate) struct DeskWindows {
    workspace: Workspace,
    all: Memo<Vec<ChipKey>>,
    displays: Memo<Vec<Rect>>,
    bands: Signal<HashMap<u32, Rect>>,
}

impl DeskWindows {
    pub(crate) fn new(workspace: Workspace) -> Self {
        let programs = workspace.program_tabs();
        let tabs = workspace.block_tabs();
        let all = create_memo(move || {
            let programs = programs.get();
            program_chips(workspace, &programs)
                .into_iter()
                .chain(tabs.get().into_iter().map(ChipKey::Block))
                .collect()
        });
        let watched = workspace.editor().watch::<DisplaysWatch>();
        let displays = create_memo(move || {
            watched.with(|displays| displays.iter().map(|shown| shown.rect).collect())
        });
        let bands = create_signal(HashMap::new());
        let host = workspace.dock_host();
        let areas = create_memo(move || {
            displays.with(|displays: &Vec<Rect>| {
                let Some(first) = displays.first().map(|first| first.min) else {
                    return Vec::new();
                };
                displays
                    .iter()
                    .enumerate()
                    .map(|(at, display)| {
                        let whole = Rect::from_min_size(Pos2::ZERO, display.size());
                        let inside = match at {
                            0 => Some(host.area()),
                            _ => bands.with(|bands: &HashMap<u32, Rect>| {
                                bands.get(&(at as u32)).copied()
                            }),
                        };
                        let inside = inside.filter(|inside| inside.is_positive());
                        let screen = whole.translate(display.min - first);
                        let work = match (at, inside) {
                            (0, Some(inside)) => inside,
                            (_, inside) => inside.unwrap_or(whole).translate(display.min - first),
                        };
                        DockArea { screen, work }
                    })
                    .collect::<Vec<DockArea>>()
            })
        });
        create_effect(move || {
            let areas = areas.get();
            untrack(|| host.set_areas(areas));
        });
        Self {
            workspace,
            all,
            displays,
            bands,
        }
    }

    pub(crate) fn report_area(&self, display: u32, area: Memo<Rect>) {
        let bands = self.bands;
        create_effect(move || {
            let area = area.get();
            bands.update(|bands| {
                bands.insert(display, area);
            });
        });
        on_cleanup(move || {
            if bands.is_alive() {
                bands.update(|bands| {
                    bands.remove(&display);
                });
            }
        });
    }

    pub(crate) fn displays(&self) -> Vec<Rect> {
        self.displays.get()
    }

    fn chips(&self, display: Option<u32>) -> Memo<Vec<ChipKey>> {
        let windows = *self;
        create_memo(move || {
            let chips = windows.all.get();
            let Some(display) = display else {
                return chips;
            };
            chips
                .into_iter()
                .filter(|chip| windows.display_of(*chip) == display)
                .collect()
        })
    }

    fn display_of(&self, chip: ChipKey) -> u32 {
        let rect = match self.shown(chip) {
            true => self.workspace.window_rect(self.tab(chip)),
            false => None,
        };
        self.displays.with(|displays| holder(rect, displays))
    }

    fn targets(&self, chip: ChipKey) -> Memo<Vec<u32>> {
        let windows = *self;
        create_memo(move || {
            let count = windows.displays.with(Vec::len) as u32;
            let floating =
                windows.shown(chip) && windows.workspace.window_rect(windows.tab(chip)).is_some();
            if count < 2 || !floating {
                return Vec::new();
            }
            let own = windows.display_of(chip);
            (0..count).filter(|display| *display != own).collect()
        })
    }

    fn move_to(&self, chip: ChipKey, display: u32) {
        self.workspace.on_dock_host(self.tab(chip), |host, tab| {
            host.move_to_area(tab, display as usize);
        });
    }

    fn tab(&self, chip: ChipKey) -> TabId {
        chip.tab()
    }

    fn shown(&self, chip: ChipKey) -> bool {
        match chip {
            ChipKey::Placeholder(_) => true,
            ChipKey::Window(tab, _) | ChipKey::Block(tab) => !self.workspace.is_hidden(tab),
        }
    }

    fn focused(&self, chip: ChipKey) -> bool {
        self.shown(chip) && self.workspace.layout().focused() == Some(self.tab(chip))
    }

    fn choose(&self, chip: ChipKey) {
        if let ChipKey::Placeholder(tab) = chip {
            self.workspace.layout().show(&tab);
            return;
        }
        match untrack(|| self.focused(chip)) {
            true => self.hide(chip),
            false => self.show(chip),
        }
    }

    fn show(&self, chip: ChipKey) {
        self.workspace.show(self.tab(chip));
    }

    fn hide(&self, chip: ChipKey) {
        if !matches!(chip, ChipKey::Placeholder(_)) {
            self.workspace.hide(self.tab(chip));
        }
    }

    fn close(&self, chip: ChipKey) {
        match chip {
            ChipKey::Placeholder(tab) => self.workspace.dismiss(tab),
            ChipKey::Window(tab, _) | ChipKey::Block(tab) => self.workspace.close(tab),
        }
    }
}

#[component]
pub(crate) fn WindowChips(
    windows: DeskWindows,
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
    #[prop(default = None)] display: Option<u32>,
) -> NodeId {
    let chips = windows.chips(display);
    let vertical = create_memo(move || direction.get() == Direction::Vertical);
    let workspace = windows.workspace;
    let kept = workspace.placeholders();
    let launches = workspace.launchable();
    let restoring = display.is_none_or(|display| display == 0);
    let waiting = create_memo(move || restoring && launches.get() && !kept.with(Vec::is_empty));
    let widest = use_size(CHIP_WIDTH);
    let least = use_size(CHIP_LEAST);
    let spacing = use_space(CHIP_SPACING);
    let count = create_memo(move || chips.with(Vec::len));
    let listed = create_memo(move || count.get() > 0);
    let empty = create_memo(move || !listed.get() && !waiting.get() && !vertical.get());
    let gaps = move || spacing.get() * count.get().saturating_sub(1) as f32;
    let wanted = create_memo(move || match vertical.get() {
        true => Extent::INTRINSIC,
        false => Extent::at_most(widest.get() * count.get() as f32 + gaps()),
    });
    let row = NodeRef::new();
    let room = create_signal(None::<f32>);
    let compact = create_memo(move || {
        let Some(room) = room.get() else {
            return false;
        };
        let count = count.get();
        count > 0 && (room - gaps()) / (count as f32) < least.get()
    });
    let icons = create_memo(move || vertical.get() || compact.get());
    let chip_grow = create_memo(move || match icons.get() {
        true => 0.0,
        false => 1.0,
    });
    let chip_max = create_memo(move || match icons.get() {
        true => f32::INFINITY,
        false => widest.get(),
    });
    let chips_row = view! {
        <List @node_ref=&row direction align=Align::Center spacing=CHIP_SPACING>
            <ForEach keys={chips}>|chip: ChipKey|
                <ListItem grow=chip_grow max=chip_max>
                    <WindowChip windows chip icons count />
                </ListItem>
            </ForEach>
        </List>
    };
    let measured = measure(row.get());
    create_effect(move || {
        let width = measured.with(|measured| measured.map(|measured| measured.size.x));
        if width != room.get_untracked() {
            room.set(width);
        }
    });
    view! {
        <List direction align=Align::Center spacing=CHIP_SPACING @test_id={"desktop.windows"}>
            <Show condition={waiting}>
                <RestoreChip workspace vertical />
            </Show>
            <Show condition={listed}>
                <ListItem shrink=1.0>
                    <Scroll direction fit=true>
                        <ListItem shrink=1.0>
                            <Layers>
                                <Frame width={wanted} />
                                {chips_row}
                            </Layers>
                        </ListItem>
                    </Scroll>
                </ListItem>
            </Show>
            <Show condition={empty}>
                <Frame
                    width={Length::Size(CHIP_WIDTH)}
                    align=Align::Center
                    @test_id={"desktop.windows.none"}
                >
                    <Text size=TextSize::Sm color=TextColor::Muted>"No windows"</Text>
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn RestoreChip(workspace: Workspace, vertical: Memo<bool>) -> NodeId {
    let keys = workspace.placeholders();
    let label = create_memo(move || match (vertical.get(), keys.with(Vec::len)) {
        (false, count) => format!("Restore {count}"),
        (true, 1) => "Restore the window from last time".to_owned(),
        (true, count) => format!("Restore the {count} windows from last time"),
    });
    view! {
        <Button
            label
            glyph=ICON_ARCHIVE_RESTORE
            icon_only={vertical}
            variant=ButtonVariant::Primary
            size=ButtonSize::Compact
            press_focus=false
            @test_id={"desktop.windows.restore"}
            on_click={move || workspace.open_placeholders()}
        />
    }
}

fn bell_flash(rung: Memo<u64>) -> ReadSignal<bool> {
    let ringing = create_signal(false);
    let quiet = create_timer(move || {
        ringing.set(false);
        None
    });
    let heard = StoredCell::new(rung.get_untracked());
    create_effect(move || {
        let count = rung.get();
        if count > heard.get() {
            untrack(|| {
                ringing.set(true);
                quiet.start(BELL_FLASH);
            });
        }
        heard.set(count);
    });
    ringing.read_only()
}

#[component]
fn WindowChip(
    windows: DeskWindows,
    chip: ChipKey,
    icons: Memo<bool>,
    count: Memo<usize>,
) -> NodeId {
    let workspace = windows.workspace;
    let tab = chip.tab();
    let waiting = matches!(chip, ChipKey::Placeholder(_));
    let title = workspace.tab_title(tab);
    let display = match chip {
        ChipKey::Window(tab, _) => untrack(|| workspace.is_display(tab)),
        ChipKey::Placeholder(_) | ChipKey::Block(_) => false,
    };
    let tab_glyph = workspace.tab_glyph(tab);
    let glyph = create_memo(move || tab_glyph.get().or(Some(ICON_APP_WINDOW)));
    let image = workspace.tab_picture(tab);
    let shown = create_memo(move || windows.shown(chip));
    let focused = create_memo(move || windows.focused(chip));
    let hollow = create_memo(move || waiting || !shown.get());
    let running = create_memo(move || !waiting);
    let closable = create_memo(move || !waiting && !display);
    let openable = workspace.placeholder_openable(tab);
    let unopenable = create_memo(move || !openable.get());
    let ringing = bell_flash(workspace.tab_rung(tab));
    let variant = create_memo(move || match (ringing.get(), shown.get()) {
        (true, _) => ButtonVariant::Primary,
        (false, true) => ButtonVariant::Secondary,
        (false, false) => ButtonVariant::Ghost,
    });
    let visibility = create_memo(move || match shown.get() {
        true => "Hide".to_owned(),
        false => "Show".to_owned(),
    });
    let visibility_glyph = create_memo(move || match shown.get() {
        true => Some(ICON_EYE_OFF),
        false => Some(ICON_EYE),
    });
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::Button);
        node.set_toggled(Toggled::from(focused.get()));
        let state = match (waiting, shown.get()) {
            (true, _) => Some("Not running"),
            (false, false) => Some("Hidden"),
            (false, true) => None,
        };
        if let Some(state) = state {
            node.set_description(state);
        }
        node
    });
    let face = NodeRef::new();
    create_effect(move || {
        count.get();
        icons.get();
        if focused.get() {
            untrack(|| face.get().reveal());
        }
    });
    let prefix = chip.test_id();
    let close_id = format!("{prefix}.close");
    let visibility_id = format!("{prefix}.visibility");
    let open_id = format!("{prefix}.open");
    let dismiss_id = format!("{prefix}.dismiss");
    let targets = windows.targets(chip);
    let display_prefix = prefix.clone();
    view! {
        <ContextMenu
            bounds={bar_bounds()}
            items={view! {
                <Show condition={running}>
                    <MenuItem
                        label={visibility}
                        glyph={visibility_glyph}
                        row_test_id={visibility_id.clone()}
                        on_click={move || match shown.get_untracked() {
                            true => windows.hide(chip),
                            false => windows.show(chip),
                        }}
                    />
                </Show>
                <Show condition=waiting>
                    <MenuItem
                        label="Open"
                        glyph=ICON_EXTERNAL_LINK
                        disabled={unopenable}
                        row_test_id={open_id.clone()}
                        on_click={move || workspace.open_placeholder(tab)}
                    />
                    <MenuItem
                        label="Dismiss"
                        glyph=ICON_X
                        row_test_id={dismiss_id.clone()}
                        on_click={move || windows.close(chip)}
                    />
                </Show>
                <ForEach keys={targets}>|target: u32|
                    <MenuItem
                        label={format!("Move to display {}", target + 1)}
                        glyph=ICON_MONITOR
                        row_test_id={format!("{display_prefix}.display.{target}")}
                        on_click={move || windows.move_to(chip, target)}
                    />
                </ForEach>
                <Show condition={closable}>
                    <MenuItem
                        label="Close"
                        glyph=ICON_X
                        row_test_id={close_id.clone()}
                        on_click={move || windows.close(chip)}
                    />
                </Show>
            }}
        >
            <unstyled::Button
                label={title}
                glyph
                icon_only={icons}
                press_focus=false
                accessibility
                @node_ref=&face
                @test_id={prefix.clone()}
                on_click={move || windows.choose(chip)}
            >|handle|
                <ButtonFace
                    handle
                    variant={variant}
                    size=ButtonSize::Compact
                    image
                    pressed={focused}
                    placeholder={hollow}
                />
            </unstyled::Button>
        </ContextMenu>
    }
}
