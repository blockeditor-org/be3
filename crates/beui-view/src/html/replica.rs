use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::on_cleanup;
use beui_core_html::document::create;

use super::bind;

pub(crate) fn replica(of: Prop<Option<NodeId>>) -> NodeId {
    let element = create("Replica", "div");
    let replica = std::rc::Rc::new(beui_core_html::replica::replica(element));
    let held = std::rc::Rc::clone(&replica);
    bind(of, move |of| held.hold(of));
    on_cleanup(move || replica.release());
    element
}
