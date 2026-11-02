use be_sdk_beui::beui::accesskit::{Node, Role, Toggled};
use be_sdk_beui::beui::dock::styled::TabId;
use be_sdk_beui::beui::icons::{
    ICON_APP_WINDOW, ICON_ARCHIVE_RESTORE, ICON_EXTERNAL_LINK, ICON_EYE, ICON_EYE_OFF,
    ICON_MONITOR, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::button::ButtonFace;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::unstyled;
use be_sdk_beui::beui::{Align, Direction, Extent, ItemSize, NodeId, Pos2, Rect, Sizing, Vec2};
use be_sdk_beui::desktop;
use be_sdk_beui::{DisplaysWatch, HostWindow, HostWindowId, WindowAction, WindowKind};
use be_sdk_shell::{Workspace, window_name, window_picture, window_tab};

use super::bar::bar_bounds;
use super::placeholders::Placeholders;

const CHIP_SPACING: Space = Space::Xs;
const CHIP_WIDTH: Size = Size::Menu;
const CHIP_LEAST: Size = Size::Custom(2.0 * Size::Xl2.px());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ChipKey {
    Window(HostWindowId),
    Placeholder(HostWindowId),
    Block(TabId),
}

impl ChipKey {
    fn test_id(self) -> String {
        match self {
            ChipKey::Window(id) => format!("desktop.windows.{}", id.0),
            ChipKey::Placeholder(key) => format!("desktop.windows.placeholder.{}", key.0),
            ChipKey::Block(tab) => format!("desktop.windows.block.{}", tab.value()),
        }
    }
}

pub(crate) fn picked(entries: &[HostWindow]) -> Vec<HostWindowId> {
    let top = entries.iter().filter(|entry| entry.parent.is_none());
    let displays = top
        .clone()
        .filter(|entry| entry.kind == WindowKind::Display);
    let windows = top.filter(|entry| entry.kind == WindowKind::Window);
    displays.chain(windows).map(|entry| entry.id).collect()
}

fn overlap(left: Rect, right: Rect) -> f32 {
    let shared = left.intersect(right);
    shared.width().max(0.0) * shared.height().max(0.0)
}

pub(crate) fn holder(window: Option<Rect>, displays: &[Rect]) -> u32 {
    let (Some(window), Some(first)) = (window, displays.first()) else {
        return 0;
    };
    let window = window.translate(first.min.to_vec2());
    let mut best = (0, 0.0);
    for (at, display) in displays.iter().enumerate() {
        let shared = overlap(window, *display);
        if shared > best.1 {
            best = (at as u32, shared);
        }
    }
    best.0
}

pub(crate) fn moved_onto(window: Rect, from: Rect, to: Rect) -> Pos2 {
    let along = |offset: f32, length: f32| match length > 0.0 {
        true => offset / length,
        false => 0.0,
    };
    let offset = window.min - from.min;
    let wanted = to.min
        + Vec2::new(
            along(offset.x, from.width()) * to.width(),
            along(offset.y, from.height()) * to.height(),
        );
    let room = to.max - window.size();
    Pos2::new(
        wanted.x.min(room.x).max(to.min.x),
        wanted.y.min(room.y).max(to.min.y),
    )
}

#[derive(Clone, Copy)]
pub(crate) struct DeskWindows {
    workspace: Workspace,
    pub(crate) placeholders: Placeholders,
    all: Memo<Vec<ChipKey>>,
    displays: Memo<Vec<Rect>>,
}

impl DeskWindows {
    pub(crate) fn new(workspace: Workspace, placeholders: Placeholders) -> Self {
        let entries = workspace.entries();
        let tabs = workspace.block_tabs();
        let kept = placeholders.keys;
        let all = create_memo(move || {
            let windows = entries.with(|entries| picked(entries));
            windows
                .into_iter()
                .map(ChipKey::Window)
                .chain(kept.get().into_iter().map(ChipKey::Placeholder))
                .chain(tabs.get().into_iter().map(ChipKey::Block))
                .collect()
        });
        let watched = workspace.editor().watch::<DisplaysWatch>();
        let displays = create_memo(move || {
            watched.with(|displays| displays.iter().map(|shown| shown.rect).collect())
        });
        Self {
            workspace,
            placeholders,
            all,
            displays,
        }
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
            true => self.workspace.layout().window_rect(&self.tab(chip)),
            false => None,
        };
        self.displays.with(|displays| holder(rect, displays))
    }

    fn targets(&self, chip: ChipKey) -> Memo<Vec<u32>> {
        let windows = *self;
        create_memo(move || {
            let count = windows.displays.with(Vec::len) as u32;
            let floating = windows.shown(chip)
                && windows
                    .workspace
                    .layout()
                    .window_rect(&windows.tab(chip))
                    .is_some();
            if count < 2 || !floating {
                return Vec::new();
            }
            let own = windows.display_of(chip);
            (0..count).filter(|display| *display != own).collect()
        })
    }

    fn move_to(&self, chip: ChipKey, display: u32) {
        let layout = self.workspace.layout();
        let tab = self.tab(chip);
        let Some(rect) = untrack(|| layout.window_rect(&tab)) else {
            return;
        };
        let from = untrack(|| self.display_of(chip));
        let placed = self.displays.with_untracked(|displays| {
            let first = displays.first()?.min.to_vec2();
            let from = *displays.get(from as usize)?;
            let to = *displays.get(display as usize)?;
            Some(moved_onto(rect.translate(first), from, to) - first)
        });
        if let Some(placed) = placed {
            layout.move_window(&tab, placed);
        }
    }

    fn tab(&self, chip: ChipKey) -> TabId {
        match chip {
            ChipKey::Window(id) => self.workspace.window_tab_of(id),
            ChipKey::Placeholder(key) => window_tab(key),
            ChipKey::Block(tab) => tab,
        }
    }

    fn shown(&self, chip: ChipKey) -> bool {
        match chip {
            ChipKey::Window(id) => self
                .workspace
                .entries()
                .with(|entries| entries.iter().any(|entry| entry.id == id && entry.shown)),
            ChipKey::Placeholder(_) => true,
            ChipKey::Block(tab) => !self.workspace.is_hidden(tab),
        }
    }

    fn focused(&self, chip: ChipKey) -> bool {
        self.shown(chip) && self.workspace.layout().focused() == Some(self.tab(chip))
    }

    fn choose(&self, chip: ChipKey) {
        if let ChipKey::Placeholder(key) = chip {
            self.workspace.layout().show(&window_tab(key));
            return;
        }
        match untrack(|| self.focused(chip)) {
            true => self.hide(chip),
            false => self.show(chip),
        }
    }

    fn show(&self, chip: ChipKey) {
        let hidden = untrack(|| !self.shown(chip));
        match chip {
            ChipKey::Window(id) => self.workspace.editor().act(move || {
                if hidden {
                    desktop::windows::request(WindowAction::Show(id)).detach();
                }
                desktop::windows::request(WindowAction::Focus(id)).detach();
            }),
            ChipKey::Placeholder(key) => self.workspace.layout().show(&window_tab(key)),
            ChipKey::Block(tab) => self.workspace.show(tab),
        }
    }

    fn hide(&self, chip: ChipKey) {
        let next = untrack(|| self.next_after(chip));
        let layout = self.workspace.layout();
        match chip {
            ChipKey::Window(id) => {
                self.workspace
                    .editor()
                    .act(move || desktop::windows::request(WindowAction::Hide(id)).detach());
                if let Some(next) = next {
                    layout.show(&next);
                }
            }
            ChipKey::Placeholder(_) => {}
            ChipKey::Block(tab) => {
                if let Some(next) = next {
                    layout.show(&next);
                }
                self.workspace.hide(tab);
            }
        }
    }

    fn next_after(&self, chip: ChipKey) -> Option<TabId> {
        let own = self.tab(chip);
        let others: Vec<TabId> = self
            .all
            .get()
            .into_iter()
            .filter(|other| *other != chip && self.shown(*other))
            .map(|other| self.tab(other))
            .collect();
        self.workspace
            .layout()
            .recent()
            .into_iter()
            .find(|tab| *tab != own && others.contains(tab))
    }

    fn close(&self, chip: ChipKey) {
        match chip {
            ChipKey::Window(id) => self
                .workspace
                .editor()
                .act(move || desktop::windows::request(WindowAction::Close(id)).detach()),
            ChipKey::Placeholder(key) => self.placeholders.dismiss(key),
            ChipKey::Block(tab) => self.workspace.close(tab),
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
    let placeholders = windows.placeholders;
    let kept = placeholders.keys;
    let launches = placeholders.launchable;
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
    let chip_size = create_memo(move || match icons.get() {
        true => Sizing::from(ItemSize::Intrinsic),
        false => ItemSize::Percent(1.0).max(widest.get()),
    });
    let chips_row = view! {
        <List @node_ref=&row direction align=Align::Center spacing=CHIP_SPACING>
            <ForEach keys={chips}>|chip: ChipKey|
                <WindowChip @sizing={chip_size} windows chip icons count />
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
                <RestoreChip placeholders vertical />
            </Show>
            <Show condition={listed}>
                <Scroll @sizing={ItemSize::Intrinsic.shrink(1.0)} direction fit=true>
                    <Layers @sizing={ItemSize::Intrinsic.shrink(1.0)}>
                        <Frame width={wanted} />
                        {chips_row}
                    </Layers>
                </Scroll>
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
fn RestoreChip(placeholders: Placeholders, vertical: Memo<bool>) -> NodeId {
    let keys = placeholders.keys;
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
            on_click={move || placeholders.open_all()}
        />
    }
}

#[component]
fn WindowChip(
    windows: DeskWindows,
    chip: ChipKey,
    icons: Memo<bool>,
    count: Memo<usize>,
) -> NodeId {
    let workspace = windows.workspace;
    let entries = workspace.entries();
    let entry = create_memo(move || match chip {
        ChipKey::Window(id) => {
            entries.with(|entries| entries.iter().find(|entry| entry.id == id).cloned())
        }
        ChipKey::Placeholder(_) | ChipKey::Block(_) => None,
    });
    let block = match chip {
        ChipKey::Block(tab) => Some((workspace.tab_title(tab), workspace.tab_glyph(tab))),
        ChipKey::Window(_) | ChipKey::Placeholder(_) => None,
    };
    let kept = match chip {
        ChipKey::Placeholder(key) => Some((key, windows.placeholders.shown(key))),
        ChipKey::Window(_) | ChipKey::Block(_) => None,
    };
    let waiting = kept.is_some();
    let title = create_memo(move || match (block, kept) {
        (Some((title, _)), _) => title.get(),
        (None, Some((key, _))) => workspace
            .placeholder(key)
            .map(|kept| kept.window_name())
            .unwrap_or_default(),
        (None, None) => entry
            .with(|entry| {
                let entry = entry.as_ref()?;
                let parent = entry.parent.and_then(|parent| {
                    entries
                        .with(|entries| entries.iter().find(|listed| listed.id == parent).cloned())
                });
                Some(window_name(entry, parent.as_ref()))
            })
            .unwrap_or_default(),
    });
    let display = create_memo(move || {
        entry.with(|entry| {
            entry
                .as_ref()
                .is_some_and(|entry| entry.kind == WindowKind::Display)
        })
    });
    let glyph = create_memo(move || match (block, display.get()) {
        (Some((_, glyph)), _) => glyph.get().or(Some(ICON_APP_WINDOW)),
        (None, true) => Some(ICON_MONITOR),
        (None, false) => Some(ICON_APP_WINDOW),
    });
    let image = create_memo(move || match kept {
        Some((_, shown)) => shown.picture.get(),
        None => entry.with(|entry| entry.as_ref()?.icon.as_ref().and_then(window_picture)),
    });
    let shown = create_memo(move || windows.shown(chip));
    let focused = create_memo(move || windows.focused(chip));
    let hollow = create_memo(move || waiting || !shown.get());
    let running = create_memo(move || !waiting);
    let closable = create_memo(move || !waiting && !display.get());
    let unopenable = create_memo(move || kept.is_some_and(|(_, shown)| !shown.openable.get()));
    let variant = create_memo(move || match shown.get() {
        true => ButtonVariant::Secondary,
        false => ButtonVariant::Ghost,
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
                        on_click={move || {
                            if let Some((key, _)) = kept {
                                windows.placeholders.open(key);
                            }
                        }}
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
