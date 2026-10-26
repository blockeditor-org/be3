use super::*;
use block_editor_beui::beui::{Input, Pos2, TouchPhase, pos2};

#[test]
fn text_left_empty_leaves_nothing_behind() {
    for finger in [true, false] {
        let mut editor = phone(&[]);
        let canvas = editor.rect_of("infinite-canvas.canvas");
        let spot = |x: f32, y: f32| pos2(canvas.left() + x, canvas.top() + y);
        let tap = |editor: &mut BeuiTest<CanvasApp>, at: Pos2| match finger {
            true => {
                editor.fingers(TouchPhase::Start, &[(1, at)]);
                editor.run();
                editor.fingers(TouchPhase::End, &[(1, at)]);
                editor.run();
            }
            false => {
                editor.click(at);
                editor.run();
            }
        };
        let write = |editor: &mut BeuiTest<CanvasApp>, at: Pos2| {
            editor.reveal("infinite-canvas.dock.tool.Text");
            match finger {
                true => editor.tap("infinite-canvas.dock.tool.Text"),
                false => editor.click("infinite-canvas.dock.tool.Text"),
            }
            editor.run();
            tap(editor, at);
        };

        write(&mut editor, spot(60.0, 120.0));
        editor.step(vec![Input::Text("Kept".to_owned())]);
        editor.run();
        write(&mut editor, spot(60.0, 260.0));
        write(&mut editor, spot(60.0, 400.0));
        assert_eq!(
            entities(&editor).len(),
            2,
            "finger {finger}: writing elsewhere removes the empty text"
        );
        let kept = format!("infinite-canvas.entity.{}", entities(&editor)[0].id);
        let kept_at = editor.rect_of(&kept).center();
        tap(&mut editor, kept_at);
        tap(&mut editor, kept_at);
        tap(&mut editor, spot(250.0, 500.0));
        let left: Vec<String> = entities(&editor)
            .iter()
            .map(|entity| entity.text.to_str_lossy().into_owned())
            .collect();
        assert_eq!(
            left,
            ["Kept"],
            "finger {finger}: switching to another text removes the empty one"
        );

        undo(&mut editor);
        assert!(
            entities(&editor).is_empty(),
            "finger {finger}: the empty texts left no step to undo, so one undo takes back the \
             written one, got {:?}",
            entities(&editor)
        );
    }
}
