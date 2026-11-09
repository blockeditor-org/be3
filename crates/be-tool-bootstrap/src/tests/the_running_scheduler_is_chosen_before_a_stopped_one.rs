use super::*;

#[test]
fn the_running_scheduler_is_chosen_before_a_stopped_one() {
    let sandboxes = [
        serde_json::json!({ "id": "a", "state": "archived" }),
        serde_json::json!({ "id": "b", "state": "idle" }),
        serde_json::json!({ "id": "c", "state": "deleting" }),
    ];
    assert_eq!(
        nativelink::pick_scheduler(&sandboxes).map(|s| s["id"].clone()),
        Some("b".into())
    );
    assert_eq!(
        nativelink::pick_scheduler(&sandboxes[2..]).map(|s| s["id"].clone()),
        None
    );
    assert_eq!(
        nativelink::pick_scheduler(&sandboxes[..1]).map(|s| s["id"].clone()),
        Some("a".into())
    );
    assert!(nativelink::pick_scheduler(&[]).is_none());
}
