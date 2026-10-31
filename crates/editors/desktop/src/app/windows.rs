use block_editor_beui::beui::dock::styled::TabId;
use block_editor_beui::beui::icons::{
    ICON_APP_WINDOW, ICON_ARCHIVE_RESTORE, ICON_EYE, ICON_EYE_OFF, ICON_MONITOR, ICON_X,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::button::ButtonFace;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled;
use block_editor_beui::beui::{Align, Direction, ItemSize, NodeId, Pos2, Rect, Vec2};
use block_editor_beui::desktop;
use block_editor_beui::{DisplaysWatch, HostWindow, HostWindowId, WindowAction, WindowKind};
use block_shell::{Workspace, window_name, window_picture};

use super::placeholders::Placeholders;

const CHIP_SPACING: Space = Space::Xs;
const CHIP_WIDTH: Size = Size::Menu;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ChipKey {
    Window(HostWindowId),
    Block(TabId),
}

impl ChipKey {
    fn test_id(self) -> String {
        match self {
            ChipKey::Window(id) => format!("desktop.windows.{}", id.0),
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
    tabs: Memo<Vec<TabId>>,
    displays: Memo<Vec<Rect>>,
    hidden: Signal<Vec<TabId>>,
    showing: Signal<Option<TabId>>,
}

impl DeskWindows {
    pub(crate) fn new(workspace: Workspace, placeholders: Placeholders) -> Self {
        let hidden = create_signal(Vec::new());
        let showing = create_signal(None::<TabId>);
        let layout = workspace.layout();
        create_effect(move || {
            let Some(tab) = showing.get() else {
                return;
            };
            if layout.contains(&tab) {
                untrack(|| {
                    showing.set(None);
                    layout.show(&tab);
                });
            }
        });
        let entries = workspace.entries();
        let tabs = workspace.block_tabs();
        let all = create_memo(move || {
            let windows = entries.with(|entries| picked(entries));
            windows
                .into_iter()
                .map(ChipKey::Window)
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
            tabs,
            displays,
            hidden,
            showing,
        }
    }

    pub(crate) fn displays(&self) -> Vec<Rect> {
        self.displays.get()
    }

    pub(crate) fn block_windows(&self) -> Memo<Vec<TabId>> {
        let tabs = self.tabs;
        let hidden = self.hidden;
        create_memo(move || {
            let mut shown = tabs.get();
            hidden.with(|hidden| shown.retain(|tab| !hidden.contains(tab)));
            shown
        })
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
            ChipKey::Block(tab) => tab,
        }
    }

    fn shown(&self, chip: ChipKey) -> bool {
        match chip {
            ChipKey::Window(id) => self
                .workspace
                .entries()
                .with(|entries| entries.iter().any(|entry| entry.id == id && entry.shown)),
            ChipKey::Block(tab) => self.hidden.with(|hidden| !hidden.contains(&tab)),
        }
    }

    fn focused(&self, chip: ChipKey) -> bool {
        self.shown(chip) && self.workspace.layout().focused() == Some(self.tab(chip))
    }

    fn choose(&self, chip: ChipKey) {
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
                    desktop::windows::request(WindowAction::Show(id));
                }
                desktop::windows::request(WindowAction::Focus(id));
            }),
            ChipKey::Block(tab) if hidden => {
                self.hidden
                    .update(|hidden| hidden.retain(|held| *held != tab));
                self.showing.set(Some(tab));
            }
            ChipKey::Block(tab) => self.workspace.layout().show(&tab),
        }
    }

    fn hide(&self, chip: ChipKey) {
        let next = untrack(|| self.next_after(chip));
        let layout = self.workspace.layout();
        match chip {
            ChipKey::Window(id) => {
                self.workspace
                    .editor()
                    .act(move || desktop::windows::request(WindowAction::Hide(id)));
                if let Some(next) = next {
                    layout.show(&next);
                }
            }
            ChipKey::Block(tab) => {
                if let Some(next) = next {
                    layout.show(&next);
                }
                self.hidden.update(|hidden| {
                    if !hidden.contains(&tab) {
                        hidden.push(tab);
                    }
                });
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
                .act(move || desktop::windows::request(WindowAction::Close(id))),
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
    view! {
        <List direction align=Align::Center spacing=CHIP_SPACING @test_id={"desktop.windows"}>
            <Show condition={waiting}>
                <RestoreChip placeholders vertical />
            </Show>
            <ForEach keys={chips}>|chip: ChipKey|
                <WindowChip
                    @sizing={ItemSize::Intrinsic.max(CHIP_WIDTH.px()).shrink(1.0)}
                    windows
                    chip
                    vertical
                />
            </ForEach>
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
fn WindowChip(windows: DeskWindows, chip: ChipKey, vertical: Memo<bool>) -> NodeId {
    let workspace = windows.workspace;
    let entries = workspace.entries();
    let entry = create_memo(move || match chip {
        ChipKey::Window(id) => {
            entries.with(|entries| entries.iter().find(|entry| entry.id == id).cloned())
        }
        ChipKey::Block(_) => None,
    });
    let block = match chip {
        ChipKey::Block(tab) => Some((workspace.tab_title(tab), workspace.tab_glyph(tab))),
        ChipKey::Window(_) => None,
    };
    let title = create_memo(move || match block {
        Some((title, _)) => title.get(),
        None => entry
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
    let image = create_memo(move || {
        entry.with(|entry| entry.as_ref()?.icon.as_ref().and_then(window_picture))
    });
    let shown = create_memo(move || windows.shown(chip));
    let focused = create_memo(move || windows.focused(chip));
    let hollow = create_memo(move || !shown.get());
    let closable = create_memo(move || !display.get());
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
    let prefix = chip.test_id();
    let close_id = format!("{prefix}.close");
    let targets = windows.targets(chip);
    let display_prefix = prefix.clone();
    view! {
        <ContextMenu
            items={view! {
                <MenuItem
                    label={visibility}
                    glyph={visibility_glyph}
                    row_test_id={format!("{prefix}.visibility")}
                    on_click={move || match shown.get_untracked() {
                        true => windows.hide(chip),
                        false => windows.show(chip),
                    }}
                />
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
                icon_only={vertical}
                press_focus=false
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
