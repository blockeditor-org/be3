use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::{Toolbar, node_anchor};
use uuid::Uuid;

use super::access::AccessMode;
use super::menu::{ReferenceMenuItem, action_for, apply};
use super::panel::{Info, Refs};
use super::workspace::Workspace;

const SPACING: f32 = 10.0;

#[component]
pub(crate) fn StatusBar(
    workspace: Workspace,
    info: ReadSignal<Option<Info>>,
) -> DynamicSegment<ListChild> {
    let type_label = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(String::new, |info| format!("Type: {}", info.type_name))
        })
    });
    let relationships = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map_or(Err(Pending::Loading), |info| info.parents.state.clone())
        })
    });
    let loaded = create_memo(move || relationships.with(Result::is_ok));
    let loading = create_memo(move || !loaded.get());
    let pending = create_memo(move || match relationships.get() {
        Err(Pending::Failed(error)) => error,
        _ => "Relationships loading…".to_owned(),
    });
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
            <Text style=TextStyle::CAPTION>{type_label}</Text>
            <Show condition={loading}>
                <Text style=TextStyle::CAPTION>{pending}</Text>
            </Show>
            <Show condition={loaded}>
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
    let refs = create_memo(move || {
        let mut sorted = refs.get();
        sorted.list.sort_by_key(|reference| reference.deleted);
        sorted
    });
    let label = create_memo(move || {
        refs.with(|refs| {
            let deleted = refs
                .list
                .iter()
                .filter(|reference| reference.deleted)
                .count();
            match (refs.state.is_ok(), deleted) {
                (false, _) => format!("{name}: …"),
                (true, 0) => format!("{name}: {}", refs.list.len()),
                (true, deleted) => {
                    format!("{name}: {} (+{deleted} deleted)", refs.list.len() - deleted)
                }
            }
        })
    });
    let indices = create_memo(move || refs.with(|refs| (0..refs.list.len()).collect::<Vec<_>>()));
    let nothing = create_memo(move || refs.with(|refs| refs.list.is_empty()));
    let nothing_label = create_memo(move || match refs.with(|refs| refs.state.clone()) {
        Ok(()) => empty.clone(),
        Err(Pending::Loading) => "Loading…".to_owned(),
        Err(Pending::Failed(error)) => error,
    });
    let button = NodeRef::new();
    view! {
        <MenuButton
            @node_ref=&button
            @test_id={named}
            label={label}
            items={view! {
                <Show condition={nothing}>
                    <MenuItem label={nothing_label} disabled=true />
                </Show>
                <ForEach keys={indices}>|index: usize| {
                    let reference = create_memo(move || {
                        refs.with(|refs| refs.list.get(index).cloned())
                    });
                    view! {
                        <ReferenceMenuItem
                            workspace={workspace}
                            reference={reference}
                            containing={containing}
                        />
                    }
                }</ForEach>
            }}
            on_select={move |path: Vec<usize>| {
                let Some((index, rest)) = path.split_first() else {
                    return;
                };
                let Some(reference) = refs.with(|refs| refs.list.get(*index).cloned())
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
                    node_anchor(&button),
                );
            }}
        />
    }
}
