use super::web::*;
use super::*;

#[test]
fn a_map_cut_short_settles_failed_once_and_never_settles_a_later_one() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let buffer = gpu.gpu_device_create_buffer(
        device,
        GpuBufferDescriptor {
            size: 8,
            usage: GpuBufferUsage::MAP_READ | GpuBufferUsage::COPY_DST,
            mapped_at_creation: None,
            label: None,
        },
    );
    let map = |gpu: &mut Gpu, ticket| {
        gpu.gpu_buffer_map_async(
            GpuBuffer(buffer.0),
            GpuMapMode::READ,
            None,
            None,
            be_wasm_ops::local::Promise::new(ticket),
        )
    };
    map(&mut gpu, 1);
    gpu.gpu_buffer_unmap(GpuBuffer(buffer.0)).unwrap();
    let first: Result<(), MapAsyncError> = settled(&mut gpu, 1);
    assert_eq!(
        first.map_err(|error| error.kind),
        Err(MapAsyncErrorKind::AbortError),
        "a map unmapped before it settled fails"
    );

    map(&mut gpu, 2);
    let second: Result<(), MapAsyncError> = settled(&mut gpu, 2);
    assert!(second.is_ok(), "the first map's late callback settles nothing of the second");
    assert!(
        gpu.settle(false).is_empty(),
        "each ticket is answered once"
    );
    gpu.gpu_buffer_unmap(GpuBuffer(buffer.0)).unwrap();

    map(&mut gpu, 3);
    gpu.gpu_device_destroy(device);
    let third: Result<(), MapAsyncError> = settled(&mut gpu, 3);
    assert!(third.is_err(), "destroying the device fails its pending map");
    assert!(gpu.take_logged().is_empty());
}
