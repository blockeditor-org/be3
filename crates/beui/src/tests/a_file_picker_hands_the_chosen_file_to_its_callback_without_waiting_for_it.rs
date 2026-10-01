use super::*;
use crate::reactive::{FileFilter, PickedFile, Text, build, create_file_picker, view};
use crate::unstyled::Button;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn a_file_picker_hands_the_chosen_file_to_its_callback_without_waiting_for_it() {
    let button = NodeRef::new();
    let slot = button.clone();
    let chosen = Rc::new(RefCell::new(Vec::new()));
    let sink = chosen.clone();
    let document = build(move || {
        let picker = create_file_picker(move |picked| sink.borrow_mut().push(picked));
        let picking = picker.picking();
        let open = move || picker.open(FileFilter::new("Images", &["png"], &["image/png"]));
        view! {
            <Button @node_ref=&slot disabled={picking} on_click={open}>
                <Text string="Choose" />
            </Button>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let at = harness.center(button.get());

    let asked = click(&mut harness, at);
    let [request] = asked.as_slice() else {
        panic!("one click asks for one file, not {asked:?}");
    };
    assert_eq!(request.filter.extensions, ["png"]);

    assert!(click(&mut harness, at).is_empty());
    assert!(chosen.borrow().is_empty());

    let photo = PickedFile {
        name: "photo.png".into(),
        data: vec![1, 2, 3],
    };
    harness
        .context()
        .file_picked(request.id, Ok(Some(photo.clone())));
    harness.frame(Vec::new());
    assert_eq!(*chosen.borrow(), [Ok(photo)]);

    let again = click(&mut harness, at);
    let [cancelled] = again.as_slice() else {
        panic!("a finished pick lets the button ask again, not {again:?}");
    };
    harness.context().file_picked(cancelled.id, Ok(None));
    harness.frame(Vec::new());
    assert_eq!(chosen.borrow().len(), 1);
    assert_eq!(click(&mut harness, at).len(), 1);
}

fn click(harness: &mut Harness, at: Pos2) -> Vec<crate::FilePickRequest> {
    harness.frame(vec![Event::PointerMoved(at)]);
    harness.frame(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness
        .frame(vec![Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }])
        .file_picks
}
