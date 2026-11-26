use super::*;

use webgpu::webgpu as web;

#[test]
fn a_descriptor_round_trips_through_its_encoding() {
    let descriptor = web::GpuRenderPassDescriptor {
        color_attachments: vec![
            None,
            Some(web::GpuRenderPassColorAttachment {
                view: web::GpuTextureView(4),
                depth_slice: None,
                resolve_target: None,
                clear_value: Some(web::GpuColor {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                }),
                load_op: web::GpuLoadOp::Clear,
                store_op: web::GpuStoreOp::Store,
            }),
        ],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        max_draw_count: None,
        label: Some("screens".into()),
    };
    let bytes = be_wasm_ops::encode(&descriptor);
    assert_eq!(be_wasm_ops::decode(&bytes), Some(descriptor));
    assert_eq!(
        be_wasm_ops::decode::<web::GpuRenderPassDescriptor>(&bytes[..bytes.len() / 2]),
        None
    );
}
