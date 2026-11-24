use be_sdk_beui::beui::dock::{DockPane, DockSplit, DockTab, Docking};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::{ContentFileReplace, Editor};
use be_type_deterministic_game::GameModuleContent;
use tabletop_host::Game;

use super::{filter, imported};
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

    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Docking>
            <DockSplit>
                <DockPane stage=true>
                    <DockTab>
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
                <DockPane chrome=true>
                    <DockTab title="Game module">
                        <ContentFileReplace
                            editor
                            label="Replace module…"
                            id_prefix="game-module"
                            filter={filter()}
                            import={imported}
                        />
                    </DockTab>
                </DockPane>
            </DockSplit>
        </Docking>
    }
}
