use beui::KeyAt;
use block_editor_beui::beui::icons::{
    ICON_BOXES, ICON_LAYOUT_GRID, ICON_MENU, ICON_PANEL_BOTTOM_OPEN, ICON_PLUS,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{Anchored, AnchoredHandle};
use block_editor_beui::beui::{Key, LayerPolicy, NodeId, Placement, Rect};
use block_editor_beui::{Editor, PickAnchor};
use block_shell::Workspace;
use uuid::Uuid;

use super::bar::BARS;
use super::launcher::{ProgramSearch, Programs};
use super::popup::BarPopup;
use super::sessions::Sessions;

const PANEL_WIDTH: Size = Size::Custom(400.0);
const SECTION_SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;

#[derive(Clone, Copy)]
pub(crate) struct Apps {
    editor: Editor,
    workspace: Workspace,
    programs: Programs,
    sessions: Sessions,
    open: Signal<Option<u32>>,
    buttons: Signal<Vec<u32>>,
}

impl Apps {
    pub(crate) fn new(workspace: Workspace) -> Self {
        let editor = workspace.editor();
        Self {
            editor,
            workspace,
            programs: Programs::new(editor),
            sessions: Sessions::new(workspace),
            open: create_signal(None),
            buttons: create_signal(Vec::new()),
        }
    }

    pub(crate) fn launches(&self) -> Memo<bool> {
        self.programs.offered
    }

    fn show(&self, display: Option<u32>) {
        if display.is_some() && self.open.get_untracked() != display {
            self.programs.opened();
        }
        self.open.set(display);
    }

    fn toggle(&self, display: u32) {
        match self.open.get_untracked() == Some(display) {
            true => self.show(None),
            false => self.show(Some(display)),
        }
    }

    fn close(&self, display: u32) {
        if self.open.get_untracked() == Some(display) {
            self.show(None);
        }
    }

    fn shown_on(&self, display: u32) -> Binding<bool> {
        let apps = *self;
        let open = self.open;
        Binding::new(
            create_memo(move || open.get() == Some(display)),
            move |shown: bool| match shown {
                true => apps.show(Some(display)),
                false => apps.close(display),
            },
        )
    }

    fn has_button(&self, display: u32) -> Memo<bool> {
        let buttons = self.buttons;
        create_memo(move || buttons.with(|buttons| buttons.contains(&display)))
    }
}

#[component]
pub(crate) fn AppsKey(apps: Apps) -> NodeId {
    Action::new("desktop.apps.toggle", "Apps", move || apps.toggle(0))
        .glyph(ICON_LAYOUT_GRID)
        .shortcut(Chord::tap(Key::Logo))
        .at(KeyAt::Window)
        .capture()
        .register();
    view! {
        <Frame />
    }
}

#[component]
pub(crate) fn AppsButton(apps: Apps, display: u32) -> NodeId {
    apps.buttons.update(|buttons| buttons.push(display));
    on_cleanup(move || {
        if !apps.buttons.is_alive() {
            return;
        }
        apps.buttons.update(|buttons| {
            if let Some(at) = buttons.iter().position(|shown| *shown == display) {
                buttons.remove(at);
            }
        })
    });
    let button = NodeRef::new();
    view! {
        <BarPopup
            @node_ref={&button}
            label="Apps"
            glyph={Some(ICON_LAYOUT_GRID)}
            icon_only=true
            width=PANEL_WIDTH
            open={apps.shown_on(display)}
            @test_id={"desktop.apps"}
        >|handle: PopoverHandle|
            <AppsPanel
                apps
                display
                shown={handle.shown}
                close={handle.close}
                menu={PickAnchor::Node(button)}
            />
        </BarPopup>
    }
}

#[component]
pub(crate) fn AppsOverlay(apps: Apps, display: u32, area: Memo<Rect>) -> NodeId {
    let open = apps.open;
    let buttoned = apps.has_button(display);
    let shown = create_memo(move || open.get() == Some(display) && !buttoned.get());
    let surface = popover_surface(Some(PANEL_WIDTH));
    let close = Callback::new(move |()| apps.close(display));
    view! {
        <Anchored
            style={surface}
            anchor={create_memo(move || OverlayAnchor::Rect(area.get()))}
            placement=Placement::CenterOn
            policy=LayerPolicy::Pass
            open={shown}
            on_dismiss={move || apps.close(display)}
        >|handle: AnchoredHandle|
            <AppsPanel apps display shown={handle.shown} close menu={PickAnchor::Centre} />
        </Anchored>
    }
}

#[component]
fn AppsPanel(
    apps: Apps,
    display: u32,
    shown: Memo<bool>,
    close: Callback<()>,
    menu: PickAnchor,
) -> NodeId {
    let launches = apps.programs.offered;
    let editor = apps.editor;
    let layout = apps.workspace.layout();
    let hidden =
        create_memo(move || display == 0 && BARS.iter().any(|bar| layout.is_bar_hidden(bar)));
    let show_bars = move || {
        close.call(());
        for bar in BARS {
            layout.show_bar(&bar);
        }
    };
    let open_menu = move || {
        close.call(());
        editor.show_app_menu(menu);
    };
    view! {
        <List spacing=SECTION_SPACING @test_id={"desktop.apps.panel"}>
            <Show condition={launches}>
                <ProgramSearch programs={apps.programs} focused={shown} close />
            </Show>
            <SessionRows apps close />
            <Separator />
            <Show condition={hidden}>
                <ListRow
                    @test_id={"desktop.apps.bars"}
                    label="Show this display's bars"
                    glyph={Some(ICON_PANEL_BOTTOM_OPEN)}
                    size=RowSize::Large
                    on_click={show_bars}
                />
            </Show>
            <ListRow
                @test_id={"desktop.apps.menu"}
                label="Menu"
                glyph={Some(ICON_MENU)}
                trailing=RowTrailing::Chevron
                size=RowSize::Large
                on_click={open_menu}
            />
        </List>
    }
}

#[component]
fn SessionRows(apps: Apps, close: Callback<()>) -> NodeId {
    let sessions = apps.sessions;
    let listed = sessions.listed;
    let keys = create_memo(move || {
        listed.with(|sessions| {
            sessions
                .iter()
                .map(|session| session.block)
                .collect::<Vec<_>>()
        })
    });
    let settings = sessions.settings;
    let unready = create_memo(move || settings.get().is_none());
    let why = create_memo(move || match unready.get() {
        true => "Needs this workspace's settings block".to_owned(),
        false => String::new(),
    });
    let create = move || {
        close.call(());
        sessions.create();
    };
    view! {
        <List spacing=ROW_SPACING>
            <Text style=TextStyle::CAPTION>"Sessions"</Text>
            <ForEach keys={keys}>|block: Uuid|
                <SessionRow apps block close />
            </ForEach>
            <ListRow
                @test_id={"desktop.apps.sessions.new"}
                label="New session"
                detail={why}
                glyph={Some(ICON_PLUS)}
                size=RowSize::Large
                disabled={unready}
                on_click={create}
            />
        </List>
    }
}

#[component]
fn SessionRow(apps: Apps, block: Uuid, close: Callback<()>) -> NodeId {
    let listed = apps.sessions.listed;
    let workspace = apps.workspace;
    let label = create_memo(move || {
        listed.with(|sessions| {
            sessions
                .iter()
                .find(|session| session.block == block)
                .map(|session| session.name.clone())
                .unwrap_or_default()
        })
    });
    let detail = create_memo(move || match workspace.shows(block) {
        true => "Open".to_owned(),
        false => String::new(),
    });
    let choose = move || {
        close.call(());
        apps.sessions.open(block);
    };
    view! {
        <ListRow
            @test_id={format!("desktop.apps.sessions.{block}")}
            label
            detail
            glyph={Some(ICON_BOXES)}
            size=RowSize::Large
            on_click={choose}
        />
    }
}
