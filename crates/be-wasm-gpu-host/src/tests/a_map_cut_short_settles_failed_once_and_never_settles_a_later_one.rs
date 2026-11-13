use super::*;

#[test]
fn a_map_cut_short_settles_failed_once_and_never_settles_a_later_one() {
    let mut gpu = gpu();
    let buffer = gpu
        .create_buffer(abi::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsages::MAP_READ.bits() | wgpu::BufferUsages::COPY_DST.bits(),
            mapped_at_creation: false,
        })
        .unwrap();

    let first = gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    gpu.buffer_unmap(buffer);
    assert_eq!(
        gpu.poll(false).maps,
        [(first, false)],
        "a map unmapped before it settled fails"
    );

    let second = gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    assert_ne!(second, first);
    assert_eq!(
        gpu.poll(true).maps,
        [(second, true)],
        "the first map's late callback settles nothing"
    );
    assert_eq!(
        gpu.poll(true).maps,
        Vec::<(u64, bool)>::new(),
        "each ticket is answered once"
    );
    gpu.buffer_unmap(buffer);

    let third = gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    gpu.destroy_device();
    let settled = gpu.poll(false);
    assert_eq!(settled.maps, [(third, false)]);
    assert_eq!(
        settled.lost.map(|lost| lost.reason),
        Some(abi::DeviceLostReason::Destroyed)
    );
    assert_eq!(gpu.take_error(), None);
}
