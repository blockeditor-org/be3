use crate::reactive::Callback;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::types::Direction;
use beui_macros::component;
use std::hash::Hash;

/// Stands for one row per key and builds only the rows its slice of the enclosing
/// viewport reaches, so it goes wherever a tall child would: under a `Scroll`, beside
/// siblings, or nested. Use it only when building every row is the cost; layout already
/// culls nodes far from the view.
///
/// - `item_size` is an estimate. Built rows are laid out at their measured size and the
///   list reports their sum, counting unbuilt rows at the average, so its length is close
///   from the first layout. A row above the view that measured differently is reported
///   with `Document::report_anchor_shift`, so rows on screen stay put. Changing
///   `item_size` forgets measured sizes. Outside a scroll it builds what fits its box.
/// - `overscan` builds that many points of rows beyond each edge, so the row a key press
///   moves to already exists.
/// - Rows are recycled: `item` runs once per row built and gets a `ReadSignal` of its
///   key, which is set to a new key when a row that left is reused. Derive everything
///   shown from that signal; wrap content in `<Dynamic value={key}>` for fresh state per
///   key. A row holding the focus is never handed another key and stays laid out off
///   screen until the focus leaves it.
/// - Changing `keys` keeps every row whose key remains; a key inserted or removed above
///   the view moves the offset by the change. A list scrolled to its very top shows a
///   row inserted above instead.
/// - Tab order runs over every key, built or not (`Element::focus_children`). A list the
///   layout stops reaching releases its rows in `Element::unplaced`.
#[component]
pub fn VirtualList<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = 0.0)] overscan: Prop<f32>,
    text: Callback<K, String>,
    #[prop(children)] item: RenderFn<ReadSignal<K>>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    crate::core::virtual_list(keys, item_size, direction, overscan, text, item)
}
