use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::reactive::on_cleanup;
use beui_core::current::with_document;
use beui_core::document::Document;
use beui_types::NodeId;

pub(crate) fn replica(of: Prop<Option<NodeId>>) -> NodeId {
    let replica = with_document(Document::create_replica);
    create_binding(move || {
        let of = of.get();
        with_document(|document| document.set_replica_of(replica, of));
    });
    on_cleanup(move || {
        beui_core::current::try_with_document(|document| document.set_replica_of(replica, None));
    });
    replica.id()
}
