use std::rc::Rc;

use block::BlockAccess;
use block_editor_plugin::AccessLevel;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{ICON_DATA_OBJECT, ICON_EDIT, ICON_LOCK, ICON_VISIBILITY};
use block_editor_plugin::beui::reactive::{ReadSignal, clone, component, create_memo, view};
use block_editor_plugin::beui::styled::MenuButton;
use block_editor_plugin::beui::unstyled::MenuItem;

use super::panel::Info;
use super::workspace::Workspace;

#[component]
pub(crate) fn AccessMode(workspace: Rc<Workspace>, info: ReadSignal<Option<Info>>) -> NodeId {
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
