use block_editor_beui::be_block::{Calendar, CalendarContent, Root, ViewState};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Frame, Memo, clone, component, create_effect, create_memo, untrack, view,
};
use block_editor_beui::{ChildBlock, ChildMode, ChildTarget, Editor};
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
        <Frame width=CALENDAR_WIDTH height=HEIGHT @test_id={"desktop.calendar"}>
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
    let loaded = editor.view_content().map(|view| view.loaded());
    let editable = editor.editable();
    let creating = editor.clone();
    create_effect(clone!(stored -> move || {
        let ready = loaded.as_ref().is_some_and(|loaded| loaded.get());
        if !ready || !editable.get() || stored.get().is_some() {
            return;
        }
        untrack(|| {
            let calendar = creating.create_child(&CalendarContent::default());
            creating.set_view_state(
                CALENDAR_STATE,
                Some(&ViewState::new(&(), vec![calendar])),
            );
        });
    }));
    stored
}
