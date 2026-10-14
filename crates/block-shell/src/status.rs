use block_editor_beui::Toolbar;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, ReadSignal, Show, Spacer, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::MenuButton;
use block_editor_beui::beui::unstyled::MenuItem;
use uuid::Uuid;

use super::access::AccessMode;
use super::menu::{ReferenceMenuItem, action_for, apply};
use super::panel::{Info, Refs};
use super::workspace::Workspace;
use block_editor_beui::beui::styled::{Text, TextStyle};

const SPACING: f32 = 10.0;

#[component]
pub(crate) fn StatusBar(workspace: Workspace, info: ReadSignal<Option<Info>>) -> NodeId {
    let type_label = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(String::new, |info| format!("Type: {}", info.type_name))
        })
    });
    let loaded = create_memo(move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.parents.loaded))
    });
    let loading = create_memo(move || !loaded.get());
    let backrefs = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(Refs::default, |info| info.backrefs.clone())
        })
    });
    let references = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(Refs::default, |info| info.references.clone())
        })
    });
    let containing = create_memo(move || info.with(|info| info.as_ref().map(|info| info.item.id)));
    let nothing_contains = create_memo(|| None);
    view! {
        <Toolbar spacing=SPACING>
            <Text style=TextStyle::CAPTION string={type_label} />
            <Show condition={loading}>
                <Text style=TextStyle::CAPTION string="Relationships loading…" />
            </Show>
            <Show condition={loaded}>
                {move || view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <ReferenceMenu
                            workspace={workspace}
                            name="Backrefs"
                            empty="No backrefs"
                            refs={backrefs}
                            containing={nothing_contains}
                            named="workspace.backrefs"
                        />
                        <ReferenceMenu
                            workspace={workspace}
                            name="References"
                            empty="No references"
                            refs={references}
                            containing={containing}
                            named="workspace.references"
                        />
                    </List>
                }}
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <AccessMode workspace={workspace} info={info} />
        </Toolbar>
    }
}

#[component]
pub(crate) fn ReferenceMenu(
    workspace: Workspace,
    name: String,
    empty: String,
    refs: Memo<Refs>,
    containing: Memo<Option<Uuid>>,
    named: String,
) -> NodeId {
    let label = create_memo(move || {
        refs.with(|refs| match refs.loaded {
            true => format!("{name}: {}", refs.list.len()),
            false => format!("{name}: …"),
        })
    });
    let indices = create_memo(move || refs.with(|refs| (0..refs.list.len()).collect::<Vec<_>>()));
    let nothing = create_memo(move || refs.with(|refs| refs.list.is_empty()));
    let nothing_label = create_memo(move || match refs.with(|refs| refs.loaded) {
        true => empty.clone(),
        false => "Loading…".to_owned(),
    });
    let items_refs = refs;
    let chosen_refs = refs;
    view! {
        <MenuButton
            @test_id={named}
            label={label}
            items={view! {
                <Show condition={nothing}>
                    <MenuItem label={nothing_label} disabled=true />
                </Show>
                <ForEach keys={indices}>
                    {move |index: usize| {
                        let reference = create_memo(move || {
                            items_refs.with(|refs| refs.list.get(index).cloned())
                        });
                        view! {
                            <ReferenceMenuItem
                                workspace={workspace}
                                reference={reference}
                                containing={containing}
                            />
                        }
                    }}
                </ForEach>
            }}
            on_select={move |path: Vec<usize>| {
                let Some((index, rest)) = path.split_first() else {
                    return;
                };
                let Some(reference) = chosen_refs.with(|refs| refs.list.get(*index).cloned())
                else {
                    return;
                };
                let Some(action) = action_for(rest) else {
                    return;
                };
                apply(
                    &workspace,
                    &reference,
                    containing.get_untracked(),
                    action,
                );
            }}
        />
    }
}
