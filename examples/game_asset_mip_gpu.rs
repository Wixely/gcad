//! Explicit atlas mip upload and bounded-sampler readback. No viewer integration.
use anyhow::{Result, ensure};
use egui_bench::viewer3d::Image;
#[path = "../tools/game-assets-renderer/explicit_image.rs"]
mod explicit_image;
use gcad::{
    asset::{Mesh, atlas, material::Material},
    model::texture::Map,
};
#[path = "game-assets/gpu_mip_raster.rs"]
mod raster;

fn explicit_upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    maps: &[&Map],
    srgb: bool,
) -> Result<explicit_image::UploadedImage> {
    let levels = maps
        .iter()
        .map(|map| Image {
            width: map.width,
            height: map.height,
            rgba: map.rgba.clone(),
        })
        .collect();
    let encoding = if srgb {
        explicit_image::ImageEncoding::Srgb
    } else {
        explicit_image::ImageEncoding::Linear
    };
    let image = explicit_image::ExplicitImage::new(levels, encoding).map_err(anyhow::Error::msg)?;
    ensure!(
        image.levels().len() == maps.len()
            && image.encoding() == encoding
            && image.max_lod() as usize + 1 == maps.len(),
        "explicit image contract mismatch"
    );
    image.upload(device, queue).map_err(anyhow::Error::msg)
}

fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    maps: &[&Map],
    srgb: bool,
    poison: bool,
) -> wgpu::TextureView {
    let base = maps[0];
    let count = if poison {
        base.width.max(base.height).ilog2() + 1
    } else {
        maps.len() as u32
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("atlas mip check"),
        size: wgpu::Extent3d {
            width: base.width as u32,
            height: base.height as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for level in 0..count {
        let (width, height) = (
            ((base.width as u32) >> level).max(1),
            ((base.height as u32) >> level).max(1),
        );
        let sentinel = [0, 255, 0, 255].repeat((width * height) as usize);
        let data = if let Some(map) = maps.get(level as usize) {
            assert_eq!((map.width, map.height), (width as usize, height as usize));
            &map.rgba
        } else {
            &sentinel
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&Default::default())
}

// Independent reference for GPU filtering of the already baked bytes. Decode
// before bilinear interpolation; alpha is always linear. NPOT levels differ.
fn reference(maps: &[&Map], uv: [f32; 2], lod: f32, srgb: bool) -> [f32; 4] {
    let sample = |level: usize| {
        let map = maps[level];
        let x = uv[0] * map.width as f32 - 0.5;
        let y = uv[1] * map.height as f32 - 0.5;
        let mut result = [0.; 4];
        for dy in 0..2 {
            for dx in 0..2 {
                let px = (x.floor() as i32 + dx).clamp(0, map.width as i32 - 1) as usize;
                let py = (y.floor() as i32 + dy).clamp(0, map.height as i32 - 1) as usize;
                let weight = if dx == 0 {
                    1. - x.fract().rem_euclid(1.)
                } else {
                    x.fract().rem_euclid(1.)
                } * if dy == 0 {
                    1. - y.fract().rem_euclid(1.)
                } else {
                    y.fract().rem_euclid(1.)
                };
                for c in 0..4 {
                    let v = map.rgba[4 * (py * map.width + px) + c] as f32 / 255.;
                    let v = if srgb && c < 3 {
                        if v <= 0.04045 {
                            v / 12.92
                        } else {
                            ((v + 0.055) / 1.055).powf(2.4)
                        }
                    } else {
                        v
                    };
                    result[c] += weight * v;
                }
            }
        }
        result
    };
    let lod = lod.clamp(0., maps.len() as f32 - 1.);
    let a = sample(lod.floor() as usize);
    let b = sample(lod.ceil() as usize);
    std::array::from_fn(|c| a[c] * (1. - lod.fract()) + b[c] * lod.fract())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(args.len() <= 1, "usage: game_asset_mip_gpu [vulkan|dx12]");
    let backends = match args.first().map(String::as_str) {
        None => wgpu::Backends::all(),
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("dx12") => wgpu::Backends::DX12,
        _ => anyhow::bail!("usage: game_asset_mip_gpu [vulkan|dx12]"),
    };
    let mesh = Mesh {
        positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        normals: vec![[0., 0., 1.]; 3],
        texcoords: Some(vec![[0., 0.], [1., 0.], [0., 1.]]),
        tangents: None,
        triangles: vec![[0, 1, 2]],
    };
    let atlas = atlas::pack(&mesh, 31, 17, 2, 4.)?;
    let map = Map {
        width: 16,
        height: 1,
        rgba: (0..16)
            .flat_map(|x| if x % 4 == 1 { [255; 4] } else { [0; 4] })
            .collect(),
    };
    let original = Material {
        colour_map: Some(map.clone()),
        roughness_map: Some(map),
        ..Material::default()
    };
    let chain = atlas.rebake_chain(&original, 2)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let info = adapter.get_info();
    println!(
        "Backend: {:?}; device type: {:?}",
        info.backend, info.device_type
    );
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let colour: Vec<_> = chain
        .levels()
        .iter()
        .map(|m| m.colour_map.as_ref().unwrap())
        .collect();
    let numeric: Vec<_> = chain
        .levels()
        .iter()
        .map(|m| m.roughness_map.as_ref().unwrap())
        .collect();
    let colour_upload = explicit_upload(&device, &queue, &colour, true)?;
    let numeric_upload = explicit_upload(&device, &queue, &numeric, false)?;
    let views = [
        colour_upload.view,
        numeric_upload.view,
        upload(&device, &queue, &colour, true, true),
    ];
    let sampler = |max| {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bounded atlas mip check"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0.,
            lod_max_clamp: max,
            anisotropy_clamp: 1,
            ..Default::default()
        })
    };
    let bounded = colour_upload.sampler;
    let unbounded = sampler(4.);
    // 100 UV positions spanning the chart bounds at five LODs, including a
    // fractional transition and a request far beyond the permitted chain.
    const SAMPLES: usize = 500;
    let bytes = (SAMPLES * 4 * 4 * 4) as u64;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("atlas mip samples"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var colour: texture_2d<f32>;
@group(0) @binding(1) var numeric: texture_2d<f32>;
@group(0) @binding(2) var poisoned: texture_2d<f32>;
@group(0) @binding(3) var bounded: sampler;
@group(0) @binding(4) var unbounded: sampler;
@group(0) @binding(5) var<storage,read_write> output: array<vec4<f32>>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i=id.x;
    if i>=500u { return; }
    let uv=(vec2<f32>(f32(i%10u),f32((i/10u)%10u))/9.0*4.0+2.5)/vec2<f32>(31.0,17.0);
    let lod=array<f32,5>(0.0,1.0,1.5,2.0,20.0)[i/100u];
    output[i*4u]=textureSampleLevel(colour,bounded,uv,lod);
    output[i*4u+1u]=textureSampleLevel(numeric,bounded,uv,lod);
    output[i*4u+2u]=textureSampleLevel(poisoned,bounded,uv,lod);
    output[i*4u+3u]=textureSampleLevel(poisoned,unbounded,uv,lod);
}
"#
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&views[2]),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&bounded),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&unbounded),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(8, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(30)),
    })?;
    rx.recv_timeout(std::time::Duration::from_secs(1))??;
    let mapped = readback.slice(..).get_mapped_range()?;
    let values: Vec<f32> = mapped
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let mut max_error = 0f32;
    for i in 0..SAMPLES {
        let uv = [
            (i % 10) as f32 / 9. * 4. + 2.5,
            ((i / 10) % 10) as f32 / 9. * 4. + 2.5,
        ];
        let uv = [uv[0] / 31., uv[1] / 17.];
        let lod = [0., 1., 1.5, 2., 20.][i / 100];
        let expected_colour = reference(&colour, uv, lod, true);
        let expected_numeric = reference(&numeric, uv, lod, false);
        for (slot, expected) in [
            expected_colour,
            expected_numeric,
            expected_colour,
            if i >= 400 {
                [0., 1., 0., 1.]
            } else {
                expected_colour
            },
        ]
        .iter()
        .enumerate()
        {
            for k in 0..4 {
                let actual = values[i * 16 + slot * 4 + k];
                let error = (actual - expected[k]).abs();
                ensure!(
                    actual.is_finite() && error < 0.002,
                    "GPU sample {i}, slot {slot}, channel {k}: {actual} != {}",
                    expected[k]
                );
                max_error = max_error.max(error);
            }
        }
    }
    println!(
        "PASS: 500 UV/LOD samples, 2000 RGBA results; three uploaded levels, sRGB colour, linear alpha/scalars, fractional LOD and max-LOD clamp; 100 unbounded sentinel controls detected; max error {max_error}"
    );
    raster::verify(&device, &queue)?;
    Ok(())
}
