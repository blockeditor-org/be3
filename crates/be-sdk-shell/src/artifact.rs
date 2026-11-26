use be_sdk_beui::beui::icons::{ICON_REFRESH_CW, ICON_SETTINGS, ICON_SPARKLES, ICON_UNLINK};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::panel::TabInfo;
use super::tab::TabItem;
use super::workspace::Workspace;
use be_sdk_beui::beui::TextAlign;
use beui::{Align, Direction, Overflow};

const PADDING_HORIZONTAL: Space = Space::Md;
const PADDING_VERTICAL: Space = Space::Xs;
const SPACING: Space = Space::Md;

#[component]
pub(crate) fn ArtifactBar(
    workspace: Workspace,
    info: ReadSignal<Option<TabInfo>>,
) -> DynamicSegment<ListItem> {
    let shown = move || {
        info.with(|info| {
            info.as_ref()
                .is_some_and(|info| info.dynamic_artifact && info.access.can_view())
        })
    };
    let source = create_memo(move || {
        info.with(|info| {
            let info = info.as_ref()?;
            let artifact = info.artifact.as_ref()?;
            Some(TabItem::new(artifact.source, artifact.source_type))
        })
    });
    let described = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| info.artifact.as_ref())
                .is_some_and(|artifact| artifact.summary.is_some())
        })
    });
    let waiting = create_memo(move || {
        !described.get()
            && info.with(|info| {
                info.as_ref().is_none_or(|info| {
                    info.artifact
                        .as_ref()
                        .is_none_or(|artifact| artifact.error.is_none())
                })
            })
    });
    let running = create_memo(move || {
        info.with(|info| {
            info.as_ref().is_some_and(|info| {
                info.artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.regenerating)
            })
        })
    });
    let failure = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| {
                    info.artifact
                        .as_ref()
                        .and_then(|artifact| artifact.error.clone())
                })
                .unwrap_or_default()
        })
    });
    let summary = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| {
                    info.artifact
                        .as_ref()
                        .and_then(|artifact| artifact.summary.clone())
                })
                .unwrap_or_default()
        })
    });
    let editable =
        create_memo(move || info.with(|info| info.as_ref().is_some_and(|info| info.can_edit)));
    let source_name = move || {
        source.get().map_or_else(String::new, |source| {
            workspace.label(source.id, source.block_type).name
        })
    };
    let block = create_memo(move || info.with(|info| info.as_ref().map(|info| info.item.id)));
    let settings = block_command(&workspace, block, Command::Settings);
    let regenerate = block_command(&workspace, block, Command::Regenerate);
    let unlink = block_command(&workspace, block, Command::Unlink);
    let heading_color = TextColor::Default;
    view! {
        <Show value={shown}>
            <Frame
                fill=ColorRole::Surface
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
            >
                <List spacing=PADDING_VERTICAL>
                    <List
                        direction=Direction::Horizontal
                        wrap=true
                        align=Align::Center
                        spacing=SPACING
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text size=TextSize::Xs color={heading_color} align=TextAlign::Center>
                                <Icon glyph=ICON_SPARKLES />
                            </Text>
                            <Text>"Dynamic artifact"</Text>
                            <Show value={waiting}>
                                <Text style=TextStyle::CAPTION>"Loading…"</Text>
                            </Show>
                        </List>
                        <Spacer />
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Show value={running}>
                                <Text style=TextStyle::CAPTION>"Regenerating…"</Text>
                            </Show>
                            <Button
                                icon_only=true
                                @test_id={"workspace.artifact.settings"}
                                glyph=ICON_SETTINGS
                                label="Settings"
                                disabled={move || !editable.get()}
                                on_click={settings}
                            />
                            <Button
                                icon_only=true
                                @test_id={"workspace.artifact.regenerate"}
                                glyph=ICON_REFRESH_CW
                                label="Regenerate"
                                disabled={move || !editable.get() || running.get() || !described.get()}
                                on_click={regenerate}
                            />
                            <Button
                                icon_only=true
                                @test_id={"workspace.artifact.unlink"}
                                glyph=ICON_UNLINK
                                label="Unlink from the source block"
                                disabled={move || !editable.get() || running.get()}
                                on_click={unlink}
                            />
                        </List>
                    </List>
                    <Show value={described}>
                        <List
                            direction=Direction::Horizontal
                            wrap=true
                            align=Align::Center
                            spacing=SPACING
                        >
                            <Text style=TextStyle::CAPTION>"Generated from"</Text>
                            <Button
                                @test_id={"workspace.artifact.source"}
                                label={source_name}
                                variant=ButtonVariant::Ghost
                                on_click={move || {
                                    if let Some(source) = source.get_untracked() {
                                        workspace.open(source, None);
                                    }
                                }}
                            />
                            <ListItem shrink=1.0>
                                <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>
                                    {summary}
                                </Text>
                            </ListItem>
                        </List>
                    </Show>
                    <Show value={move || failure.with(|failure| !failure.is_empty())}>
                        <Text style=TextStyle::CAPTION color=TextColor::Danger>{failure}</Text>
                    </Show>
                </List>
            </Frame>
        </Show>
    }
}

#[derive(Clone, Copy)]
enum Command {
    Settings,
    Regenerate,
    Unlink,
}

fn block_command(
    workspace: &Workspace,
    block: be_sdk_beui::beui::reactive::Memo<Option<Uuid>>,
    command: Command,
) -> impl Fn() + Copy + 'static {
    let workspace = *workspace;
    move || {
        let Some(id) = block.get_untracked() else {
            return;
        };
        match command {
            Command::Settings => workspace.open_dialog(OpenDialog::ArtifactSettings(id)),
            Command::Regenerate => workspace.host().regenerate_artifact(id),
            Command::Unlink => workspace.open_dialog(OpenDialog::Unlink(id)),
        }
    }
}
