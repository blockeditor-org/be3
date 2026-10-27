use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{Editor, FileChooser, Sidebar};
use deterministic_game_types::GameModuleContent;
use game_host::Game;

use super::{filter, imported};
use beui::{Direction, ItemSize};

const PADDING: Space = Space::Lg;
const SPACING: Space = Space::Md;

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
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content} padding=PADDING>
                <List spacing=SPACING>
                    <Show condition={named}>
                        <Text
                            style=TextStyle::HEADING
                            align=TextAlign::Center
                            @test_id={"game-module.name"}
                        >
                            {name}
                        </Text>
                    </Show>
                    <Show condition={failed}>
                        <Text
                            style=TextStyle::PARAGRAPH
                            align=TextAlign::Center
                            color=TextColor::Danger
                            @test_id={"game-module.error"}
                        >
                            {reason}
                        </Text>
                    </Show>
                    <Show condition={waiting}>
                        <Text style=TextStyle::CAPTION align=TextAlign::Center>"Loading…"</Text>
                    </Show>
                    <Text align=TextAlign::Center>{source}</Text>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{bytes}</Text>
                </List>
            </Frame>
            <Sidebar shown={chrome}>
                <ModulePanel editor={editor} />
            </Sidebar>
        </List>
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
            <Text style=TextStyle::HEADING>"Game module"</Text>
            <Button
                label="Replace module..."
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"game-module.replace"}
                on_click={replace}
            />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
            </Show>
        </List>
    }
}
