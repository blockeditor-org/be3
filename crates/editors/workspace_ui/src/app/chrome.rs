use std::rc::Rc;

use block::{BlockAccess, BlockParent, BlockReference};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_CHEVRON_RIGHT, ICON_DATA_OBJECT, ICON_EDIT,
    ICON_LOCK, ICON_REDO, ICON_SHARE, ICON_UNDO, ICON_VISIBILITY,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, ReadSignal, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::theme::FONT_SMALL;
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconButton, IconSized, MenuButton, use_theme,
};
use block_editor_plugin::beui::unstyled::{MenuItem, TabId};
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::{AccessLevel, Toolbar};

use super::panel::Info;
use super::tab::{Navigation, TabItem};
use super::workspace::Workspace;

const SPACING: f32 = 6.0;
const CRUMB_SPACING: f32 = 2.0;

#[component]
pub(crate) fn ChromeBar(
    workspace: Rc<Workspace>,
    tab: TabId,
    info: ReadSignal<Option<Info>>,
) -> NodeId {
    let back_off = create_memo(clone!(info -> move || {
        info.with(|info| !info.as_ref().is_some_and(|info| info.can_go_back))
    }));
    let forward_off = create_memo(clone!(info -> move || {
        info.with(|info| !info.as_ref().is_some_and(|info| info.can_go_forward))
    }));
    let undo_off = create_memo(clone!(info -> move || {
        info.with(|info| !info.as_ref().is_some_and(|info| info.can_undo))
    }));
    let redo_off = create_memo(clone!(info -> move || {
        info.with(|info| !info.as_ref().is_some_and(|info| info.can_redo))
    }));
    let share_off = create_memo(clone!(info -> move || {
        info.with(|info| !info.as_ref().is_some_and(|info| info.can_edit))
    }));
    let item =
        create_memo(clone!(info -> move || info.with(|info| info.as_ref().map(|info| info.item))));
    let back = Rc::clone(&workspace);
    let forward = Rc::clone(&workspace);
    let undo = Rc::clone(&workspace);
    let redo = Rc::clone(&workspace);
    let shared = Rc::clone(&workspace);
    let undo_item = item.clone();
    let redo_item = item.clone();
    let share_item = item.clone();
    view! {
        <Toolbar spacing=SPACING>
            <IconButton
                @test_id={"workspace.back"}
                glyph={ICON_ARROW_BACK.to_owned()}
                label="Back"
                disabled={back_off}
                on_click={move || back.navigate(tab, Navigation::Back)}
            />
            <IconButton
                @test_id={"workspace.forward"}
                glyph={ICON_ARROW_FORWARD.to_owned()}
                label="Forward"
                disabled={forward_off}
                on_click={move || forward.navigate(tab, Navigation::Forward)}
            />
            <IconButton
                @test_id={"workspace.undo"}
                glyph={ICON_UNDO.to_owned()}
                label="Undo (Ctrl/Cmd+Z)"
                disabled={undo_off}
                on_click={move || {
                    if let Some(item) = undo_item.get_untracked() {
                        history(&undo, item, false);
                    }
                }}
            />
            <IconButton
                @test_id={"workspace.redo"}
                glyph={ICON_REDO.to_owned()}
                label="Redo (Ctrl+Y or Ctrl/Cmd+Shift+Z)"
                disabled={redo_off}
                on_click={move || {
                    if let Some(item) = redo_item.get_untracked() {
                        history(&redo, item, true);
                    }
                }}
            />
            <Breadcrumbs
                @sizing=ItemSize::Percent(100.0)
                workspace={Rc::clone(&workspace)}
                tab={tab}
                info={info.clone()}
            />
            <Button
                @test_id={"workspace.share"}
                label="Share"
                glyph={ICON_SHARE.to_owned()}
                variant=ButtonVariant::Secondary
                disabled={share_off}
                on_click={move || {
                    if let Some(item) = share_item.get_untracked() {
                        shared.host().share_block(item.id);
                    }
                }}
            />
            <AccessMode workspace={workspace} info={info} />
        </Toolbar>
    }
}

fn history(workspace: &Rc<Workspace>, item: TabItem, redo: bool) {
    workspace.read_handle(item, |handle| {
        let Some(history) = handle.history() else {
            return;
        };
        match redo {
            true => history.redo(),
            false => history.undo(),
        }
    });
}

