use std::cell::RefCell;

use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::TabId;
use be_sdk_beui::beui::icons::{
    ICON_FILE_PLUS, ICON_PENCIL_LINE, ICON_SHARE_2, ICON_TRASH, ICON_UNLINK,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockInfo, BlockParent};
use uuid::Uuid;

use be_sdk_beui::beui::TextAlign;
use be_sdk_shell::{
    BlockAction, Refs, TabInfo, TabItem, Watched, Workspace, apply, permissions, read_info,
};
use beui::{Align, Direction, ItemSize};

const SHEET_PADDING: Space = Space::Lg;
const SECTION_PADDING: Space = Space::Md;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhoneSheet {
    Closed,
    Details(TabId),
}

#[component]
pub(crate) fn DetailsSheet(workspace: Workspace, sheet: Signal<PhoneSheet>) -> NodeId {
    let shown = create_memo(move || match sheet.get() {
        PhoneSheet::Details(shown) => Some(shown),
        PhoneSheet::Closed => None,
    });
    let open = create_memo(move || shown.get().is_some());
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[1]}
            on_close={move || {
                if shown.get_untracked().is_some() {
                    sheet.set(PhoneSheet::Closed);
                }
            }}
        >
            <List spacing=Space::None>
                <Dynamic value={shown}>|tab: Option<TabId>| {
                    match tab {
                        Some(tab) => view! {
                            <DetailsBody @sizing=ItemSize::Percent(100.0) workspace sheet tab />
                        },
                        None => view! {
                            <Frame @sizing=ItemSize::Percent(100.0) />
                        },
                    }
                }</Dynamic>
            </List>
        </ModalSheet>
    }
}

#[component]
fn DetailsBody(workspace: Workspace, sheet: Signal<PhoneSheet>, tab: TabId) -> NodeId {
    let info = create_signal(None::<TabInfo>);
    let watched = RefCell::new(Watched::default());
    create_effect(move || {
        info.set(read_info(&workspace, tab, &watched));
    });
    let name = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| workspace.label(info.item.id, info.item.block_type).name)
                .unwrap_or_default()
        })
    });
    let glyph = create_memo(move || {
        info.with(|info| {
            info.as_ref().and_then(|info| {
                be_sdk_beui::label_glyph(&workspace.label(info.item.id, info.item.block_type))
            })
        })
    });
    let kind = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.type_name.clone())
                .unwrap_or_default()
        })
    });
    let rights = create_memo(move || {
        info.with(|info| {
            info.as_ref().and_then(|info| {
                let reference = workspace.info(info.item.id)?;
                let allowed = permissions(&workspace, &reference, info.container);
                Some((
                    allowed.edit,
                    allowed.delete,
                    allowed.is_reference && allowed.unlink.is_ok(),
                    allowed.is_reference,
                    allowed.share,
                    allowed.offers_delete,
                ))
            })
        })
    });
    let rename_off = create_memo(move || !rights.get().is_some_and(|r| r.0));
    let delete_off = create_memo(move || !rights.get().is_some_and(|r| r.1));
    let share_off = create_memo(move || !rights.get().is_some_and(|r| r.4));
    let offers_delete = create_memo(move || rights.get().is_none_or(|r| r.5));
    let unlinkable = create_memo(move || rights.get().is_some_and(|r| r.2));
    let delete_label = create_memo(move || match rights.get().is_some_and(|r| r.3) {
        true => "Remove link".to_owned(),
        false => "Delete".to_owned(),
    });
    let references = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.references.clone())
                .unwrap_or_default()
        })
    });
    let backrefs = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.backrefs.clone())
                .unwrap_or_default()
        })
    });
    let act = move |action: BlockAction| {
        let Some((id, container)) =
            info.with_untracked(|info| info.as_ref().map(|info| (info.item.id, info.container)))
        else {
            return;
        };
        sheet.set(PhoneSheet::Closed);
        if let Some(reference) = workspace.info(id) {
            apply(&workspace, &reference, container, action, None);
        }
    };
    let new_here = move || {
        let Some(id) = info.with_untracked(|info| info.as_ref().map(|info| info.item.id)) else {
            return;
        };
        sheet.set(PhoneSheet::Closed);
        workspace.new_file_near(id);
    };
    view! {
        <Frame padding=SECTION_PADDING>
            <List spacing=Space::None>
                <Frame padding={Padding::symmetric(SHEET_PADDING, SECTION_PADDING)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                        <Text color=TextColor::Accent align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                        <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                            <Text style=TextStyle::HEADING>{name}</Text>
                            <Text style=TextStyle::CAPTION>{kind}</Text>
                        </List>
                    </List>
                </Frame>
                <ActionRow
                    @test_id={"workspace.details.new"}
                    label="New file here"
                    glyph=ICON_FILE_PLUS
                    on_click={new_here}
                />
                <ActionRow
                    @test_id={"workspace.details.rename"}
                    label="Rename"
                    glyph=ICON_PENCIL_LINE
                    disabled={rename_off}
                    on_click={move || act(BlockAction::Rename)}
                />
                <ActionRow
                    @test_id={"workspace.details.share"}
                    label="Share"
                    glyph=ICON_SHARE_2
                    disabled={share_off}
                    on_click={move || act(BlockAction::Share)}
                />
                <Show condition={unlinkable}>
                    <ActionRow
                        @test_id={"workspace.details.unlink"}
                        label="Unlink"
                        glyph=ICON_UNLINK
                        on_click={move || act(BlockAction::Unlink)}
                    />
                </Show>
                <Show condition={offers_delete}>
                    <ActionRow
                        @test_id={"workspace.details.delete"}
                        label={delete_label}
                        glyph=ICON_TRASH
                        disabled={delete_off}
                        danger=true
                        on_click={move || act(BlockAction::Delete)}
                    />
                </Show>
                <RelatedBlocks
                    workspace={workspace}
                    sheet
                    title="References"
                    refs={references}
                    named="workspace.details.reference"
                />
                <RelatedBlocks
                    workspace={workspace}
                    sheet
                    title="Referenced by"
                    refs={backrefs}
                    named="workspace.details.backref"
                />
            </List>
        </Frame>
    }
}

