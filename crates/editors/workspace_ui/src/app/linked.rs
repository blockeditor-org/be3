use std::rc::Rc;

use block_editor_plugin::BlockParent;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{ICON_LINK, ICON_LINK_OFF};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, ReadSignal, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::theme::FONT_SMALL;
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconSized, use_theme,
};

use super::menu::unlink_permission;
use super::panel::Info;
use super::status::ReferenceMenu;
use super::workspace::Workspace;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 5.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn LinkedBar(workspace: Rc<Workspace>, info: ReadSignal<Option<Info>>) -> NodeId {
    let shown = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.access.can_view()
                    && info.backrefs.loaded
                    && info.backrefs.list.len() > 1
                    && info.parent.is_some()
            })
        })
    }));
    let via_reference = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.container
                    .is_some_and(|container| info.parent != Some(BlockParent::Block(container)))
            })
        })
    }));
    let original = create_memo(clone!(via_reference -> move || !via_reference.get()));
    let count = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map_or(0, |info| info.backrefs.list.len()))
    }));
    let elsewhere = create_memo(clone!(count -> move || {
        let others = count.get().saturating_sub(1);
        format!(
            "This block also appears in {others} other place{}. Editing it here changes it everywhere it appears.",
            match others == 1 {
                true => "",
                false => "s",
            }
        )
    }));
    let everywhere = create_memo(clone!(count -> move || {
        format!(
            "This block appears in {} places. Editing it here changes it everywhere.",
            count.get()
        )
    }));
    let permission = create_memo(clone!(info workspace -> move || {
        info.with(|info| {
            info.as_ref()
                .is_some_and(|info| unlink_permission(&workspace, info.container).is_ok())
        })
    }));
    let unlink_off = create_memo(clone!(permission -> move || !permission.get()));
    let backrefs = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.backrefs.clone())
                .unwrap_or_default()
        })
    }));
    let nowhere = create_memo(|| None);
    let original_block = Rc::clone(&workspace);
    let original_info = info.clone();
    let unlinking = Rc::clone(&workspace);
    let unlinking_info = info.clone();
    let theme = use_theme();
    let heading_color = theme.text.clone();
    view! {
        <Frame
            visible={shown}
            color={theme.surface.clone()}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <IconSized
                    glyph={ICON_LINK.to_owned()}
                    font_size=FONT_SMALL
                    color={heading_color}
                />
                <Body content="Linked block" />
                <Show condition={via_reference}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Caption content={elsewhere} />
                        <Button
                            @test_id={"workspace.linked.original"}
                            label="Go to original"
                            variant=ButtonVariant::Secondary
                            on_click={move || {
                                let Some(id) = original_info
                                    .with(|info| info.as_ref().map(|info| info.item.id))
                                else {
                                    return;
                                };
                                original_block.forget_container(id);
                            }}
                        />
                        <Button
                            @test_id={"workspace.linked.unlink"}
                            label="Unlink"
                            glyph={ICON_LINK_OFF.to_owned()}
                            variant=ButtonVariant::Secondary
                            disabled={unlink_off}
                            on_click={move || {
                                let Some(info) = unlinking_info.get_untracked() else {
                                    return;
                                };
                                if let Some(container) = info.container {
                                    unlinking.host().unlink_block(info.item.id, container);
                                }
                            }}
                        />
                    </List>
                </Show>
                <Show condition={original}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Caption content={everywhere} />
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
    }
}