#[component]
fn Breadcrumbs(workspace: Rc<Workspace>, tab: TabId, info: ReadSignal<Option<Info>>) -> NodeId {
    let parents = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.parents.list.clone())
                .unwrap_or_default()
        })
    }));
    let root = create_memo(clone!(parents -> move || {
        match parents.with(|parents| parents.first().map(|parent| parent.parent)) {
            Some(BlockParent::Orphaned) => "Recently Deleted".to_owned(),
            Some(BlockParent::Root) => "Root".to_owned(),
            Some(BlockParent::Uuid(_)) | None => "Unknown".to_owned(),
        }
    }));
    let indices = create_memo(clone!(parents -> move || {
        parents.with(|parents| (0..parents.len()).collect::<Vec<_>>())
    }));
    let label = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(String::new, |info| info.label.clone())
        })
    }));
    let theme = use_theme();
    let crumb_parents = parents.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CRUMB_SPACING>
            <Caption content={root} />
            <ForEach keys={indices}>
                {move |index: usize| {
                    let parent = create_memo(clone!(crumb_parents -> move || {
                        crumb_parents.with(|parents| parents.get(index).cloned())
                    }));
                    let workspace = Rc::clone(&workspace);
                    view! {
                        <Crumb workspace={workspace} tab={tab} parent={parent} />
                    }
                }}
            </ForEach>
            <IconSized
                glyph={ICON_CHEVRON_RIGHT.to_owned()}
                font_size=FONT_SMALL
                color={theme.text_muted.clone()}
            />
            <Body content={label} />
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn Crumb(workspace: Rc<Workspace>, tab: TabId, parent: Memo<Option<BlockReference>>) -> NodeId {
    let naming = Rc::clone(&workspace);
    let label = create_memo(clone!(parent -> move || {
        let types = naming.types();
        parent.with(|parent| {
            parent.as_ref().map_or_else(
                || "Untitled".to_owned(),
                |parent| BlockLabel::for_reference(types.as_ref(), parent).name,
            )
        })
    }));
    let opened = parent.clone();
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CRUMB_SPACING>
            <IconSized
                glyph={ICON_CHEVRON_RIGHT.to_owned()}
                font_size=FONT_SMALL
                color={theme.text_muted.clone()}
            />
            <Button
                label={label}
                variant=ButtonVariant::Ghost
                on_click={move || {
                    let Some(parent) = opened.get_untracked() else {
                        return;
                    };
                    workspace.navigate(
                        tab,
                        Navigation::Open(TabItem {
                            id: parent.id,
                            block_type: parent.block_type,
                        }),
                    );
                }}
            />
        </List>
    }
}

#[component]
fn AccessMode(workspace: Rc<Workspace>, info: ReadSignal<Option<Info>>) -> NodeId {
    let debugging = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.debugging))
    }));
    let access = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or(BlockAccess::None, |info| info.access)
        })
    }));
    let ceiling = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or(BlockAccess::None, |info| info.ceiling)
        })
    }));
    let label = create_memo(clone!(debugging access -> move || match debugging.get() {
        true => "Debug".to_owned(),
        false => wording(access.get()).to_owned(),
    }));
    let glyph = create_memo(clone!(debugging access -> move || match debugging.get() {
        true => ICON_DATA_OBJECT.to_owned(),
        false => access_glyph(access.get()).to_owned(),
    }));
    let editing_off = create_memo(clone!(ceiling -> move || BlockAccess::Edit > ceiling.get()));
    let viewing_off = create_memo(clone!(ceiling -> move || BlockAccess::View > ceiling.get()));
    let knowing_off =
        create_memo(clone!(ceiling -> move || BlockAccess::KnowExists > ceiling.get()));
    let debug_off = create_memo(clone!(ceiling -> move || !ceiling.get().can_view()));
    let chosen = info.clone();
    view! {
        <MenuButton
            @test_id={"workspace.access"}
            label={label}
            glyph={glyph}
            items={view! {
                <MenuItem label="Editing" disabled={editing_off} />
                <MenuItem label="Viewing" disabled={viewing_off} />
                <MenuItem label="No access" disabled={knowing_off} />
                <MenuItem label="Debug" disabled={debug_off} />
            }}
            on_select={move |path: Vec<usize>| {
                let Some(info) = chosen.get_untracked() else {
                    return;
                };
                let level = match path.as_slice() {
                    [0] => AccessLevel::Edit,
                    [1] => AccessLevel::View,
                    [2] => AccessLevel::KnowExists,
                    [3] => {
                        workspace.debug(info.item.id, true);
                        return;
                    }
                    _ => return,
                };
                workspace.debug(info.item.id, false);
                if chosen_access(level) != info.access {
                    workspace.simulate(info.item.id, level);
                }
            }}
        />
    }
}

fn chosen_access(level: AccessLevel) -> BlockAccess {
    match level {
        AccessLevel::Edit => BlockAccess::Edit,
        AccessLevel::View => BlockAccess::View,
        AccessLevel::KnowExists => BlockAccess::KnowExists,
        AccessLevel::None => BlockAccess::None,
    }
}

fn wording(access: BlockAccess) -> &'static str {
    match access {
        BlockAccess::Edit => "Editing",
        BlockAccess::View => "Viewing",
        BlockAccess::KnowExists | BlockAccess::None => "No access",
    }
}

fn access_glyph(access: BlockAccess) -> &'static str {
    match access {
        BlockAccess::Edit => ICON_EDIT,
        BlockAccess::View => ICON_VISIBILITY,
        BlockAccess::KnowExists | BlockAccess::None => ICON_LOCK,
    }
}
