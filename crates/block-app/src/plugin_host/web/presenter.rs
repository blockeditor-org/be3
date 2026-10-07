use crate::plugin_host::presenter::{RegionLayout, SurfacePresenter};

pub(crate) struct Presenter {
    punch: wgpu::RenderPipeline,
}

pub(crate) fn presenter(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    regions: &RegionLayout,
    format: wgpu::TextureFormat,
) -> Result<Presenter, String> {
    let shader = device.create_shader_module(wgpu::include_wgsl!("punch.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("plugin canvas punch layout"),
        bind_group_layouts: &[Some(&regions.layout)],
        immediate_size: 0,
    });
    let punch = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("plugin canvas punch"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("punch_vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("punch_fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    Ok(Presenter { punch })
}

impl SurfacePresenter for Presenter {
    type Frame = ();

    fn replace(&mut self, _device: &wgpu::Device, _slot: u32, _frame: &()) -> Result<(), String> {
        Ok(())
    }

    fn prepare(&mut self, _queue: &wgpu::Queue, _slot: u32, _frame: &()) -> Result<(), String> {
        Ok(())
    }

    fn paint(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        _slot: u32,
        _surface: u32,
        regions: &wgpu::BindGroup,
        offset: u32,
    ) {
        pass.set_pipeline(&self.punch);
        pass.set_bind_group(0, regions, &[offset]);
        pass.draw(0..6, 0..1);
    }

    fn retain(&mut self, _slot: u32, _surfaces: &[u32]) {}

    fn release(&mut self, _slot: u32) {}
}
