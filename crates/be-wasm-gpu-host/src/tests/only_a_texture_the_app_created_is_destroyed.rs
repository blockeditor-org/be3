use super::web::*;
use super::*;

#[test]
fn only_a_texture_the_app_created_is_destroyed() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (4, 4), Vec::new());
    let shown = gpu.gpu_canvas_context_get_current_texture(context);
    gpu.gpu_texture_destroy(GpuTexture(shown.0));
    let viewed = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_texture_create_view(GpuTexture(shown.0), None);
    });
    assert_eq!(
        viewed, None,
        "the canvas's texture was destroyed by the app"
    );

    let made = texture(&mut gpu, device, GpuTextureUsage::RENDER_ATTACHMENT);
    gpu.gpu_texture_destroy(GpuTexture(made.0));
    let mut view = None;
    let viewed = scoped(&mut gpu, device, |gpu| {
        view = Some(gpu.gpu_texture_create_view(GpuTexture(made.0), None));
    });
    assert_eq!(
        viewed, None,
        "a destroyed texture's view is made, as WebGPU says"
    );
    let queue = gpu.gpu_device_queue(device);
    let used = scoped(&mut gpu, device, |gpu| {
        let encoder = gpu.gpu_device_create_command_encoder(device, None);
        let pass = gpu.gpu_command_encoder_begin_render_pass(
            encoder,
            GpuRenderPassDescriptor {
                color_attachments: vec![Some(GpuRenderPassColorAttachment {
                    view: view.unwrap(),
                    depth_slice: None,
                    resolve_target: None,
                    clear_value: None,
                    load_op: GpuLoadOp::Clear,
                    store_op: GpuStoreOp::Store,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                max_draw_count: None,
                label: None,
            },
        );
        gpu.gpu_render_pass_encoder_end(pass);
        let commands = gpu.gpu_command_encoder_finish(encoder, None);
        gpu.gpu_queue_submit(queue, &[commands]);
    });
    assert!(used.is_some(), "drawing into it is what fails");
}
