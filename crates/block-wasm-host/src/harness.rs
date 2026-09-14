use std::{path::Path, time::Instant};

use block_gpu_abi as abi;
use block_gpu_host::Gpu;
use wasmtime::{Caller, Linker, Store, TypedFunc};
use wasmtime_wasi::{p1, FsPerms, I32Exit, WasiCtxBuilder};

use crate::{gpu, module, shared_memory, state::State, threads, transport, Host};

const START: &str = "_start";
const ALIGNMENT: u32 = 256;

impl Host {
    pub fn run_tests(&self, wasm: &Path, arguments: &[String], root: &Path) -> Result<i32, String> {
        let bytes = std::fs::read(wasm)
            .map_err(|error| format!("{} could not be read: {error}", wasm.display()))?;
        let module = module(&self.engine, &bytes)?;
        let memory = shared_memory(&self.engine, &module)?;
        let directory = root
            .to_str()
            .ok_or_else(|| format!("{} is not a usable path", root.display()))?;
        let mut builder = WasiCtxBuilder::new();
        builder
            .inherit_stdio()
            .inherit_env()
            .args(arguments)
            .preopened_dir(root, directory, FsPerms::ReadWrite)
            .map_err(|error| {
                format!(
                    "{} could not be opened for the tests: {error}",
                    root.display()
                )
            })?;
        let state = State {
            wasi: builder.build_p1(),
            memory: memory.clone(),
            gpu: Gpu::new(self.device.clone(), self.queue.clone()),
            inbox: Default::default(),
            outbox: Vec::new(),
            started: Instant::now(),
            threads: threads::Spawner::new(self.engine.clone(), module.clone(), memory.clone()),
        };
        let mut store = Store::new(&self.engine, state);
        let mut linker: Linker<State> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked: {error}"))?;
        linker
            .define(&store, "env", "memory", memory)
            .map_err(|error| format!("the test memory could not be linked: {error}"))?;
        gpu::link(&mut linker)?;
        transport::link(&mut linker)?;
        threads::link(&mut linker)?;
        link(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(&module)
            .map_err(|error| format!("the test imports could not be stubbed: {error}"))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| format!("the tests could not be instantiated: {error}"))?;
        let start: TypedFunc<(), ()> = instance
            .get_typed_func(&mut store, START)
            .map_err(|error| format!("the tests have no usable {START} export: {error}"))?;
        let outcome = start.call(&mut store, ());
        let state = store.data_mut();
        if let Some(failure) = state
            .gpu
            .take_error()
            .or_else(|| state.threads.take_failure())
        {
            return Err(failure);
        }
        match outcome {
            Ok(()) => Ok(0),
            Err(error) => match error.downcast_ref::<I32Exit>() {
                Some(exit) => Ok(exit.0),
                None => Err(format!("the tests trapped: {error:?}")),
            },
        }
    }
}

fn link(linker: &mut Linker<State>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::TEST_MODULE,
            "surface_read",
            |mut caller: Caller<'_, State>, surface: u32, pointer: u32, capacity: u32| -> u32 {
                let state = caller.data_mut();
                match surface_read(state, surface, pointer, capacity) {
                    Ok(needed) => needed,
                    Err(message) => {
                        state.report(message);
                        0
                    }
                }
            },
        )
        .map(|_| ())
        .map_err(|error| format!("surface_read could not be linked: {error}"))
}

fn surface_read(
    state: &mut State,
    surface: u32,
    pointer: u32,
    capacity: u32,
) -> Result<u32, String> {
    let texture = state
        .gpu
        .surface(surface)
        .map(|(texture, _)| texture.clone())
        .ok_or_else(|| format!("surface {surface} was never configured"))?;
    let needed = u64::from(texture.width()) * u64::from(texture.height()) * 4;
    let needed = u32::try_from(needed)
        .map_err(|_| format!("surface {surface} holds more pixels than a test can read"))?;
    if needed > capacity {
        return Ok(needed);
    }
    let pixels = read_back(state.gpu.device(), state.gpu.queue(), &texture)?;
    Ok(state.write(pointer, capacity, &pixels))
}

fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Vec<u8>, String> {
    let swizzle = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        format => return Err(format!("a test painted into a {format:?} surface, which reads back as no colour a snapshot holds")),
    };
    let width = texture.width();
    let height = texture.height();
    let stride = width * 4;
    let padded = stride.div_ceil(ALIGNMENT) * ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("plugin test readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("plugin test readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| format!("the device never finished the painting: {error}"))?;
    let mapped = buffer.slice(..).get_mapped_range().to_vec();
    Ok(rows(&mapped, width, height, padded, swizzle))
}

fn rows(mapped: &[u8], width: u32, height: u32, padded: u32, swizzle: bool) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * padded) as usize;
        for column in 0..width as usize {
            let texel = start + column * 4;
            let channels = [
                mapped[texel],
                mapped[texel + 1],
                mapped[texel + 2],
                mapped[texel + 3],
            ];
            match swizzle {
                true => {
                    pixels.extend_from_slice(&[channels[2], channels[1], channels[0], channels[3]])
                }
                false => pixels.extend_from_slice(&channels),
            }
        }
    }
    pixels
}
