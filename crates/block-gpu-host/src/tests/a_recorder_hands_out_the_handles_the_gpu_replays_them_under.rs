use super::*;

fn texture_descriptor() -> Vec<u8> {
    abi::encode(&abi::TextureDescriptor {
        label: "image".into(),
        size: abi::Extent3d {
            width: 4,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: abi::TextureDimension::D2,
        format: abi::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING.bits() | wgpu::TextureUsages::COPY_DST.bits(),
        view_formats: Vec::new(),
    })
}

type Step = Box<dyn Fn(&mut Recorder) -> Option<abi::Handle>>;

#[test]
fn a_recorder_hands_out_the_handles_the_gpu_replays_them_under() {
    let mut direct = gpu();
    let mut replayed = gpu();
    let mut recorder = Recorder::new(&direct.limits()).unwrap();

    let steps: Vec<Step> = vec![
        Box::new(|recorder| Some(recorder.create_buffer(&buffer_descriptor()))),
        Box::new(|recorder| Some(recorder.create_buffer(b"not a descriptor"))),
        Box::new(|recorder| Some(recorder.create_buffer(&buffer_descriptor()))),
        Box::new(|recorder| Some(recorder.acquire_surface(0))),
        Box::new(|recorder| {
            recorder.configure_surface(0, &configuration(16, 8));
            None
        }),
        Box::new(|recorder| Some(recorder.acquire_surface(0))),
        Box::new(|recorder| Some(recorder.create_texture(&texture_descriptor()))),
    ];
    let mut recorded = Vec::new();
    for step in &steps {
        recorded.push(step(&mut recorder));
    }
    for call in recorder.take_calls() {
        replayed.apply(call);
    }

    let expected = [
        Some(direct.create_buffer(&buffer_descriptor())),
        Some(direct.create_buffer(b"not a descriptor")),
        Some(direct.create_buffer(&buffer_descriptor())),
        Some(direct.acquire_surface(0)),
        {
            direct.configure_surface(0, &configuration(16, 8));
            None
        },
        Some(direct.acquire_surface(0)),
        Some(direct.create_texture(&texture_descriptor())),
    ];
    for (index, (recorded, expected)) in recorded.iter().zip(expected).enumerate() {
        if expected != Some(abi::NULL_HANDLE) {
            assert_eq!(*recorded, expected, "step {index} got another handle");
        }
    }

    let surface = recorded[5].unwrap();
    let texture = recorded[6].unwrap();
    for handle in [surface, texture] {
        assert_eq!(
            recorder.describe_texture(handle),
            replayed.describe_texture(handle),
            "the recorder describes texture {handle} unlike the gpu"
        );
    }
}
