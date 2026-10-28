use super::*;

#[test]
fn a_request_reaches_the_root_through_each_placer() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.heard(
        "inner",
        vec![DockMessage::Request(DockRegionRequest {
            region: RegionRef {
                instance: INNER,
                region: EditorRegion::Frame,
            },
            request: DockRequest::Show(path(&[], 5)),
        })],
    );
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::ChildRequest(DockChildRequest {
                child: OUTER_CHILD,
                owner: vec![INNER_CHILD],
                request: DockRequest::Show(path(&[INNER_CHILD], 5)),
            })
        )]
    );
}
