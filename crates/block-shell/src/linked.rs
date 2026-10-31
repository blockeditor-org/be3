use block_editor_beui::BlockParent;
use block_editor_beui::be_block::VIEW_EDITOR;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_LINK, ICON_UNLINK};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use uuid::Uuid;

use super::menu::ReferenceMenuItem;
use super::menu::unlink_permission;
use super::panel::{Info, Refs};
use super::workspace::Workspace;
use beui::{Align, Direction, ItemSize, Overflow};
use block_editor_beui::beui::TextAlign;

const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Xs;
const SPACING: Space = Space::Md;

#[component]
pub(crate) fn LinkedBar(
    workspace: Workspace,
    info: ReadSignal<Option<Info>>,
) -> DynamicSegment<ListChild> {
    let backrefs = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| places(&info.backrefs))
                .unwrap_or_default()
        })
    });
    let shown = create_memo(move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.access.can_view()
                    && info.backrefs.state.is_ok()
                    && backrefs.with(|places| places.list.len() > 1)
                    && info.parent.is_some()
            })
        })
    });
    let via_reference = create_memo(move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.container
                    .is_some_and(|container| info.parent != Some(BlockParent::Block(container)))
            })
        })
    });
    let original = create_memo(move || !via_reference.get());
    let count = create_memo(move || backrefs.with(|places| places.list.len()));
    let elsewhere = create_memo(move || {
        let others = count.get().saturating_sub(1);
        format!(
            "This block also appears in {others} other place{}. Editing it here changes it everywhere it appears.",
            match others == 1 {
                true => "",
                false => "s",
            }
        )
    });
    let everywhere = create_memo(move || {
        format!(
            "This block appears in {} places. Editing it here changes it everywhere.",
            count.get()
        )
    });
    let permission = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .is_some_and(|info| unlink_permission(&workspace, info.container).is_ok())
        })
    });
    let unlink_off = create_memo(move || !permission.get());
    let nowhere = create_memo(|| None);
    let heading_color = TextColor::Default;
    view! {
        <Show condition={shown}>
            <Frame
                fill=ColorRole::Surface
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Xs color={heading_color} align=TextAlign::Center>
                        <Icon glyph=ICON_LINK />
                    </Text>
                    <Text>"Linked block"</Text>
                    <Show condition={via_reference}>
                        <List
                            @sizing={ItemSize::Intrinsic.shrink(1.0)}
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=SPACING
                        >
                            <Text
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                style=TextStyle::CAPTION
                                overflow=Overflow::Ellipsis
                            >
                                {elsewhere}
                            </Text>
                            <Button
                                @test_id={"workspace.linked.original"}
                                label="Go to original"
                                variant=ButtonVariant::Secondary
                                on_click={move || {
                                    let Some(id) = info
                                        .with(|info| info.as_ref().map(|info| info.item.id))
                                    else {
                                        return;
                                    };
                                    workspace.forget_container(id);
                                }}
                            />
                            <Button
                                @test_id={"workspace.linked.unlink"}
                                label="Unlink"
                                glyph=ICON_UNLINK
                                variant=ButtonVariant::Secondary
                                disabled={unlink_off}
                                on_click={move || {
                                    let Some(info) = info.get_untracked() else {
                                        return;
                                    };
                                    if let Some(container) = info.container {
                                        workspace.host().unlink_block(info.item.id, container);
                                    }
                                }}
                            />
                        </List>
                    </Show>
                    <Show condition={original}>
                        <List
                            @sizing={ItemSize::Intrinsic.shrink(1.0)}
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=SPACING
                        >
                            <Text
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                style=TextStyle::CAPTION
                                overflow=Overflow::Ellipsis
                            >
                                {everywhere}
                            </Text>
                            <ReferenceMenu
                                workspace={workspace}
                                name="Show references"
                                empty="No backrefs"
                                refs={backrefs}
                                containing={nowhere}
                                named="workspace.linked.references"
                            />
                        </List>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </Frame>
        </Show>
    }
}

fn places(backrefs: &Refs) -> Refs {
    Refs {
        state: backrefs.state.clone(),
        list: backrefs
            .list
            .iter()
            .filter(|block| !block.metadata.contains(&VIEW_EDITOR) && !block.deleted)
            .cloned()
            .collect(),
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
                            anchor={button}
                        />
                    }
                }</ForEach>
            }}
        />
    }
}
