use std::cell::RefCell;
use std::rc::Rc;

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
pub(crate) fn DetailsSheet(workspace: Rc<Workspace>, tab: TabId) -> NodeId {
    let sheet = workspace.sheet.clone();
    let shown = create_memo(clone!(sheet -> move || match sheet.get() {
        PhoneSheet::Details(shown) if shown == tab => Some(shown),
        PhoneSheet::Details(_) | PhoneSheet::Closed => None,
    }));
    let open = create_memo(clone!(shown -> move || shown.get().is_some()));
    let closing = Rc::clone(&workspace);
    let dismissed = shown.clone();
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[1]}
            on_close={move || {
                if let Some(tab) = dismissed.get_untracked() {
                    closing.dismiss_sheet(PhoneSheet::Details(tab));
                }
            }}
        >
            <List spacing=0.0>
                <Dynamic value={shown}>
                    {move |tab: Option<TabId>| {
                        let workspace = Rc::clone(&workspace);
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
fn DetailsBody(workspace: Rc<Workspace>, tab: TabId) -> NodeId {
    let theme = use_theme();
    let (info, set_info) = create_signal(None::<Info>);
    let reading = Rc::downgrade(&workspace);
    let watched = RefCell::new(Watched::default());
    create_effect(move || {
        let Some(workspace) = reading.upgrade() else {
            return;
        };
        set_info.set(read_info(&workspace, tab, &watched));
    });
    let naming = Rc::clone(&workspace);
    let name = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| naming.label(info.item.id, info.item.block_type).name)
                .unwrap_or_default()
        })
    }));
    let glyphs = Rc::clone(&workspace);
    let glyph = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| glyphs.label(info.item.id, info.item.block_type).icon)
                .unwrap_or_default()
                .to_owned()
        })
    }));
    let kind = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map(|info| info.type_name.clone()).unwrap_or_default())
    }));
    let judging = Rc::clone(&workspace);
    let rights = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref().and_then(|info| {
                let reference = judging.info(info.item.id)?;
                let allowed = permissions(&judging, &reference, info.container);
                Some((
                    allowed.edit,
                    allowed.delete,
                    allowed.is_reference && allowed.unlink.is_ok(),
                    allowed.is_reference,
                ))
            })
        })
    }));
    let rename_off = create_memo(clone!(rights -> move || !rights.get().is_some_and(|r| r.0)));
    let share_off = rename_off.clone();
    let delete_off = create_memo(clone!(rights -> move || !rights.get().is_some_and(|r| r.1)));
    let unlinkable = create_memo(clone!(rights -> move || rights.get().is_some_and(|r| r.2)));
    let delete_label = create_memo(clone!(rights -> move || {
        match rights.get().is_some_and(|r| r.3) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        }
    }));
    let references = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map(|info| info.references.clone()).unwrap_or_default())
    }));
    let backrefs = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map(|info| info.backrefs.clone()).unwrap_or_default())
    }));
    let act = Rc::new(clone!(workspace info -> move |action: Action| {
        let Some((id, container)) = info.with_untracked(|info| {
            info.as_ref().map(|info| (info.item.id, info.container))
        }) else {
            return;
        };
        workspace.set_sheet(PhoneSheet::Closed);
        if let Some(reference) = workspace.info(id) {
            apply(&workspace, &reference, container, action);
        }
    }));
    let creating = Rc::clone(&workspace);
    let new_here = clone!(info -> move || {
        let Some(id) = info.with_untracked(|info| info.as_ref().map(|info| info.item.id)) else {
            return;
        };
        creating.set_sheet(PhoneSheet::Closed);
        creating.new_file_near(id);
    });
    let renaming = Rc::clone(&act);
    let sharing = Rc::clone(&act);
    let unlinking = Rc::clone(&act);
    let deleting = Rc::clone(&act);
    let referencing = Rc::clone(&workspace);
    let backreferencing = Rc::clone(&workspace);
    view! {
        <Frame padding_horizontal=SECTION_PADDING padding_vertical=SECTION_PADDING>
            <List spacing=0.0>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SECTION_PADDING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                        <Icon glyph color={theme.accent.clone()} />
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
                    on_click={move || renaming(Action::Rename)}
                />
                <ActionRow
                    @test_id={"workspace.details.share"}
                    label="Share"
                    glyph={ICON_SHARE.to_owned()}
                    disabled={share_off}
                    on_click={move || sharing(Action::Share)}
                />
                <Show condition={unlinkable}>
                    {move || clone!(unlinking -> view! {
                        <ActionRow
                            @test_id={"workspace.details.unlink"}
                            label="Unlink"
                            glyph={ICON_LINK_OFF.to_owned()}
                            on_click={move || unlinking(Action::Unlink)}
                        />
                    })}
                </Show>
                <ActionRow
                    @test_id={"workspace.details.delete"}
                    label={delete_label}
                    glyph={ICON_DELETE.to_owned()}
                    disabled={delete_off}
                    danger=true
                    on_click={move || deleting(Action::Delete)}
                />
                <RelatedBlocks
                    workspace={referencing}
                    title="References"
                    refs={references}
                    named="workspace.details.reference"
                />
                <RelatedBlocks
                    workspace={backreferencing}
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
    workspace: Rc<Workspace>,
    title: String,
    refs: Memo<Refs>,
    named: String,
) -> NodeId {
    let ids = create_memo(clone!(refs -> move || {
        refs.with(|refs| refs.list.iter().map(|info| info.id).collect::<Vec<Uuid>>())
    }));
    let some = create_memo(clone!(ids -> move || !ids.get().is_empty()));
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
                    let workspace = Rc::clone(&workspace);
                    let reference = create_memo(clone!(refs -> move || {
                        refs.with(|refs| refs.list.iter().find(|info| info.id == id).cloned())
                    }));
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
fn RelatedRow(
    workspace: Rc<Workspace>,
    reference: Memo<Option<BlockInfo>>,
    named: String,
) -> NodeId {
    let naming = Rc::clone(&workspace);
    let label = create_memo(clone!(reference -> move || {
        let types = naming.types();
        reference.with(|reference| reference.as_ref().map(|reference| reference.label(types.as_ref())))
    }));
    let name = create_memo(clone!(label -> move || {
        label.get().map(|label| label.name).unwrap_or_default()
    }));
    let glyph = create_memo(clone!(label -> move || {
        label.get().and_then(|label| label.icon).unwrap_or_default().to_owned()
    }));
    let opening = clone!(reference -> move || {
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
    });
    view! {
        <ActionRow @test_id={named} label={name} glyph on_click={opening} />
    }
}
