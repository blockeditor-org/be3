use super::*;

#[test]
fn a_lazy_representation_renders_only_when_the_host_asks() {
    let host = EditorHost::default();
    let renders = Rc::new(RefCell::new(0));
    let counted = renders.clone();
    let mut copy = Some(());
    let mut surface = launched(
        &host,
        Windowed::empty().hooked(move |context| {
            if copy.take().is_some() {
                let counted = counted.clone();
                context.copy(beui_core::clipboard::Clip::text("plain").with_lazy(
                    "image/png",
                    move |answer| {
                        *counted.borrow_mut() += 1;
                        answer(Some(vec![137, 80, 78, 71]));
                    },
                ));
            }
        }),
    );
    one(&mut surface, &whole(40.0, 30.0));
    let copies = host.take_copies();
    let [(serial, items)] = &copies[..] else {
        panic!("one copy, not {copies:?}");
    };
    assert_eq!(items[1].format, "image/png");
    assert_eq!(items[1].data, None);
    assert_eq!(*renders.borrow(), 0);

    let rendered = Rc::new(RefCell::new(None));
    let answered = rendered.clone();
    host.render_clip(
        *serial,
        "image/png",
        Box::new(move |data| *answered.borrow_mut() = Some(data)),
    );
    assert_eq!(*rendered.borrow(), Some(Some(vec![137, 80, 78, 71])));
    let stale = Rc::new(RefCell::new(None));
    let answered = stale.clone();
    host.render_clip(
        serial + 1,
        "image/png",
        Box::new(move |data| *answered.borrow_mut() = Some(data)),
    );
    assert_eq!(*stale.borrow(), Some(None));
}
