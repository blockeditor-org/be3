use super::*;
use be_block::ObjectId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_plugin::HostContent;
use presentation_types::PresentationContent;

thread_local! {
    static SEEN: RefCell<Vec<Memo<Option<usize>>>> = const { RefCell::new(Vec::new()) };
}

struct CountingApp;

fn count(editor: crate::Editor) -> beui::NodeId {
    let content = editor.block_content::<PresentationContent>();
    let slides = content.project(|deck| deck.root().slides.len());
    let seen = beui::reactive::create_memo(move || content.ready().ok().map(|()| slides.get()));
    SEEN.with(|held| held.borrow_mut().push(seen));
    Frame().build()
}

#[component]
impl crate::BeuiApp for CountingApp {
    #[component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        let node = { count(editor) };
        view! {
            <beui::styled::Root>{node}</beui::styled::Root>
        }
    }

    #[component]
    fn Preview(editor: crate::Editor) -> beui::NodeId {
        count(editor)
    }
}

fn screen(region: EditorRegion, screen: u32) -> (ScreenPlacement, ScreenRequest) {
    let placement = ScreenPlacement {
        screen: ScreenId(screen.into()),
        instance: EditorInstanceId(0),
        region,
        surface: screen + 1,
        width: 400,
        height: 300,
        scale_factor_millis: 1000,
    };
    let request = ScreenRequest {
        screen: ScreenId(screen.into()),
        instance: EditorInstanceId(0),
        region,
        metrics: ViewportMetrics {
            logical: block_plugin_api::Size {
                width: 400.0,
                height: 300.0,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: 400,
            pixel_height: 300,
            scale_factor: 1.0,
        },
        frame: (region == EditorRegion::Frame).then(FrameSpec::default),
        windows: false, placement: None,
    };
    (placement, request)
}

#[test]
fn the_frame_and_the_preview_of_one_block_both_load_its_content() {
    let mut session = session::<CountingApp>(Uuid::new_v4());
    let (frame, frame_request) = screen(EditorRegion::Frame, 0);
    let (preview, preview_request) = screen(EditorRegion::Preview, 1);
    session.place(&[frame, preview], &[frame_request, preview_request]);
    let mut deck = PresentationContent::default();
    let edit = deck.root().insert(ObjectId::new(), 0, Uuid::new_v4());
    deck.apply(&edit);
    session.host().set_block_content(HostContent::of(&deck));

    session.frame(&[(EditorRegion::Frame, 0), (EditorRegion::Preview, 0)]);

    let seen: Vec<_> = SEEN.with(|held| {
        held.borrow()
            .iter()
            .map(|seen| seen.get_untracked())
            .collect()
    });
    assert_eq!(seen, [Some(1), Some(1)]);
}
