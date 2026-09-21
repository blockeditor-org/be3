use std::rc::Rc;

use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_AUTO_AWESOME, ICON_LINK_OFF, ICON_REFRESH, ICON_SETTINGS,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, ReadSignal, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::theme::FONT_SMALL;
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, IconButton, IconSized, use_theme,
};
use block_editor_plugin::beui::unstyled::TabId;
use uuid::Uuid;

use super::panel::Info;
use super::tab::{Navigation, TabItem};
use super::workspace::Workspace;

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 5.0;
const SPACING: f32 = 8.0;

#[component]
pub(crate) fn ArtifactBar(
    workspace: Rc<Workspace>,
    tab: TabId,
    info: ReadSignal<Option<Info>>,
) -> NodeId {
    let shown = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .is_some_and(|info| info.dynamic_artifact && info.access.can_view())
        })
    }));
    let source = create_memo(clone!(info -> move || {
        info.with(|info| {
            let info = info.as_ref()?;
            let artifact = info.artifact.as_ref()?;
            Some(TabItem {
                id: artifact.source?,
                block_type: artifact.source_type,
            })
        })
    }));
    let described = create_memo(clone!(source -> move || source.get().is_some()));
    let waiting = create_memo(clone!(info described -> move || {
        !described.get()
            && info.with(|info| {
                info.as_ref()
                    .is_none_or(|info| info.artifact.as_ref().is_none_or(|artifact| artifact.error.is_none()))
            })
    }));
    let running = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .is_some_and(|info| info.artifact.as_ref().is_some_and(|artifact| artifact.regenerating))
        })
    }));
    let failure = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| info.artifact.as_ref().and_then(|artifact| artifact.error.clone()))
                .unwrap_or_default()
        })
    }));
    let failed = create_memo(clone!(failure -> move || !failure.get().is_empty()));
    let summary = create_memo(clone!(info -> move || {
        info.with(|info| {
            info.as_ref()
                .and_then(|info| info.artifact.as_ref().map(|artifact| artifact.summary.clone()))
                .unwrap_or_default()
        })
    }));
    let editable = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.can_edit))
    }));
    let settings_off = create_memo(clone!(editable -> move || !editable.get()));
    let regenerate_off = create_memo(clone!(editable running described -> move || {
        !editable.get() || running.get() || !described.get()
    }));
    let unlink_off =
        create_memo(clone!(editable running -> move || !editable.get() || running.get()));
    let source_name = create_memo(clone!(source workspace -> move || {
        source.get().map_or_else(String::new, |source| {
            workspace.label(source.id, source.block_type).name
        })
    }));
    let block = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().map(|info| info.item.id))
    }));
    let opened = Rc::clone(&workspace);
    let open_source = source.clone();
    let settings = block_command(&workspace, block.clone(), Command::Settings);
    let regenerate = block_command(&workspace, block.clone(), Command::Regenerate);
    let unlink = block_command(&workspace, block, Command::Unlink);
    let theme = use_theme();
    let heading_color = theme.text.clone();
    view! {
        <Frame
            visible={shown}
            color={theme.surface.clone()}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List spacing=PADDING_VERTICAL>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <IconSized
                        glyph={ICON_AUTO_AWESOME.to_owned()}
                        font_size=FONT_SMALL
                        color={heading_color}
                    />
                    <Body content="Dynamic artifact" />
                    <Show condition={waiting}>
                        <Caption content="Loading…" />
                    </Show>
                    <Show condition={described}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Caption content="Generated from" />
                            <Button
                                @test_id={"workspace.artifact.source"}
                                label={source_name}
                                variant=ButtonVariant::Ghost
                                on_click={move || {
                                    if let Some(source) = open_source.get_untracked() {
                                        opened.navigate(tab, Navigation::Open(source));
                                    }
                                }}
                            />
                            <Caption content={summary} />
                        </List>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Show condition={running}>
                        <Caption content="Regenerating…" />
                    </Show>
                    <IconButton
                        @test_id={"workspace.artifact.settings"}
                        glyph={ICON_SETTINGS.to_owned()}
                        label="Settings"
                        disabled={settings_off}
                        on_click={settings}
                    />
                    <IconButton
                        @test_id={"workspace.artifact.regenerate"}
                        glyph={ICON_REFRESH.to_owned()}
                        label="Regenerate"
                        disabled={regenerate_off}
                        on_click={regenerate}
                    />
                    <IconButton
                        @test_id={"workspace.artifact.unlink"}
                        glyph={ICON_LINK_OFF.to_owned()}
                        label="Unlink from the source block"
                        disabled={unlink_off}
                        on_click={unlink}
                    />
                </List>
                <Show condition={failed}>
                    <Caption content={failure} color={theme.danger.clone()} />
                </Show>
            </List>
        </Frame>
    }
}

#[derive(Clone, Copy)]
enum Command {
    Settings,
    Regenerate,
    Unlink,
}

fn block_command(
    workspace: &Rc<Workspace>,
    block: block_editor_plugin::beui::reactive::Memo<Option<Uuid>>,
    command: Command,
) -> impl Fn() + 'static {
    let workspace = Rc::clone(workspace);
    move || {
        let Some(id) = block.get_untracked() else {
            return;
        };
        match command {
            Command::Settings => workspace.host().edit_artifact(id),
            Command::Regenerate => workspace.host().regenerate_artifact(id),
            Command::Unlink => workspace.host().unlink_artifact(id),
        }
    }
}
