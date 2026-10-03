use super::*;
use std::cell::Cell;
use std::rc::Rc;

use beui_view::reactive::{
    Drawing, Prop, WriteSignal, build, create_signal, draw_gpu, view, with_reactive_scope,
};

struct Held(Rc<Cell<bool>>);

impl Draw for Held {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _at: DrawAt,
    ) {
    }

    fn paint(&self, _pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {}
}

impl Drop for Held {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn a_replaced_drawing_is_released_once_it_is_repainted() {
    let first = Rc::new(Cell::new(false));
    let setter: Rc<RefCell<Option<WriteSignal<Option<beui_core::drawing::Drawing>>>>> =
        Rc::default();
    let initial = crate::drawing(Held(first.clone()));
    let mut document = build({
        let setter = setter.clone();
        move || {
            let (drawing, set_drawing) = create_signal(Some(initial));
            *setter.borrow_mut() = Some(set_drawing);
            view! {
                <Drawing draw={Prop::Dynamic(Rc::new(move || draw_gpu(drawing.get())))} />
            }
        }
    });
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut target = Target::new();
    let everything = everything();
    target.draw_in(
        &context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| document.show(painter.ctx(), everything),
    );

    let second = Rc::new(Cell::new(false));
    let next = crate::drawing(Held(second.clone()));
    with_reactive_scope(&mut document, || {
        setter
            .borrow()
            .as_ref()
            .expect("the document was built")
            .set(Some(next));
    });
    for _ in 0..2 {
        target.draw_in(
            &context,
            Color32::BLACK,
            |output| output.repaint(Color32::BLACK),
            |painter| document.show(painter.ctx(), everything),
        );
    }

    assert!(
        first.get(),
        "a drawing the document replaced is not held by the renderer's encoded lists"
    );
    assert!(!second.get(), "the drawing on screen is still held");
}
