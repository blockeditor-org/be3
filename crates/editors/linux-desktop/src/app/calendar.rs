use block_editor_beui::be_block::{Calendar, CalendarContent, Root, ViewState};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Frame, Memo, batch, clone, component, create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_beui::{BlockQuery, ChildBlock, ChildMode, ChildTarget, Editor};
use uuid::Uuid;

pub(crate) const CALENDAR_STATE: &str = "desktop.calendar";
pub(crate) const CALENDAR_WIDTH: f32 = 640.0;
const HEIGHT: f32 = 480.0;

#[component]
pub(crate) fn DesktopCalendar(editor: Editor) -> NodeId {
    let block = calendar_block(&editor);
    let target = create_memo(move || {
        block
            .get()
            .map(|id| ChildTarget::new(id, Calendar::CONTENT_TYPE))
    });
    view! {
        <Frame max_width=CALENDAR_WIDTH max_height=HEIGHT @test_id={"desktop.calendar"}>
            <ChildBlock editor={editor} block={target} mode=ChildMode::Live own_frame=true />
        </Frame>
    }
}

fn calendar_block(editor: &Editor) -> Memo<Option<Uuid>> {
    let view = editor.view_content();
    let stored = create_memo(move || {
        view.as_ref().and_then(|view| {
            view.read(|held| {
                held.root()
                    .state(CALENDAR_STATE)
                    .and_then(|state| state.refs.first().copied())
                    .filter(|id| !id.is_nil())
            })
            .flatten()
        })
    });
    let children = editor.watch_blocks(BlockQuery::Children(editor.block_id()));
    let (created, set_created) = create_signal(None::<Uuid>);
    let held = create_memo(clone!(created -> move || {
        stored.get().filter(|id| {
            created.get() == Some(*id)
                || children.with(|children| {
                    children
                        .as_ref()
                        .is_none_or(|children| children.iter().any(|child| child.id == *id))
                })
        })
    }));
    let loaded = editor.view_content().map(|view| view.loaded());
    let editable = editor.editable();
    let creating = editor.clone();
    create_effect(clone!(held -> move || {
        let ready = loaded.as_ref().is_some_and(|loaded| loaded.get());
        if !ready || !editable.get() || held.get().is_some() {
            return;
        }
        untrack(|| {
            let calendar = creating.create_child(&CalendarContent::default());
            batch(|| {
                set_created.set(Some(calendar));
                creating.set_view_state(
                    CALENDAR_STATE,
                    Some(&ViewState::new(&(), vec![calendar])),
                );
            });
        });
    }));
    held
}
