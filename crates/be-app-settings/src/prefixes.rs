use be_sdk_beui::be_block::PrefixRegistryContent;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{BlockQuery, Editor, Loaded};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 240.0;

pub struct PrefixesRoot;

#[component]
impl be_sdk_beui::BeuiRoot for PrefixesRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <PrefixesView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[component]
fn PrefixesView(editor: Editor) -> NodeId {
    let registry = editor.block_content::<PrefixRegistryContent>();
    let listed = registry.project(|registry| registry.root().prefixes());
    view! {
        <SettingsPage>
            <SettingsSection
                title="Database prefixes"
                description="The prefix in front of each row id, and the databases here that use it. A row id resolves through this list; a prefix two databases share names neither."
            >
                <ForEach
                    value={listed}
                    key={|(prefix, _): &(String, Vec<Uuid>)| prefix.clone()}
                    fallback={|| view! {
                        <SettingsRow label="No database here has a prefix yet." compact=true />
                    }}
                >|entry: ReadSignal<(String, Vec<Uuid>)>| {
                    let prefix = entry.with_untracked(|(prefix, _)| prefix.clone());
                    let databases = create_memo(move || entry.with(|(_, databases)| databases.clone()));
                    let note = move || match databases.with(Vec::len) {
                        1 => String::new(),
                        count => format!("{count} databases share it, so its row ids name none of them."),
                    };
                    view! {
                        <SettingsRow label={prefix.clone()} description={note}>
                            <List spacing=Space::Xs2>
                                <ForEach value={databases}>|database: Uuid|
                                    <DatabaseName editor={editor} database={database} />
                                </ForEach>
                            </List>
                        </SettingsRow>
                    }
                }</ForEach>
            </SettingsSection>
        </SettingsPage>
    }
}

#[component]
fn DatabaseName(editor: Editor, database: Uuid) -> NodeId {
    let found = editor.watch_blocks(BlockQuery::Block(database));
    let name = create_memo(move || {
        let types = editor.block_types();
        found
            .get()
            .ok()
            .and_then(|found| found.into_iter().next())
            .map_or_else(
                || "A database you can't open".to_owned(),
                |info| info.label(types.as_ref()).name,
            )
    });
    view! {
        <Text>{name}</Text>
    }
}
