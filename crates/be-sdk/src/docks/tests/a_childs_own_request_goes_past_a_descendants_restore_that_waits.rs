use super::*;
use crate::Waker;
use be_sdk_api::{DockChildRequest, DockLayout, DockTabPath, DockTree};

const CHILD: ChildId = ChildId(3);
const NESTED: ChildId = ChildId(4);
const DEEPER: ChildId = ChildId(5);

fn restore() -> DockRequest {
    DockRequest::Restore(DockLayout {
        tree: DockTree {
            items: Vec::new(),
            focus: None,
        },
        homes: Vec::new(),
        bars: Vec::new(),
        outside: Vec::new(),
        unlocked: Vec::new(),
        compact: Vec::new(),
        auto_hidden: Vec::new(),
    })
}

fn show(tab: u64) -> DockRequest {
    DockRequest::Show(DockTabPath::own(tab))
}

/// A descendant's restore waits for the root to host its group, which can be for as long as the
/// group is missing: the child's own requests, and other descendants', go on past it, while the
/// waiting owner's later requests, and those of its own descendants, keep their order behind it.
#[test]
fn a_childs_own_request_goes_past_a_descendants_restore_that_waits() {
    let host = EditorHost::new(Waker::new(|| {}));
    let requests = [
        (vec![NESTED], restore()),
        (Vec::new(), show(1)),
        (vec![NESTED], show(2)),
        (vec![NESTED, DEEPER], show(3)),
        (vec![DEEPER], show(4)),
    ];
    host.receive_dock_updates(
        requests
            .iter()
            .map(|(owner, request)| {
                DockUpdate::ChildRequest(DockChildRequest {
                    child: CHILD,
                    owner: owner.clone(),
                    request: request.clone(),
                })
            })
            .collect(),
    );
    let mut taken = Vec::new();
    host.take_child_dock_requests(CHILD, |owner, request| {
        let waits = owner == [NESTED] && matches!(request, DockRequest::Restore(_));
        if !waits {
            taken.push((owner.to_vec(), request.clone()));
        }
        !waits
    });
    assert_eq!(
        taken,
        vec![(Vec::new(), show(1)), (vec![DEEPER], show(4)),],
        "only the waiting owner and its descendants are held back"
    );
    assert_eq!(host.docks.borrow().child_restored.get(&CHILD), None);

    let mut taken = Vec::new();
    host.take_child_dock_requests(CHILD, |owner, request| {
        taken.push((owner.to_vec(), request.clone()));
        true
    });
    assert_eq!(
        taken,
        requests[..4]
            .iter()
            .filter(|(owner, _)| !owner.is_empty())
            .cloned()
            .collect::<Vec<_>>(),
        "what waited goes in the order it was asked once the restore is taken"
    );
    assert_eq!(host.docks.borrow().child_restored.get(&CHILD), Some(&1));
}
