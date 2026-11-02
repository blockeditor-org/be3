use be_sdk_beui::beui::dock::styled::{
    DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::{Editor, FileChooser, SIDE_PANEL_WIDTH};
use be_type_deterministic_game::GameModuleContent;
use tabletop_host::Game;

use super::{filter, imported};
const STAGE: &str = "stage";
const PANEL: &str = "panel";
const PANEL_WIDTH: f32 = SIDE_PANEL_WIDTH;
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
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="module" size={SplitSize::Second(PANEL_WIDTH)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Module">
                        <Frame @node_ref={&content} padding=PADDING>
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
                                    <Text style=TextStyle::CAPTION align=TextAlign::Center>
                                        "Loading…"
                                    </Text>
                                </Show>
                                <Text align=TextAlign::Center>{source}</Text>
                                <Text style=TextStyle::CAPTION align=TextAlign::Center>
                                    {bytes}
                                </Text>
                            </List>
                        </Frame>
                    </DockTab>
                </DockPane>
                <Show condition={chrome}>
                    <DockPane id="panel" locked=true>
                        <DockTab id=PANEL title="Game module">
                            <ModulePanel editor={editor} />
                        </DockTab>
                    </DockPane>
                </Show>
            </DockSplit>
        </Docking>
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
        <Frame fill=ColorRole::Surface>
            <Scroll>
                <Frame padding=PADDING>
                    <List spacing=SPACING>
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
                </Frame>
            </Scroll>
        </Frame>
    }
}
