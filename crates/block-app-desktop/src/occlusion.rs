use beui::Rect;
use beui::reactive::*;

#[derive(Clone)]
pub struct Occlusion(pub Memo<Vec<Rect>>);

pub fn occlusion() -> Option<Memo<Vec<Rect>>> {
    use_context::<Occlusion>().map(|Occlusion(rects)| rects)
}
