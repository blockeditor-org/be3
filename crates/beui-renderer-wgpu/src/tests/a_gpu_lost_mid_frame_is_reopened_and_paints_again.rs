use super::*;
use crate::present::create_offscreen_gpu;

fn crowded(painter: &Painter) {
    for row in 0..SIZE * 2 {
        for column in 0..SIZE * 2 {
            let min = pos2(column as f32 / 2.0, row as f32 / 2.0);
            painter.rect_filled(
                Rect::from_min_max(min, min + vec2(0.5, 0.5)),
                0.0,
                Color32::WHITE,
            );
        }
    }
}

#[test]
fn a_gpu_lost_mid_frame_is_reopened_and_paints_again() {
    assert!(
        (SIZE * SIZE * 4) as usize > LISTED,
        "the crowded frame outgrows the instance list the renderer starts with"
    );
    let mut gpu =
        pollster::block_on(create_offscreen_gpu(FORMAT)).expect("no graphics adapter is available");
    let mut lost = Target::on(gpu.device.clone(), gpu.queue.clone(), FORMAT);
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(gpu.lost(), "a destroyed device reports itself lost");

    lost.draw(Color32::BLACK, Repaint::Everything, crowded);

    pollster::block_on(gpu.reopen(None)).expect("a replacement device opens");
    assert!(!gpu.lost(), "the replacement device is not lost");
    let mut target = Target::on(gpu.device.clone(), gpu.queue.clone(), FORMAT);
    target.draw(Color32::BLACK, Repaint::Everything, crowded);
    let capture = target.read();
    assert_eq!(capture.pixel(32, 32), [255, 255, 255, 255]);
}
