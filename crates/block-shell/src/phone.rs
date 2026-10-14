use std::cell::RefCell;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_DELETE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_LINK_OFF, ICON_NOTE_ADD, ICON_SHARE,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, Show, clone, component,
    create_effect, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    ActionRow, Caption, Heading, Icon, ModalSheet, SHEET_STOPS, use_theme,
};
use block_editor_beui::beui::unstyled::TabId;
use block_editor_beui::{BlockInfo, BlockParent};
use uuid::Uuid;

use super::menu::{Action, apply, permissions};
use super::panel::{Info, Refs, Watched, read_info};
use super::tab::TabItem;
use super::workspace::{PhoneSheet, Workspace};

const SHEET_PADDING: f32 = 12.0;
const SECTION_PADDING: f32 = 8.0;

#[component]
pub(crate) fn DetailsSheet(workspace: Workspace, tab: TabId) -> NodeId {
    let sheet = workspace.sheet;
    let shown = create_memo(move || match sheet.get() {
        PhoneSheet::Details(shown) if shown == tab => Some(shown),
        PhoneSheet::Details(_) | PhoneSheet::Closed => None,
    });
    let open = create_memo(move || shown.get().is_some());
    let closing = workspace;
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[1]}
            on_close={move || {
                if let Some(tab) = shown.get_untracked() {
                    closing.dismiss_sheet(PhoneSheet::Details(tab));
                }
            }}
        >
            <List spacing=0.0>
                <Dynamic value={shown}>
                    {move |tab: Option<TabId>| {
                        match tab {
                            Some(tab) => view! {
                                <DetailsBody @sizing=ItemSize::Percent(100.0) workspace tab />
                            },
                            None => view! {
                                <Frame @sizing=ItemSize::Percent(100.0) />
                            },
                        }
                    }}
                </Dynamic>
            </List>
        </ModalSheet>
    }
}

#[component]
fn DetailsBody(workspace: Workspace, tab: TabId) -> NodeId {
    let theme = use_theme();
    let (info, set_info) = create_signal(None::<Info>);
    let watched = RefCell::new(Watched::default());
    create_effect(move || {
        set_info.set(read_info(&workspace, tab, &watched));
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
            info.as_ref()
                .and_then(|info| workspace.label(info.item.id, info.item.block_type).icon)
                .unwrap_or_default()
                .to_owned()
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
                ))
            })
        })
    });
    let rename_off = create_memo(move || !rights.get().is_some_and(|r| r.0));
    let delete_off = create_memo(move || !rights.get().is_some_and(|r| r.1));
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
    let act = move |action: Action| {
        let Some((id, container)) =
            info.with_untracked(|info| info.as_ref().map(|info| (info.item.id, info.container)))
        else {
            return;
        };
        workspace.set_sheet(PhoneSheet::Closed);
        if let Some(reference) = workspace.info(id) {
            apply(&workspace, &reference, container, action);
        }
    };
    let new_here = move || {
        let Some(id) = info.with_untracked(|info| info.as_ref().map(|info| info.item.id)) else {
            return;
        };
        workspace.set_sheet(PhoneSheet::Closed);
        workspace.new_file_near(id);
    };
    view! {
        <Frame padding_horizontal=SECTION_PADDING padding_vertical=SECTION_PADDING>
            <List spacing=0.0>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SECTION_PADDING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                        <Icon glyph color={theme.accent} />
                        <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                            <Heading content={name} />
                            <Caption content={kind} />
                        </List>
                    </List>
                </Frame>
                <ActionRow
                    @test_id={"workspace.details.new"}
                    label="New file here"
                    glyph={ICON_NOTE_ADD.to_owned()}
                    on_click={new_here}
                />
                <ActionRow
                    @test_id={"workspace.details.rename"}
                    label="Rename"
                    glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                    disabled={rename_off}
                    on_click={move || act(Action::Rename)}
                />
                <ActionRow
                    @test_id={"workspace.details.share"}
                    label="Share"
                    glyph={ICON_SHARE.to_owned()}
                    disabled={rename_off}
                    on_click={move || act(Action::Share)}
                />
                <Show condition={unlinkable}>
                    {move || view! {
                        <ActionRow
                            @test_id={"workspace.details.unlink"}
                            label="Unlink"
                            glyph={ICON_LINK_OFF.to_owned()}
                            on_click={move || act(Action::Unlink)}
                        />
                    }}
                </Show>
                <ActionRow
                    @test_id={"workspace.details.delete"}
                    label={delete_label}
                    glyph={ICON_DELETE.to_owned()}
                    disabled={delete_off}
                    danger=true
                    on_click={move || act(Action::Delete)}
                />
                <RelatedBlocks
                    workspace={workspace}
                    title="References"
                    refs={references}
                    named="workspace.details.reference"
                />
                <RelatedBlocks
                    workspace={workspace}
                    title="Referenced by"
                    refs={backrefs}
                    named="workspace.details.backref"
                />
            </List>
        </Frame>
    }
}

#[component]
fn RelatedBlocks(workspace: Workspace, title: String, refs: Memo<Refs>, named: String) -> NodeId {
    let ids = create_memo(move || {
        refs.with(|refs| refs.list.iter().map(|info| info.id).collect::<Vec<Uuid>>())
    });
    let some = create_memo(move || !ids.get().is_empty());
    view! {
        <List spacing=0.0>
            <Show condition={some}>
                {move || clone!(title -> view! {
                    <Frame padding_horizontal=SHEET_PADDING padding_vertical=SECTION_PADDING>
                        <Caption content={title} />
                    </Frame>
                })}
            </Show>
            <ForEach keys={ids}>
                {move |id: Uuid| {
                    let reference = create_memo(move || {
                        refs.with(|refs| refs.list.iter().find(|info| info.id == id).cloned())
                    });
                    let named = format!("{named}.{id}");
                    view! {
                        <RelatedRow workspace reference named />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn RelatedRow(workspace: Workspace, reference: Memo<Option<BlockInfo>>, named: String) -> NodeId {
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
            .and_then(|label| label.icon)
            .unwrap_or_default()
            .to_owned()
    });
    let opening = move || {
        let Some(reference) = reference.get_untracked() else {
            return;
        };
        workspace.set_sheet(PhoneSheet::Closed);
        let via = match reference.parent {
            BlockParent::Block(parent) => Some(parent),
            BlockParent::Root | BlockParent::Detached => None,
        };
        workspace.open(
            TabItem {
                id: reference.id,
                block_type: reference.block_type,
            },
            via,
        );
    };
    view! {
        <ActionRow @test_id={named} label={name} glyph on_click={opening} />
    }
}
