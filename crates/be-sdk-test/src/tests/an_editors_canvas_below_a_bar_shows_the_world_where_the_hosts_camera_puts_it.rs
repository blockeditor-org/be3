use beui::styled::Root;
use beui::{Align, CanvasView, Rect, Vec2, pos2};

use super::*;

const BAR: f32 = 40.0;

struct BarredRoot;

#[component]
impl BeuiRoot for BarredRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Barred editor={editor} />
            </Root>
        }
    }
}

/// An editor whose content holds a bar above its canvas, so the canvas is not at the content's
/// corner: it names both, and its canvas is handed the camera in its own points.
#[component]
fn Barred(editor: Editor) -> NodeId {
    let (content, canvas) = (NodeRef::new(), NodeRef::new());
    editor.content(&content);
    editor.name_canvas(&canvas);
    let view: ReadSignal<Option<CanvasView>> = editor.canvas();
    view! {
        <List @node_ref=&content spacing=Space::None align=Align::Stretch>
            <Frame height={Size::Custom(BAR)} />
            <ListItem grow=1.0>
                <Canvas @node_ref=&canvas view={view}>
                    <CanvasItem x=10.0 y=5.0 width=20.0 height=10.0>
                        <Frame @test_id={"barred.item"} />
                    </CanvasItem>
                </Canvas>
            </ListItem>
        </List>
    }
}

#[test]
fn an_editors_canvas_below_a_bar_shows_the_world_where_the_hosts_camera_puts_it() {
    let mut test: BeuiTest<BarredRoot> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.set_view(
        Rect::from_min_size(pos2(100.0, 80.0), Vec2::new(400.0, 300.0)),
        2.0,
    );
    test.run();

    assert_eq!(
        test.rect_of("barred.item"),
        Rect::from_min_size(pos2(120.0, 90.0), Vec2::new(40.0, 20.0)),
        "the item is where the host's camera shows its world point, not a bar's height lower"
    );
}

test_editor!(BarredRoot);
