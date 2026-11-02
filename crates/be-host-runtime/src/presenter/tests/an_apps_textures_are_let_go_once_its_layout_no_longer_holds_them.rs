use super::*;
use crate::surface::{PresentedTexture, SurfaceFrame};

const SLOT: u32 = 0;

fn device() -> (wgpu::Device, wgpu::Queue) {
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        },
    ))
    .expect("no graphics adapter is available");
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("the adapter did not provide a device")
}

fn presented(device: &wgpu::Device, surface: u32) -> PresentedTexture {
    PresentedTexture {
        surface,
        texture: device.create_texture(&wgpu::TextureDescriptor {
            label: Some("app surface"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }),
        generation: 1,
    }
}

fn holds(surface: u32) -> bool {
    PRESENTER.with(|presenter| {
        presenter
            .borrow()
            .as_ref()
            .and_then(|presenter| presenter.platform.as_ref())
            .is_some_and(|platform| platform.texture(SLOT, surface).is_some())
    })
}

#[test]
fn an_apps_textures_are_let_go_once_its_layout_no_longer_holds_them() {
    let (device, queue) = device();
    let mut setup = beui::native::Setup::new(beui::native::Waker::new(|| {}));
    setup.provide(beui_host::native::GpuSetup {
        device: device.clone(),
        queue,
        format: wgpu::TextureFormat::Rgba8Unorm,
    });
    install(&setup);
    PRESENTER.with(|presenter| {
        let mut presenter = presenter.borrow_mut();
        let platform = presenter
            .as_mut()
            .and_then(|presenter| presenter.platform.as_mut())
            .expect("the presenter is installed");
        let frame = SurfaceFrame {
            textures: vec![presented(&device, 5), presented(&device, 6)],
            presents: 1,
            damage: None,
        };
        platform
            .replace(&device, SLOT, &frame)
            .expect("the frame is shown");
    });
    assert!(holds(5) && holds(6));

    keep(SLOT, &[6]);
    assert!(
        !holds(5),
        "a view the app no longer lays out lets its texture go"
    );
    assert!(holds(6), "a view it still lays out keeps its texture");

    keep(SLOT, &[]);
    assert!(
        !holds(6),
        "an app that shows nothing holds no textures, though it draws no frame to say so"
    );
    stop();
    crate::surface::stop();
}
