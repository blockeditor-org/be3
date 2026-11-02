use beui::Rect;
use beui::reactive::*;

use crate::{Editor, Fit};

#[derive(Clone, Copy)]
pub struct Fitting {
    editor: Editor,
    fit: Memo<Option<Fit>>,
}

impl Editor {
    pub fn fit(&self, world: Rect, room: Rect, max_scale: f32) -> bool {
        self.core().fit(world, room, max_scale)
    }

    pub fn keep_fitted(&self, fit: impl Fn() -> Option<Fit> + 'static) -> Fitting {
        let editor = *self;
        let fit = create_memo(fit);
        let world = self.world();
        let core = self.core();
        let area = create_memo(move || {
            let _ = world.get();
            fit.get().map(|fit| core.fit_area(fit))
        });
        create_effect(move || {
            let area = area.get();
            untrack(|| editor.keep_fitted_area(area));
        });
        Fitting { editor, fit }
    }
}

impl Fitting {
    pub fn fit(&self) {
        self.editor.core().fit_now(self.fit.get_untracked());
    }
}
