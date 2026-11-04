use be_sdk_beui::beui::TextAlign;
use be_sdk_beui::beui::icons::{ICON_ROTATE_CCW_CLOCK, ICON_TRASH};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;

use super::panel::TabInfo;
use super::workspace::Workspace;
use beui::{Align, Direction, Overflow};

const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Xs;
const SPACING: Space = Space::Md;

#[component]
pub(crate) fn DeletedBar(
    workspace: Workspace,
    info: ReadSignal<Option<TabInfo>>,
) -> DynamicSegment<ListItem> {
    let deleted = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .filter(|info| info.access.can_view())
                .and_then(|info| info.deleted.clone())
        })
    });
    let shown = create_memo(move || deleted.with(Option::is_some));
    let caption = create_memo(move || {
        deleted.with(
            |deleted| match deleted.as_ref().and_then(|deleted| deleted.inside.as_ref()) {
                Some(inside) => {
                    format!("This block is inside “{inside}”, which is in Recently Deleted.")
                }
                None => "This block is in Recently Deleted.".to_owned(),
            },
        )
    });
    let locked = create_memo(move || {
        deleted.with(|deleted| {
            deleted
                .as_ref()
                .is_none_or(|deleted| !workspace.can_edit(deleted.root))
        })
    });
    view! {
        <Show condition={shown}>
            <Frame
                fill=ColorRole::Surface
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={"workspace.deleted"}
            >
                <List direction=Direction::Horizontal wrap=true align=Align::Center spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text size=TextSize::Xs align=TextAlign::Center>
                            <Icon glyph=ICON_TRASH />
                        </Text>
                        <Text>"Deleted block"</Text>
                        <ListItem shrink=1.0>
                            <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>
                                {caption}
                            </Text>
                        </ListItem>
                    </List>
                    <Button
                        @test_id={"workspace.deleted.restore"}
                        label="Restore"
                        glyph=ICON_ROTATE_CCW_CLOCK
                        variant=ButtonVariant::Secondary
                        disabled={locked}
                        on_click={move || {
                            if let Some(deleted) = deleted.get_untracked() {
                                workspace.host().restore_block(deleted.root);
                            }
                        }}
                    />
                </List>
            </Frame>
        </Show>
    }
}
