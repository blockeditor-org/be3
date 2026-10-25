use block_editor_beui::be_block::{Home, HomeContent};
use block_editor_beui::beui::icons::ICON_X;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockQuery, Editor, Loaded};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

pub struct HomeApp;

#[component]
impl block_editor_beui::BeuiApp for HomeApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <HomeView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Landing,
    Pinned,
    Template,
}

#[component]
fn HomeView(editor: Editor) -> NodeId {
    let home = editor.block_content::<HomeContent>();
    let landing = home.project(|home| home.root().landing.map(|landing| landing.0));
    let landing = create_memo(move || landing.get().into_iter().collect::<Vec<_>>());
    let pinned = home.project(|home| home.root().pinned());
    let pinned = create_memo(move || pinned.get());
    let templates = home.project(|home| home.root().templates());
    let templates = create_memo(move || templates.get());
    let no_landing = create_memo(move || landing.with(Vec::is_empty));
    let no_pins = create_memo(move || pinned.with(Vec::is_empty));
    let no_templates = create_memo(move || templates.with(Vec::is_empty));
    view! {
        <SettingsPage>
            <SettingsSection
                title="Landing page"
                description="Opens first when someone opens the workspace. Make one from a block's menu in Files."
            >
                <Show condition={no_landing}>
                    <SettingsRow label="None" compact=true />
                </Show>
                <ForEach keys={landing}>|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Landing />
                </ForEach>
            </SettingsSection>
            <SettingsSection
                title="Pinned"
                description="Shown first in the workspace in Files. Pin one from a block's menu."
            >
                <Show condition={no_pins}>
                    <SettingsRow label="Nothing is pinned" compact=true />
                </Show>
                <ForEach keys={pinned}>|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Pinned />
                </ForEach>
            </SettingsSection>
            <SettingsSection
                title="Templates"
                description="New from template, in the new-block menu, copies one of these with what is inside it."
            >
                <Show condition={no_templates}>
                    <SettingsRow label="No templates" compact=true />
                </Show>
                <ForEach keys={templates}>|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Template />
                </ForEach>
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn Placed(editor: Editor, block: Uuid, place: Place) -> NodeId {
    let found = editor.watch_blocks(BlockQuery::Block(block));
    let name = create_memo(move || {
        let types = editor.block_types();
        found
            .get()
            .ok()
            .and_then(|found| found.into_iter().next())
            .map_or_else(
                || "A block you can't open".to_owned(),
                |info| info.label(types.as_ref()).name,
            )
    });
    let editable = editor.editable();
    let locked = create_memo(move || !editable.get());
    let home = editor.block_content::<HomeContent>();
    let remove = move || {
        home.operate(match place {
            Place::Landing => Home::set_landing(None),
            Place::Pinned => Home::pin(block, false),
            Place::Template => Home::keep_template(block, false),
        });
    };
    view! {
        <SettingsRow label={name}>
            <IconButton
                glyph=ICON_X
                label="Remove"
                disabled={locked}
                @test_id={format!("home.{block}.remove")}
                on_click={remove}
            />
        </SettingsRow>
    }
}
