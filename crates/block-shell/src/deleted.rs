use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::icons::{ICON_DELETE, ICON_RESTORE};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::panel::Info;
use super::workspace::Workspace;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 5.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn DeletedBar(
    workspace: Workspace,
    info: ReadSignal<Option<Info>>,
) -> DynamicSegment<ListChild> {
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
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={"workspace.deleted"}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Xs align=TextAlign::Center>
                        <Icon glyph={ICON_DELETE.to_owned()} />
                    </Text>
                    <Text>"Deleted block"</Text>
                    <Text style=TextStyle::CAPTION>{caption}</Text>
                    <Button
                        @test_id={"workspace.deleted.restore"}
                        label="Restore"
                        glyph={ICON_RESTORE.to_owned()}
                        variant=ButtonVariant::Secondary
                        disabled={locked}
                        on_click={move || {
                            if let Some(deleted) = deleted.get_untracked() {
                                workspace.host().restore_block(deleted.root);
                            }
                        }}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </Frame>
        </Show>
    }
}
