use crate::reactive::Callback;
use crate::reactive::ForEachRef;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::reactive::RowArgs;
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
/// - `anchor` names the row kept in place while rows around it are measured anew (a text's
///   line at the top of its view); with none, or while it is not built, the first row built
///   is. The web leaves this to the browser's scroll anchoring (`overflow-anchor`), which
///   Safari lacks: there the rows above the view still move it.
/// - Rows are recycled: `item` runs once per row built and gets a `ReadSignal` of its
///   key, which is set to a new key when a row that left is reused, and may take its
///   `index` after it, as a `ForEach` row does (`|key: ReadSignal<K>, index:
///   ReadSignal<usize>|`), which follows the key. Derive everything shown from those
///   signals; wrap content in `<Dynamic value={key}>` for fresh state per key.
/// - Given a `handle`, it tells it the keys and the rows built (`ForEachRef`): a key not
///   built has a place but no nodes. A row holding the focus is never handed another key and stays laid out off
///   screen until the focus leaves it.
/// - Changing `value` keeps every row whose key remains; a key inserted or removed above
///   the view moves the offset by the change. A list scrolled to its very top shows a
///   row inserted above instead.
/// - Tab order runs over every key, built or not (`Element::focus_children`). A list the
///   layout stops reaching releases its rows in `Element::unplaced`.
#[component]
pub fn VirtualList<K, H>(
    value: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = 0.0)] overscan: Prop<f32>,
    #[prop(default = None)] anchor: Prop<Option<K>>,
    text: Callback<K, String>,
    #[prop(default = None)] handle: Option<ForEachRef<K, NodeId>>,
    #[prop(children)] item: RenderFn<H, NodeId>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
    H: RowArgs<ReadSignal<K>>,
{
    let item = RenderFn::new(move |(key, index): (ReadSignal<K>, ReadSignal<usize>)| {
        item.call(H::row(key, index))
    });
    crate::core::virtual_list(
        value, item_size, direction, overscan, anchor, text, handle, item,
    )
}
