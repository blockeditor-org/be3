use block_editor_beui::AccessLevel;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_EDIT, ICON_LOCK, ICON_VISIBILITY};
use block_editor_beui::beui::reactive::{ReadSignal, component, create_memo, view};
use block_editor_beui::beui::styled::MenuButton;
use block_editor_beui::beui::unstyled::MenuItem;

use super::panel::Info;
use super::workspace::Workspace;

#[component]
pub(crate) fn AccessMode(workspace: Workspace, info: ReadSignal<Option<Info>>) -> NodeId {
    let access = create_memo(move || {
        info.with(|info| info.as_ref().map_or(AccessLevel::None, |info| info.access))
    });
    let ceiling = create_memo(move || {
        info.with(|info| info.as_ref().map_or(AccessLevel::None, |info| info.ceiling))
    });
    let label = create_memo(move || wording(access.get()).to_owned());
    let glyph = create_memo(move || access_glyph(access.get()).to_owned());
    let editing_off = create_memo(move || AccessLevel::Edit > ceiling.get());
    let viewing_off = create_memo(move || AccessLevel::View > ceiling.get());
    let knowing_off = create_memo(move || AccessLevel::KnowExists > ceiling.get());
    view! {
        <MenuButton
            @test_id={"workspace.access"}
            label={label}
            glyph={glyph}
            items={view! {
                <MenuItem label="Editing" disabled={editing_off} />
                <MenuItem label="Viewing" disabled={viewing_off} />
                <MenuItem label="No access" disabled={knowing_off} />
            }}
            on_select={move |path: Vec<usize>| {
                let Some(info) = info.get_untracked() else {
                    return;
                };
                let level = match path.as_slice() {
                    [0] => AccessLevel::Edit,
                    [1] => AccessLevel::View,
                    [2] => AccessLevel::KnowExists,
                    _ => return,
                };
                if chosen_access(level) != info.access {
                    workspace.simulate(info.item.id, level);
                }
            }}
        />
    }
}

fn chosen_access(level: AccessLevel) -> AccessLevel {
    level
}

fn wording(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Edit => "Editing",
        AccessLevel::View => "Viewing",
        AccessLevel::KnowExists | AccessLevel::None => "No access",
    }
}

fn access_glyph(access: AccessLevel) -> &'static str {
    match access {
        AccessLevel::Edit => ICON_EDIT,
        AccessLevel::View => ICON_VISIBILITY,
        AccessLevel::KnowExists | AccessLevel::None => ICON_LOCK,
    }
}
