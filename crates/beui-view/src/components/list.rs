use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Align;
use crate::types::Direction;
use crate::types::ItemSize;
use crate::types::Justify;
use crate::types::Sizing;
pub use crate::types::tree::ListItem;
use beui_macros::component;

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<ListItem>,
) -> NodeId {
    crate::core::list(direction, align, justify, wrap, spacing, children)
}

/// Places one child of a `List` or `Stack` and decides how long it is along the list. A
/// plain node in a list is a `ListItem` with no props: as long as it measures. `grow`
/// takes a share of the length the other children leave, by weight against the siblings
/// that grow; `length` fixes it, and cannot be given with `grow`. `shrink` gives up length
/// by weight when the children overflow, down to `min`, and `max` caps the length either
/// way. `align` overrides the list's cross alignment for this child, and `gap` the spacing
/// before it.
#[component]
pub fn ListItem(
    #[prop(default = 0.0)] grow: Prop<f32>,
    #[prop(default = None)] length: Prop<Option<f32>>,
    #[prop(default = 0.0)] min: Prop<f32>,
    #[prop(default = f32::INFINITY)] max: Prop<f32>,
    #[prop(default = 0.0)] shrink: Prop<f32>,
    #[prop(default = None)] align: Prop<Option<Align>>,
    #[prop(default = None)] gap: Prop<Option<f32>>,
    children: Child,
) -> ListItem {
    let sizing = move || {
        let grow = grow.get();
        let size = match length.get() {
            Some(length) => {
                assert!(
                    grow == 0.0,
                    "a `ListItem` takes `length` or `grow`, not both"
                );
                ItemSize::Fixed(length)
            }
            None if grow > 0.0 => ItemSize::Grow(grow),
            None => ItemSize::Intrinsic,
        };
        Sizing {
            size,
            min: min.get(),
            max: max.get(),
            shrink: shrink.get(),
            align: align.get(),
            gap: gap.get(),
        }
    };
    let fixed = [
        matches!(grow, Prop::Static(_)),
        matches!(length, Prop::Static(_)),
        matches!(min, Prop::Static(_)),
        matches!(max, Prop::Static(_)),
        matches!(shrink, Prop::Static(_)),
        matches!(align, Prop::Static(_)),
        matches!(gap, Prop::Static(_)),
    ];
    let data = match fixed.iter().all(|fixed| *fixed) {
        true => Prop::Static(crate::reactive::untrack(sizing)),
        false => Prop::dynamic(sizing),
    };
    ListItem {
        node: children,
        data,
    }
}

/// An empty child of a `List` or `Stack` that grows: by default it takes the length the
/// other children leave, pushing the ones after it to the end. `grow` weighs it against
/// the siblings that also grow.
#[component]
pub fn Spacer(#[prop(default = 1.0)] grow: Prop<f32>) -> ListItem {
    ListItem {
        node: crate::core::spacer(),
        data: grow.map(|grow| match grow > 0.0 {
            true => ItemSize::Grow(grow).into(),
            false => Sizing::default(),
        }),
    }
}
