use super::*;

#[test]
fn a_descriptor_round_trips_through_its_encoding() {
    let descriptor = device::RenderPassDescriptor {
        label: Some("screens".into()),
        color_attachments: vec![
            None,
            Some(device::ColorAttachment {
                view: device::TextureView(4),
                resolve_target: None,
                load: device::ColorLoadOp::Clear(device::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                }),
                store: device::StoreOp::Store,
                depth_slice: None,
            }),
        ],
        depth_stencil_attachment: None,
    };
    let bytes = be_ops::encode(&descriptor);
    assert_eq!(be_ops::decode(&bytes), Some(descriptor));
    assert_eq!(
        be_ops::decode::<device::RenderPassDescriptor>(&bytes[..bytes.len() / 2]),
        None
    );
}
