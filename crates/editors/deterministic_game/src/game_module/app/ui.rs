use block_editor_plugin::be_block::GameModuleContent;
use block_editor_plugin::beui::reactive::{
    Direction, Frame, ItemSize, List, NodeRef, Show, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Paragraph, use_theme,
};
use block_editor_plugin::beui::{NodeId, TextAlign};
use block_editor_plugin::{Editor, FileChooser, Sidebar};
use game_host::Game;

use super::{filter, imported};

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;

#[derive(Clone, Default, PartialEq)]
enum Loaded {
    #[default]
    Loading,
    Named(String),
    Failed(String),
}

#[component]
pub fn ModuleView(editor: Editor) -> NodeId {
    let module = editor.block_content::<GameModuleContent>();
    let loaded = module.project(|module| {
        if module.data().is_empty() {
            return Loaded::Loading;
        }
        match Game::load(module.data()) {
            Ok(game) => Loaded::Named(game.name().to_owned()),
            Err(error) => Loaded::Failed(error),
        }
    });
    let source = module.project(|module| module.header().source_name.clone());
    let size = module.project(|module| module.data().len());
    let bytes = create_memo(clone!(size -> move || format!("{} bytes", size.get())));

    let name = create_memo(clone!(loaded -> move || match loaded.get() {
        Loaded::Named(name) => name,
        _ => String::new(),
    }));
    let named = create_memo(clone!(loaded -> move || matches!(loaded.get(), Loaded::Named(_))));
    let reason = create_memo(clone!(loaded -> move || match loaded.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(loaded -> move || matches!(loaded.get(), Loaded::Failed(_))));
    let waiting = create_memo(clone!(loaded -> move || loaded.get() == Loaded::Loading));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let panel = editor.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=SPACING>
                        <Show condition={named}>
                            <Heading
                                content={name}
                                align=TextAlign::Center
                                @test_id={"game-module.name"}
                            />
                        </Show>
                        <Show condition={failed}>
                            <Paragraph
                                content={reason}
                                align=TextAlign::Center
                                color={theme.danger.clone()}
                                @test_id={"game-module.error"}
                            />
                        </Show>
                        <Show condition={waiting}>
                            <Caption content="Loading…" align=TextAlign::Center />
                        </Show>
                        <Body content={source} align=TextAlign::Center />
                        <Caption content={bytes} align=TextAlign::Center />
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <ModulePanel editor={panel} />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn ModulePanel(editor: Editor) -> NodeId {
    let replacing = editor.clone();
    let chooser = FileChooser::new(filter(), imported);
    chooser.on_reply(editor.replies(), editor.host().clone(), move |chooser| {
        if let Some(replacement) = chooser.take() {
            replacing.replace_content(replacing.block_id(), &replacement);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(clone!(busy read_only -> move || busy.get() || read_only.get()));
    let error = chooser.error();
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let opened = editor.host().clone();
    let replace = move || chooser.open(&opened);
    let theme = use_theme();
    view! {
        <List spacing=SPACING>
            <Heading content="Game module" />
            <Button
                label="Replace module..."
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"game-module.replace"}
                on_click={replace}
            />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}
