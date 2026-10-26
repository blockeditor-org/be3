use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockInfo, BlockList, BlockParent, BlockQuery};
use block_editor_beui::{BlockLink, ChildBlock, ChildMode, ChildTarget, Editor, Sidebar};
use database_types::DatabaseSchemaContent;
use database_types::database::DatabaseContent;
use database_types::database_view::{self, DatabaseViewContent};
use uuid::Uuid;

const PADDING: Space = Space::Xl2;
const SECTION_SPACING: Space = Space::Md;
const INTRINSIC_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 24.0;
const CHROME_HEIGHT: f32 = 90.0;

#[component]
pub fn DatabaseEditor(editor: Editor) -> NodeId {
    let database = editor.block_content::<DatabaseContent>();
    let rows = watch_views(editor);
    let reference = database.project(|database| database.root().schema);
    let schema = create_memo(move || reference.get());

    create_effect(move || {
        let height = CHROME_HEIGHT + ROW_HEIGHT * rows.with(Vec::len) as f32;
        editor.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    let none_yet = create_memo(move || rows.with(Vec::is_empty));
    let read_only = editor.read_only();
    let block_id = editor.block_id();
    let new_view = move || {
        let blocks = editor.blocks();
        let parent_of = |block: Uuid| blocks.info(block).and_then(|info| info.parent);
        let container = match parent_of(block_id) {
            Some(BlockParent::Block(above))
                if blocks
                    .info(above)
                    .is_some_and(|info| info.block_type == DatabaseViewContent::CONTENT_TYPE) =>
            {
                parent_of(above)
            }
            other => other,
        };
        let created = blocks.create(
            &DatabaseViewContent::new(&database_view::DatabaseView::of(block_id)),
            BlockParent::Detached,
        );
        match container {
            Some(BlockParent::Block(container)) => editor.host().place_block(
                created,
                DatabaseViewContent::CONTENT_TYPE,
                container,
                false,
            ),
            Some(other) => blocks.set_parent(created, other),
            None => {}
        }
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content} padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Text style=TextStyle::HEADING>"Views"</Text>
                    <Show condition={none_yet}>
                        <Text style=TextStyle::CAPTION>"This database has no views yet."</Text>
                    </Show>
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <ViewLinks editor={editor} rows={rows} />
                    </Scroll>
                    <List direction=Direction::Horizontal spacing=Space::None>
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
    }
}

#[component]
fn ViewLinks(editor: Editor, rows: Memo<Vec<BlockInfo>>) -> NodeId {
    let keys =
        create_memo(move || rows.with(|rows| rows.iter().map(|row| row.id).collect::<Vec<Uuid>>()));
    view! {
        <List spacing=Space::Xs>
            <ForEach keys={keys}>|id: Uuid| {
                let target = Some(ChildTarget::new(id, DatabaseViewContent::CONTENT_TYPE));
                view! {
                    <BlockLink
                        editor={editor}
                        block={target}
                        @test_id={format!("database.view.{id}")}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn SchemaPanel(editor: Editor, schema: Memo<Option<Uuid>>) -> NodeId {
    let target = create_memo(move || {
        schema
            .get()
            .map(|id| ChildTarget::new(id, DatabaseSchemaContent::CONTENT_TYPE))
    });
    let missing = create_memo(move || schema.get().is_none());
    view! {
        <List spacing=Space::None>
            <Show condition={missing}>
                <Text style=TextStyle::CAPTION>"This database has no schema."</Text>
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

fn watch_views(editor: Editor) -> Memo<Vec<BlockInfo>> {
    let references = editor
        .blocks()
        .watch(BlockQuery::Backrefs(editor.block_id()));
    let references = store(references);
    hold(create_memo(move || {
        references.with(BlockList::read).map(|references| {
            references
                .into_iter()
                .filter(|reference| reference.block_type == DatabaseViewContent::CONTENT_TYPE)
                .collect()
        })
    }))
}
