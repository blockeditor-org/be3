use std::rc::Rc;

use block_editor_beui::be_block::database::DatabaseContent;
use block_editor_beui::be_block::database_view::{self, DatabaseViewContent};
use block_editor_beui::be_block::{BlockContent, DatabaseSchemaContent};
use block_editor_beui::beui::reactive::{
    Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef, Show, Spacer, clone, component,
    create_effect, create_memo, view,
};
use block_editor_beui::beui::styled::{Button, ButtonVariant, Caption, Heading, Scroll, use_theme};
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockInfo, BlockQuery};
use block_editor_beui::{BlockLink, ChildBlock, ChildMode, ChildTarget, Editor, Sidebar};
use uuid::Uuid;

const PADDING: f32 = 20.0;
const SECTION_SPACING: f32 = 10.0;
const INTRINSIC_WIDTH: f32 = 400.0;
const ROW_HEIGHT: f32 = 24.0;
const CHROME_HEIGHT: f32 = 90.0;

#[component]
pub fn DatabaseEditor(editor: Editor) -> NodeId {
    let database = editor.block_content::<DatabaseContent>();
    let views = watch_views(&editor);
    let loaded = views.loaded.clone();
    let rows = views.rows.clone();
    let reference = database.project(|database| database.root().schema);
    let schema = create_memo(move || reference.get());

    let sized = editor.clone();
    create_effect(clone!(rows -> move || {
        let height = CHROME_HEIGHT + ROW_HEIGHT * rows.with(Vec::len) as f32;
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let empty = create_memo(clone!(rows -> move || rows.with(Vec::is_empty)));
    let waiting = create_memo(clone!(empty loaded -> move || empty.get() && !loaded.get()));
    let none_yet = create_memo(clone!(empty loaded -> move || empty.get() && loaded.get()));
    let read_only = editor.read_only();
    let block_id = editor.block_id();
    let creating = editor.clone();
    let new_view = move || {
        creating.create_child(&DatabaseViewContent::new(&database_view::DatabaseView::of(
            block_id,
        )));
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
                        <Scroll @sizing=ItemSize::Percent(100.0)>
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
fn ViewLinks(editor: Editor, rows: Memo<Vec<BlockInfo>>) -> NodeId {
    let keys = create_memo(clone!(rows -> move || {
        rows.with(|rows| rows.iter().map(|row| row.id).collect::<Vec<Uuid>>())
    }));
    view! {
        <List spacing=4.0>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let target = Some(ChildTarget::new(id, DatabaseViewContent::CONTENT_TYPE));
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
        schema.get().map(|id| ChildTarget::new(id, DatabaseSchemaContent::CONTENT_TYPE))
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
    rows: Memo<Vec<BlockInfo>>,
    loaded: Memo<bool>,
}

fn watch_views(editor: &Editor) -> Views {
    let references = editor
        .blocks()
        .watch(BlockQuery::Backrefs(editor.block_id()));
    let references = Rc::new(references);
    let loaded = create_memo(clone!(references -> move || references.is_loaded()));
    let rows = create_memo(move || {
        references
            .read()
            .into_iter()
            .filter(|reference| reference.block_type == DatabaseViewContent::CONTENT_TYPE)
            .collect()
    });
    Views { rows, loaded }
}
