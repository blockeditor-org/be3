use be_sdk_beui::be_block::{Home, HomeContent};
use be_sdk_beui::beui::icons::ICON_X;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{BlockQuery, Editor, Loaded};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 360.0;

pub struct HomeRoot;

#[component]
impl be_sdk_beui::BeuiRoot for HomeRoot {
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
    let templates = home.project(|home| home.root().templates());
    view! {
        <SettingsPage>
            <SettingsSection
                title="Landing page"
                description="Opens first when someone opens the workspace. Make one from a block's menu in Files."
            >
                <ForEach
                    value={landing}
                    fallback={|| view! {
                        <SettingsRow label="None" compact=true />
                    }}
                >|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Landing />
                </ForEach>
            </SettingsSection>
            <SettingsSection
                title="Pinned"
                description="Shown first in the workspace in Files. Pin one from a block's menu."
            >
                <ForEach
                    value={pinned}
                    fallback={|| view! {
                        <SettingsRow label="Nothing is pinned" compact=true />
                    }}
                >|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Pinned />
                </ForEach>
            </SettingsSection>
            <SettingsSection
                title="Templates"
                description="New from template, in the new-block menu, copies one of these with what is inside it."
            >
                <ForEach
                    value={templates}
                    fallback={|| view! {
                        <SettingsRow label="No templates" compact=true />
                    }}
                >|block: Uuid|
                    <Placed editor={editor} block={block} place=Place::Template />
                </ForEach>
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn Placed(editor: Editor, block: Uuid, place: Place) -> NodeId {
    let found = editor.watch_blocks(BlockQuery::Block(block));
    let name = move || {
        let types = editor.block_types();
        found
            .get()
            .ok()
            .and_then(|found| found.into_iter().next())
            .map_or_else(
                || "A block you can't open".to_owned(),
                |info| info.label(types.as_ref()).name,
            )
    };
    let editable = editor.editable();
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
            <Button
                icon_only=true
                glyph=ICON_X
                label="Remove"
                disabled={move || !editable.get()}
                @test_id={format!("home.{block}.remove")}
                on_click={remove}
            />
        </SettingsRow>
    }
}
