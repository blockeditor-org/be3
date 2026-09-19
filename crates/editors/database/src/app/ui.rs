use block::{Block, BlockReference, BlockReferenceList};
use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::DatabaseSchema;
use block_client::blocks::database_view::DatabaseView;
use block_editor_plugin::beui::reactive::{
    Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef, Scroll, Show, Spacer, clone,
    component, create_effect, create_memo, create_signal, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockLink, ChildBlock, ChildMode, ChildTarget, Editor, Sidebar};
use uuid::Uuid;

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 10.0;
const INTRINSIC_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 24.0;
const CHROME_HEIGHT: f32 = 90.0;

#[component]
pub fn DatabaseEditor(editor: Editor) -> NodeId {
    let database = editor.block::<Database>();
    let views = watch_views(&editor);
    let loaded = views.loaded.clone();
    let rows = views.rows.clone();
    let reference = database.project(|database| Some(database.schema_id()));
    let own_id = editor.block_id();
    let schema = editor.resolve(create_memo(move || Some(own_id)), reference);

    let sized = editor.clone();
    create_effect(clone!(rows -> move || {
        let height = CHROME_HEIGHT + ROW_HEIGHT * rows.with(Vec::len) as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let empty = create_memo(clone!(rows -> move || rows.with(Vec::is_empty)));
    let waiting = create_memo(clone!(empty loaded -> move || empty.get() && !loaded.get()));
    let none_yet = create_memo(clone!(empty loaded -> move || empty.get() && loaded.get()));
    let read_only = editor.read_only();
    let client = editor.client().clone();
    let block_id = editor.block_id();
    let new_view = move || {
        client.create_block(DatabaseView::new(BlockRef::Direct(block_id)));
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let links = editor.clone();
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
                    <List spacing=SECTION_SPACING>
                        <Heading content="Views" />
                        <Show condition={waiting}>
                            <Caption content="Loading…" />
                        </Show>
                        <Show condition={none_yet}>
                            <Caption content="This database has no views yet." />
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0) focus_color={theme.accent.clone()}>
                            <ViewLinks editor={links} rows={rows} />
                        </Scroll>
                        <List direction=Direction::Horizontal spacing=0.0>
                            <Button
                                label="New view"
                                variant=ButtonVariant::Primary
                                disabled={read_only}
                                @test_id={"database.new-view"}
                                on_click={new_view}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        </List>
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <SchemaPanel @sizing=ItemSize::Percent(100.0) editor={editor} schema={schema} />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn ViewLinks(editor: Editor, rows: Memo<Vec<BlockReference>>) -> NodeId {
    let keys = create_memo(clone!(rows -> move || {
        rows.with(|rows| rows.iter().map(|row| row.id).collect::<Vec<Uuid>>())
    }));
    view! {
        <List spacing=4.0>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let target = Some(ChildTarget::new(id, DatabaseView::TYPE_ID));
                    view! {
                        <BlockLink
                            editor={editor.clone()}
                            block={target}
                            @test_id={format!("database.view.{id}")}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn SchemaPanel(editor: Editor, schema: Memo<Option<Uuid>>) -> NodeId {
    let target = create_memo(clone!(schema -> move || {
        schema.get().map(|id| ChildTarget::new(id, DatabaseSchema::TYPE_ID))
    }));
    let missing = create_memo(clone!(schema -> move || schema.get().is_none()));
    view! {
        <List spacing=0.0>
            <Show condition={missing}>
                <Caption content="Loading the schema…" />
            </Show>
            <ChildBlock
                @sizing=ItemSize::Percent(100.0)
                editor={editor}
                block={target}
                mode=ChildMode::Live
            />
        </List>
    }
}

struct Views {
    rows: Memo<Vec<BlockReference>>,
    loaded: Memo<bool>,
}

fn watch_views(editor: &Editor) -> Views {
    let references = editor
        .client()
        .watch_references(BlockReferenceList::Backrefs(editor.block_id()));
    let (rows, set_rows) = create_signal(Vec::<BlockReference>::new());
    let (loaded, set_loaded) = create_signal(false);
    editor.each_frame(move || {
        set_loaded.set(references.is_loaded());
        set_rows.set(
            references
                .read()
                .into_iter()
                .filter(|reference| reference.block_type == DatabaseView::TYPE_ID)
                .collect(),
        );
    });
    Views {
        rows: create_memo(move || rows.get()),
        loaded: create_memo(move || loaded.get()),
    }
}
