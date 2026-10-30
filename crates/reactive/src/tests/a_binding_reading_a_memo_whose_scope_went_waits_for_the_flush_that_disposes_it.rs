use crate::create_binding;

use super::*;

#[test]
fn a_binding_reading_a_memo_whose_scope_went_waits_for_the_flush_that_disposes_it() {
    let outer = Scope::new();
    let shown = Rc::new(RefCell::new(Vec::new()));
    let (hover, place, tick) = outer.run(|| {
        let hover = create_signal(0);
        let place = create_signal(1);
        let tick = create_signal(0);
        let rows = create_signal(0);
        let items: Rc<RefCell<Option<Scope>>> = Rc::default();
        create_binding({
            let items = items.clone();
            move || {
                place.get();
                let gone = items.borrow_mut().take();
                if gone.is_some() {
                    drop(gone);
                    rows.update(|rows| *rows += 1);
                }
            }
        });
        let item = Scope::new();
        let label = item.run(|| create_memo(move || format!("place {}", place.get())));
        *items.borrow_mut() = Some(item);
        create_binding(move || {
            hover.get();
            label.get();
        });
        let row: Rc<RefCell<Option<Scope>>> = Rc::default();
        create_binding({
            let shown = shown.clone();
            move || {
                if rows.get() > 0 {
                    let gone = row.borrow_mut().take();
                    drop(gone);
                    return;
                }
                let view = Scope::new();
                let shown = shown.clone();
                view.run(|| {
                    let loud = create_memo(move || label.get().to_uppercase());
                    create_binding({
                        let shown = shown.clone();
                        move || shown.borrow_mut().push(loud.get())
                    });
                    create_binding(move || {
                        tick.get();
                        shown.borrow_mut().push(label.get());
                    });
                });
                *row.borrow_mut() = Some(view);
            }
        });
        (hover, place, tick)
    });

    outer.run(|| {
        hover.set(1);
        place.set(2);
        tick.set(1);
    });

    assert_eq!(
        *shown.borrow(),
        vec!["PLACE 1".to_owned(), "place 1".to_owned()],
        "the row that read the label goes with the label's scope, before it reads the label again"
    );
}
