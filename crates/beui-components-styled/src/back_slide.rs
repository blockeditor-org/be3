use beui_components_unstyled as unstyled;
use beui_components_unstyled::SlideCurves;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

use crate::frame::ColorRole;
use crate::theme::use_theme;
use crate::tokens::Movement;

/// A back slide's curves: pages enter, leave and come back on
/// `Movement::Settle`'s spring, carrying the speed the back gesture let them
/// go at.
pub const SLIDE_CURVES: SlideCurves = SlideCurves {
    enter: Movement::Settle.spring(),
    leave: Movement::Settle.spring(),
    settle: Movement::Settle.spring(),
};

/// `unstyled::BackSlide` moving on the theme's movements: a page a back
/// gesture carries away. `shade` dims the page `behind` while it shows.
#[component]
pub fn BackSlide(
    #[prop(default = true)] enabled: Prop<bool>,
    #[prop(default = true)] enters: bool,
    #[prop(default = Callback::default())] on_back: Callback<()>,
    open: Option<Prop<bool>>,
    #[prop(default = Callback::default())] on_gone: Callback<()>,
    #[prop(default = Callback::default())] on_presence: Callback<f32>,
    behind: Option<BuildFn<NodeId>>,
    #[prop(default = ColorRole::None)] shade: Prop<ColorRole>,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    let shade = create_memo(move || shade.get().resolve(&theme));
    // `view!` takes an optional prop only as a value, so each pair of the two
    // optional ones is its own tag.
    macro_rules! slide {
        ($($optional:tt)*) => {
            view! {
                <unstyled::BackSlide
                    curves=SLIDE_CURVES
                    enabled
                    enters
                    on_back={on_back}
                    on_gone={on_gone}
                    on_presence={on_presence}
                    shade={shade}
                    $($optional)*
                >
                    {children}
                </unstyled::BackSlide>
            }
        };
    }
    match (open, behind) {
        (None, None) => slide!(),
        (Some(open), None) => slide!(open = { open }),
        (None, Some(behind)) => slide!(behind = { move || behind.call() }),
        (Some(open), Some(behind)) => slide!(open={open} behind={move || behind.call()}),
    }
}
