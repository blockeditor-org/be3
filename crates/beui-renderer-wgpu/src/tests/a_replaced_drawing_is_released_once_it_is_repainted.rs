use super::*;
use std::cell::Cell;
use std::rc::Rc;

use crate::Draw;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
    let setter: Rc<RefCell<Option<Signal<Option<beui_core::drawing::Drawing>>>>> = Rc::default();
    let initial = beui_draw::drawing(Held(first.clone()));
    let mut document = build({
        let setter = setter.clone();
        move || {
            let drawing = create_signal(Some(initial));
            *setter.borrow_mut() = Some(drawing);
            view! {
                <Root clear_color={beui_types::color::Color32::BLACK}>
                    <Drawing draw={Prop::dynamic(move || draw_gpu(drawing.get()))} />
                </Root>
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
    let next = beui_draw::drawing(Held(second.clone()));
    setter
        .borrow()
        .as_ref()
        .expect("the document was built")
        .set(Some(next));
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
