use beui::Extent;
use block_editor_beui::be_block::Root;
use block_editor_beui::be_block::ViewState;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{BlockParent, BlockQuery, ChildBlock, ChildMode, ChildTarget, Editor};
use database_types::database::{Database, DatabaseContent};
use database_types::database_schema::{DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent};
use database_types::database_view::{DatabaseView, DatabaseViewContent, DatabaseViewKind};
use uuid::Uuid;

pub(crate) const CALENDAR_STATE: &str = "desktop.calendar";
pub(crate) const CALENDAR_WIDTH: f32 = 640.0;
const HEIGHT: f32 = 600.0;
pub(crate) const SCREEN_SHARE: f32 = 0.8;

#[component]
pub(crate) fn DesktopCalendar(
    editor: Editor,
    #[prop(default = SCREEN_SHARE)] share: Prop<f32>,
) -> NodeId {
    let block = calendar_block(editor);
    let target = create_memo(move || {
        block
            .get()
            .map(|id| ChildTarget::new(id, DatabaseView::CONTENT_TYPE))
    });
    let screen = container_size();
    let height = create_memo(move || {
        let room = screen
            .as_ref()
            .map(|screen| screen.get().y * share.get())
            .filter(|room| *room > 0.0);
        Size::Custom(room.map_or(HEIGHT, |room| room.min(HEIGHT)))
    });
    view! {
        <Frame width={Extent::at_most(CALENDAR_WIDTH)} height=height @test_id={"desktop.calendar"}>
            <ChildBlock editor={editor} block={target} mode=ChildMode::Live own_frame=true />
        </Frame>
    }
}

fn calendar_block(editor: Editor) -> Memo<Option<Uuid>> {
    let view = editor.view_content();
    let stored = create_memo(move || {
        view.as_ref().and_then(|view| {
            view.read(|held| {
                held.root()
                    .state(CALENDAR_STATE)
                    .and_then(|state| state.refs.first().copied())
                    .filter(|id| !id.is_nil())
            })
            .ok()
            .flatten()
        })
    });
    let children = editor.watch_blocks(BlockQuery::Children(editor.block_id()));
    let created = create_signal(None::<Uuid>);
    let held = create_memo(move || {
        stored.get().filter(|id| {
            created.get() == Some(*id)
                || children.with(|children| {
                    children.as_ref().map_or(true, |children| {
                        children.iter().any(|child| child.id == *id)
                    })
                })
        })
    });
    let editable = editor.editable();
    create_effect(move || {
        let ready = view.as_ref().is_some_and(|view| view.ready().is_ok());
        if !ready || !editable.get() || held.get().is_some() {
            return;
        }
        untrack(|| {
            let calendar = create_calendar(editor);
            created.set(Some(calendar));
            editor.set_view_state(CALENDAR_STATE, Some(&ViewState::new(&(), vec![calendar])));
        });
    });
    held
}

fn create_calendar(editor: Editor) -> Uuid {
    let blocks = editor.blocks();
    let mut schema = DatabaseSchemaContent::default();
    let mut add = |name: &str, field_type: DatabaseFieldType| {
        let (id, edit) = DatabaseSchema::add_field(name, field_type);
        schema.apply(&edit);
        id
    };
    add("Name", DatabaseFieldType::String);
    let start = add("Starts", DatabaseFieldType::Datetime);
    let end = add("Ends", DatabaseFieldType::Datetime);
    let schema = blocks.create(&schema, BlockParent::Detached);
    let database = blocks.create_named(
        &DatabaseContent::new(&Database::with_schema(schema)),
        BlockParent::Detached,
        "Calendar",
    );
    blocks.set_parent(schema, BlockParent::Block(database));
    let view = editor.create_child(&DatabaseViewContent::new(&DatabaseView {
        kind: DatabaseViewKind::Calendar,
        calendar_start: Some(start),
        calendar_end: Some(end),
        ..DatabaseView::of(database)
    }));
    blocks.set_parent(database, BlockParent::Block(view));
    view
}
