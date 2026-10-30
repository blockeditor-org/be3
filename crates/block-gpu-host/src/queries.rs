use crate::Gpu;

pub(crate) struct QuerySet {
    pub(crate) set: wgpu::QuerySet,
    pub(crate) timestamps: bool,
}

/// Browsers hand out timestamps in nanoseconds, coarsened to 100
/// microseconds so that a page cannot time the gpu finely; Chrome does it with
/// a compute pass after each resolve, as this does. A device counts in ticks
/// of `Queue::get_timestamp_period` nanoseconds, so the pass also converts.
pub(crate) const QUANTUM_NANOSECONDS: u32 = 100_000;

const WORKGROUP: u32 = 64;

const SHADER: &str = "
@group(0) @binding(0) var<storage, read_write> stamps: array<vec2<u32>>;

const PERIOD: u32 = PERIOD_FIXEDu;
const QUANTUM: u32 = QUANTUM_NANOSECONDSu;

fn multiply(a: u32, b: u32) -> vec2<u32> {
    let low = (a & 0xffffu) * (b & 0xffffu);
    let across = (a >> 16u) * (b & 0xffffu);
    let down = (a & 0xffffu) * (b >> 16u);
    let middle = (low >> 16u) + (across & 0xffffu) + (down & 0xffffu);
    let high = (a >> 16u) * (b >> 16u) + (across >> 16u) + (down >> 16u) + (middle >> 16u);
    return vec2((low & 0xffffu) | (middle << 16u), high);
}

@compute @workgroup_size(WORKGROUP)
fn coarsen(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= arrayLength(&stamps) {
        return;
    }
    let ticks = stamps[id.x];
    let low = multiply(ticks.x, PERIOD);
    let high = multiply(ticks.y, PERIOD);
    let middle = low.y + high.x;
    let top = high.y + select(0u, 1u, middle < low.y);
    let nanoseconds = vec2((low.x >> 16u) | (middle << 16u), (middle >> 16u) | (top << 16u));
    var rest = 0u;
    for (var byte = 0u; byte < 8u; byte++) {
        let word = select(nanoseconds.x, nanoseconds.y, byte < 4u);
        rest = (rest * 256u + ((word >> (24u - 8u * (byte % 4u))) & 0xffu)) % QUANTUM;
    }
    let borrow = select(0u, 1u, nanoseconds.x < rest);
    stamps[id.x] = vec2(nanoseconds.x - rest, nanoseconds.y - borrow);
}
";

#[derive(Clone)]
pub(crate) struct Coarsening {
    pipeline: wgpu::ComputePipeline,
}

impl Coarsening {
    fn new(device: &wgpu::Device, period: f32) -> Self {
        let fixed = (f64::from(period) * 65536.0)
            .round()
            .clamp(1.0, f64::from(u32::MAX)) as u32;
        let source = SHADER
            .replace("PERIOD_FIXED", &fixed.to_string())
            .replace("QUANTUM_NANOSECONDS", &QUANTUM_NANOSECONDS.to_string())
            .replace("WORKGROUP", &WORKGROUP.to_string());
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("timestamp coarsening"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("timestamp coarsening"),
            layout: None,
            module: &module,
            entry_point: Some("coarsen"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self { pipeline }
    }

    /// Whether a resolve into this range is valid as far as the coarsening
    /// pass's own binding goes. When it is not, the resolve itself is refused,
    /// and the pass is left out so that the resolve's error is the one told.
    pub(crate) fn fits(buffer: &wgpu::Buffer, offset: u64, count: u32) -> bool {
        let end = offset.checked_add(u64::from(count) * 8);
        count > 0
            && offset % wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT == 0
            && buffer.usage().contains(wgpu::BufferUsages::STORAGE)
            && end.is_some_and(|end| end <= buffer.size())
    }

    pub(crate) fn record(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        buffer: &wgpu::Buffer,
        offset: u64,
        count: u32,
    ) {
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("timestamp coarsening"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset,
                    size: wgpu::BufferSize::new(u64::from(count) * 8),
                }),
            }],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("timestamp coarsening"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
    }
}

impl Gpu {
    pub(crate) fn coarsening(&mut self) -> &Coarsening {
        let (device, queue) = (&self.device, &self.queue);
        self.coarsening
            .get_or_insert_with(|| Coarsening::new(device, queue.get_timestamp_period()))
    }

    /// A buffer queries resolve into is also bound by the coarsening pass.
    pub(crate) fn resolvable(&self, usage: wgpu::BufferUsages) -> wgpu::BufferUsages {
        let timestamps = self.device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        match timestamps && usage.contains(wgpu::BufferUsages::QUERY_RESOLVE) {
            true => usage | wgpu::BufferUsages::STORAGE,
            false => usage,
        }
    }
}
