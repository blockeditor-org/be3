use std::cell::RefCell;

use beui_macros::component;
use beui_reactive::{ErrorBoundary, Scope, create_binding, store};

use crate::children::{Branch, ChildSlot, Count, Counted, DynamicSegment, FormOf, Or, SlotChild};
use crate::component::{Build, RenderFn};
use crate::control::{built, mount};

/// Shows `content`, and `fallback` instead while a value read under it has failed
/// (`Pending::Failed`), whether or not a `Loading` around or inside it has revealed. The
/// fallback gets the failure and the boundary, whose `reset()` asks again: the async memos
/// made under it run once more, as do the failed ones a failure read under it came from, and
/// the content shows as soon as nothing read under it has failed. The content is kept while
/// the fallback shows.
#[component]
pub fn Errored<C, B, F>(
    fallback: RenderFn<(String, ErrorBoundary), F>,
    #[prop(children)] content: Build<B>,
) -> FormOf<C, <B as Counted>::Count, <F as Counted>::Count>
where
    C: SlotChild,
    B: Branch<C>,
    F: Branch<C>,
    <B as Counted>::Count: Or<<F as Counted>::Count>,
{
    let fallback = built(fallback);
    <<<B as Counted>::Count as Or<<F as Counted>::Count>>::Count as Count>::form(
        DynamicSegment::new(move |slot: ChildSlot<C>| {
            let scope = Scope::detached();
            let (boundary, host) = scope.context().run(|| {
                let boundary = ErrorBoundary::provide();
                (
                    boundary,
                    content.call().into_children().mount_hidden(slot.clone()),
                )
            });
            host.set_visible(true);
            let content = store((host, scope));
            let shown = store(RefCell::new(None::<(String, Scope)>));
            create_binding(move || {
                let failure = boundary.failure();
                let shown = shown.value();
                let mut shown = shown.borrow_mut();
                let content = content.value();
                match failure {
                    None => {
                        content.0.set_visible(true);
                        drop(shown.take());
                    }
                    Some(error) => {
                        if shown.as_ref().is_some_and(|(held, _)| *held == error) {
                            return;
                        }
                        content.0.set_visible(false);
                        let built = mount(&slot, || fallback.call((error.clone(), boundary)));
                        drop(shown.replace((error, built)));
                    }
                }
            });
        }),
    )
}
