use be_sdk_beui::beui::dock::{DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize};
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
    let payload = module.payload();
    let loaded = create_memo(move || match payload.get() {
        Err(Pending::Loading) => Loaded::Loading,
        Err(Pending::Failed(error)) => Loaded::Failed(error),
        Ok(data) if data.is_empty() => Loaded::Loading,
        Ok(data) => match Game::load(&data) {
            Ok(game) => Loaded::Named(game.name().to_owned()),
            Err(error) => Loaded::Failed(error),
        },
    });
    let source = module.project(|module| module.header().source_name.clone());
    let size = create_memo(move || payload.get().map_or(0, |data| data.len()));
    let bytes = create_memo(move || format!("{} bytes", size.get()));

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
                                <Dynamic value={loaded}>|loaded: Loaded| {
                                    match loaded {
                                    Loaded::Named(name) => view! {
                                        <Text
                                            style=TextStyle::HEADING
                                            align=TextAlign::Center
                                            @test_id={"game-module.name"}
                                        >
                                            {name}
                                        </Text>
                                    },
                                    Loaded::Failed(reason) => view! {
                                        <Text
                                            style=TextStyle::PARAGRAPH
                                            align=TextAlign::Center
                                            color=TextColor::Danger
                                            @test_id={"game-module.error"}
                                        >
                                            {reason}
                                        </Text>
                                    },
                                    Loaded::Loading => view! {
                                        <Text style=TextStyle::CAPTION align=TextAlign::Center>
                                            "Loading…"
                                        </Text>
                                    },
                                    }
                                }</Dynamic>
                                <Text align=TextAlign::Center>{source}</Text>
                                <Text style=TextStyle::CAPTION align=TextAlign::Center>
                                    {bytes}
                                </Text>
                            </List>
                        </Frame>
                    </DockTab>
                </DockPane>
                <Show value={chrome}>
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
    let error = chooser.error();
    let replace = move || chooser.open();
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll>
                <Frame padding=PADDING>
                    <List spacing=SPACING>
                        <Button
                            label="Replace module..."
                            variant=ButtonVariant::Secondary
                            disabled={move || busy.get() || read_only.get()}
                            @test_id={"game-module.replace"}
                            on_click={replace}
                        />
                        <Show value={error}>|reason: ReadSignal<String>|
                            <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
                        </Show>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}
