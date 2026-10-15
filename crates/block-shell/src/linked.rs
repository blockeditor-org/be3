use block_editor_beui::BlockParent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_LINK, ICON_LINK_OFF};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::menu::unlink_permission;
use super::panel::Info;
use super::status::ReferenceMenu;
use super::workspace::Workspace;
use block_editor_beui::beui::TextAlign;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 5.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn LinkedBar(workspace: Workspace, info: ReadSignal<Option<Info>>) -> NodeId {
    let shown = create_memo(move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.access.can_view()
                    && info.backrefs.loaded
                    && info.backrefs.list.len() > 1
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
    let count = create_memo(move || {
        info.with(|info| info.as_ref().map_or(0, |info| info.backrefs.list.len()))
    });
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
    let backrefs = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| info.backrefs.clone())
                .unwrap_or_default()
        })
    });
    let nowhere = create_memo(|| None);
    let original_info = info;
    let unlinking_info = info;
    let heading_color = TextColor::Default;
    view! {
        <Frame
            visible={shown}
            fill=ColorRole::Surface
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text size=TextSize::Xs color={heading_color} align=TextAlign::Center>
                    <Icon glyph={ICON_LINK.to_owned()} />
                </Text>
                <Text string="Linked block" />
                <Show condition={via_reference}>
                    {move || view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text style=TextStyle::CAPTION string={elsewhere} />
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
                                    workspace.forget_container(id);
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
                                        workspace.host().unlink_block(info.item.id, container);
                                    }
                                }}
                            />
                        </List>
                    }}
                </Show>
                <Show condition={original}>
                    {move || view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text style=TextStyle::CAPTION string={everywhere} />
                            <ReferenceMenu
                                workspace={workspace}
                                name="Show references"
                                empty="No backrefs"
                                refs={backrefs}
                                containing={nowhere}
                                named="workspace.linked.references"
                            />
                        </List>
                    }}
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}
