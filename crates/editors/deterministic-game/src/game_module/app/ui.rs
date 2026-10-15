use block_editor_beui::be_block::GameModuleContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{Editor, FileChooser, Sidebar};
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
    let bytes = create_memo(move || format!("{} bytes", size.get()));

    let name = create_memo(move || match loaded.get() {
        Loaded::Named(name) => name,
        _ => String::new(),
    });
    let named = create_memo(move || matches!(loaded.get(), Loaded::Named(_)));
    let reason = create_memo(move || match loaded.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    });
    let failed = create_memo(move || matches!(loaded.get(), Loaded::Failed(_)));
    let waiting = create_memo(move || loaded.get() == Loaded::Loading);

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Frame fill=ColorRole::Background>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content} padding=PADDING>
                    <List spacing=SPACING>
                        <Show condition={named}>
                            <Text
                                style=TextStyle::HEADING
                                string={name}
                                align=TextAlign::Center
                                @test_id={"game-module.name"}
                            />
                        </Show>
                        <Show condition={failed}>
                            <Text
                                style=TextStyle::PARAGRAPH
                                string={reason}
                                align=TextAlign::Center
                                color=TextColor::Danger
                                @test_id={"game-module.error"}
                                wrap=true
                            />
                        </Show>
                        <Show condition={waiting}>
                            <Text
                                style=TextStyle::CAPTION
                                string="Loading…"
                                align=TextAlign::Center
                            />
                        </Show>
                        <Text string={source} align=TextAlign::Center />
                        <Text style=TextStyle::CAPTION string={bytes} align=TextAlign::Center />
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <ModulePanel editor={editor} />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn ModulePanel(editor: Editor) -> NodeId {
    let chooser = FileChooser::new(filter(), imported);
    chooser.on_chosen(move |chooser| {
        if let Some(replacement) = chooser.take() {
            editor.replace_content(editor.block_id(), &replacement);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(move || busy.get() || read_only.get());
    let error = chooser.error();
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let replace = move || chooser.open();
    view! {
        <List spacing=SPACING>
            <Text style=TextStyle::HEADING string="Game module" />
            <Button
                label="Replace module..."
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"game-module.replace"}
                on_click={replace}
            />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION string={reason} color=TextColor::Danger />
            </Show>
        </List>
    }
}