#[component]
fn RelatedBlocks(
    workspace: Workspace,
    sheet: Signal<PhoneSheet>,
    title: String,
    refs: Memo<Refs>,
    named: String,
) -> NodeId {
    let ids = create_memo(move || {
        refs.with(|refs| refs.list.iter().map(|info| info.id).collect::<Vec<Uuid>>())
    });
    let some = create_memo(move || !ids.get().is_empty());
    view! {
        <List spacing=Space::None>
            <Show condition={some}>|| {
                let title = title.clone(); view! {
                    <Frame padding={Padding::symmetric(SHEET_PADDING, SECTION_PADDING)}>
                        <Text style=TextStyle::CAPTION>{title}</Text>
                    </Frame>
                }
            }</Show>
            <ForEach keys={ids}>|id: Uuid| {
                let reference = create_memo(move || {
                    refs.with(|refs| refs.list.iter().find(|info| info.id == id).cloned())
                });
                let named = format!("{named}.{id}");
                view! {
                    <RelatedRow workspace sheet reference named />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn RelatedRow(
    workspace: Workspace,
    sheet: Signal<PhoneSheet>,
    reference: Memo<Option<BlockInfo>>,
    named: String,
) -> NodeId {
    let label = create_memo(move || {
        let types = workspace.types();
        reference.with(|reference| {
            reference
                .as_ref()
                .map(|reference| reference.label(types.as_ref()))
        })
    });
    let name = create_memo(move || label.get().map(|label| label.name).unwrap_or_default());
    let glyph = create_memo(move || {
        label
            .get()
            .and_then(|label| be_sdk_beui::label_glyph(&label))
    });
    let detail = create_memo(move || {
        match reference.with(|reference| {
            reference
                .as_ref()
                .is_some_and(|reference| reference.deleted)
        }) {
            true => "In Recently Deleted".to_owned(),
            false => String::new(),
        }
    });
    let opening = move || {
        let Some(reference) = reference.get_untracked() else {
            return;
        };
        sheet.set(PhoneSheet::Closed);
        let via = reference.parent.and_then(BlockParent::block);
        workspace.open(TabItem::new(reference.id, reference.block_type), via);
    };
    view! {
        <ActionRow @test_id={named} label={name} glyph detail on_click={opening} />
    }
}
