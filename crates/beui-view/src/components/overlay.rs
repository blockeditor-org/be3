use crate::reactive::Callback;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::reactive::{
    Get, Memo, Scope, Set, Signal, create_effect, create_memo, create_signal, store, untrack,
    use_context,
};
use crate::types::Color;
use crate::types::Dismiss;
use crate::types::LayerLevel;
use crate::types::LayerPolicy;
use crate::types::Placement;
use crate::types::Pos2;
use crate::types::WindowId;
use crate::types::WindowSpec;
use crate::types::window::{WindowPlacement, WindowRole};
use crate::window::Window;
use beui_macros::{component, view};
use std::cell::RefCell;

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color::TRANSPARENT)] scrim: Prop<Color>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    #[prop(default = 0.0)] beak: Prop<f32>,
    #[prop(default = None)] window: Prop<Option<WindowSpec>>,
    open: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    #[prop(default = true)] dismissable: Prop<bool>,
    on_dismiss: Callback<Dismiss>,
    #[prop(children)] content: RenderFn<()>,
) -> NodeId {
    let presence = OverlayPresence::new(create_memo(move || open.get()));
    let outer = crate::window::use_window();
    let windowed = match outer {
        Some(outer)
            if window.get_untracked().is_none() && crate::core::window::overlays_in_windows() =>
        {
            outer
        }
        _ => {
            return crate::core::overlay(
                anchor,
                placement,
                scrim,
                policy,
                level,
                traps_focus,
                shown_in,
                trigger,
                pointer,
                beak,
                window,
                keep_alive,
                dismissable,
                on_dismiss,
                presence,
                content,
            );
        }
    };
    let id = WindowId::next();
    let opens = crate::window::opens_windows();
    let asked = shown_in.clone();
    let own = create_memo(move || opens.get() && windowed.is_window() && asked.get().is_none());
    let shown_in = Prop::dynamic(move || match own.get() {
        true => Some(id),
        false => shown_in.get(),
    });
    let shown = presence.shown();
    create_effect(move || {
        if !shown.get() {
            untrack(|| presence.lingering().update(Vec::clear));
        }
    });
    let asked_open = presence.open();
    let on_dismiss = Callback::new(move |why: Dismiss| {
        if why != Dismiss::WithParent || asked_open.get_untracked() {
            on_dismiss.call(why);
        }
    });
    let inner = store(move || {
        crate::core::overlay(
            anchor.clone(),
            placement.clone(),
            scrim.clone(),
            policy.clone(),
            level.clone(),
            traps_focus.clone(),
            shown_in.clone(),
            trigger,
            pointer,
            beak.clone(),
            Prop::Static(None),
            keep_alive,
            dismissable.clone(),
            on_dismiss,
            presence,
            content.clone(),
        )
    });
    let size = create_memo(move || windowed.size.get());
    let real = windowed.watch_real_window();
    let over = create_memo(move || WindowPlacement::Over {
        window: real.get().unwrap_or(windowed.id),
        at: Pos2::ZERO,
    });
    let showing = create_memo(move || own.get() && shown.get());
    let node = create_signal(None::<NodeId>);
    let scope = store(RefCell::new(None::<Scope>));
    create_effect(move || {
        let windowed = own.get();
        untrack(|| {
            let built = Scope::detached();
            let shown = built.context().run(|| match windowed {
                true => view! {
                    <Window
                        id={Some(id)}
                        size={size}
                        placement={over}
                        role=WindowRole::Overlay
                        open={showing}
                        on_report={Callback::empty()}
                    >|_| {
                        crate::core::window::clear_window(id);
                        inner.with(|inner| inner())
                    }</Window>
                },
                false => inner.with(|inner| inner()),
            });
            node.set(Some(shown));
            drop(scope.value().replace(Some(built)));
        });
    });
    crate::core::portal(Prop::dynamic(move || node.get()))
}

#[derive(Clone, Copy)]
pub struct OverlayPresence {
    open: Memo<bool>,
    lingering: Signal<Vec<Memo<bool>>>,
}

impl OverlayPresence {
    pub(crate) fn new(open: Memo<bool>) -> Self {
        Self {
            open,
            lingering: create_signal(Vec::new()),
        }
    }

    pub(crate) fn lingering(self) -> Signal<Vec<Memo<bool>>> {
        self.lingering
    }

    pub(crate) fn shown(self) -> Memo<bool> {
        create_memo(move || {
            self.open.get()
                || self
                    .lingering
                    .with(|lingering| lingering.iter().any(Get::get))
        })
    }

    pub fn open(self) -> Memo<bool> {
        self.open
    }

    pub fn linger_while(self, showing: Memo<bool>) {
        self.lingering.update(|lingering| lingering.push(showing));
    }
}

pub fn use_overlay_presence() -> Option<OverlayPresence> {
    use_context::<OverlayPresence>()
}
