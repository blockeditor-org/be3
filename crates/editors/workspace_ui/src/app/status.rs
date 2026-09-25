use std::rc::Rc;

use block_editor_plugin::Toolbar;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, ReadSignal, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{Caption, MenuButton};
use block_editor_plugin::beui::unstyled::MenuItem;
use uuid::Uuid;

use super::access::AccessMode;
use super::menu::{ReferenceMenuItem, action_for, apply};
use super::panel::{Info, Refs};
use super::workspace::Workspace;

const SPACING: f32 = 10.0;

#[component]
pub(crate) fn StatusBar(workspace: Rc<Workspace>, info: ReadSignal<Option<Info>>) -> NodeId {
    let type_label = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(String::new, |info| format!("Type: {}", info.type_name))
        })
    }));
    let loaded = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.parents.loaded))
    }));
    let loading = create_memo(clone!(loaded -> move || !loaded.get()));
    let backrefs = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(Refs::default, |info| info.backrefs.clone())
        })
    }));
    let references = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map_or_else(Refs::default, |info| info.references.clone())
        })
    }));
    let containing = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map(|info| info.item.id))
    }));
    let listed = Rc::clone(&workspace);
    let access = Rc::clone(&workspace);
    let nothing_contains = create_memo(|| None);
    view! {
        <Toolbar spacing=SPACING>
            <Caption content={type_label} />
            <Show condition={loading}>
                <Caption content="Relationships loading…" />
            </Show>
            <Show condition={loaded}>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <ReferenceMenu
                        workspace={Rc::clone(&workspace)}
                        name="Backrefs"
                        empty="No backrefs"
                        refs={backrefs}
                        containing={nothing_contains}
                        named="workspace.backrefs"
                    />
                    <ReferenceMenu
                        workspace={listed}
                        name="References"
                        empty="No references"
                        refs={references}
                        containing={containing}
                        named="workspace.references"
                    />
                </List>
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <AccessMode workspace={access} info={info} />
        </Toolbar>
    }
}

#[component]
pub(crate) fn ReferenceMenu(
    workspace: Rc<Workspace>,
    name: String,
    empty: String,
    refs: Memo<Refs>,
    containing: Memo<Option<Uuid>>,
    named: String,
) -> NodeId {
    let label = create_memo(clone!(refs -> move || {
        refs.with(|refs| match refs.loaded {
            true => format!("{name}: {}", refs.list.len()),
            false => format!("{name}: …"),
        })
    }));
    let indices = create_memo(clone!(refs -> move || {
        refs.with(|refs| (0..refs.list.len()).collect::<Vec<_>>())
    }));
    let nothing = create_memo(clone!(refs -> move || refs.with(|refs| refs.list.is_empty())));
    let nothing_label = create_memo(clone!(refs -> move || match refs.with(|refs| refs.loaded) {
        true => empty.clone(),
        false => "Loading…".to_owned(),
    }));
    let items_workspace = Rc::clone(&workspace);
    let items_refs = refs.clone();
    let items_containing = containing.clone();
    let chosen_refs = refs.clone();
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
                        let reference = create_memo(clone!(items_refs -> move || {
                            items_refs.with(|refs| refs.list.get(index).cloned())
                        }));
                        view! {
                            <ReferenceMenuItem
                                workspace={Rc::clone(&items_workspace)}
                                reference={reference}
                                containing={items_containing.clone()}
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
