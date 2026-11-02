use super::*;

#[test]
fn a_timestamp_query_set_needs_its_feature_requested() {
    let (device, _queue) = gpu();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let _queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let refused = refusal(scope);
    assert!(
        refused.contains("TIMESTAMP_QUERY"),
        "the query set was refused as {refused}"
    );
}
