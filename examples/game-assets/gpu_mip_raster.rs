//! Rasterized implicit-LOD check for adjacent atlas charts.
use anyhow::{Result, ensure};
use gcad::{
    asset::{Mesh, atlas, material::Material},
    model::texture::Map,
};
use wgpu::util::DeviceExt;

pub fn verify(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    let mesh = Mesh {
        positions: vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [2., 0., 0.],
            [3., 0., 0.],
            [2., 1., 0.],
        ],
        normals: vec![[0., 0., 1.]; 6],
        tangents: None,
        texcoords: Some(vec![
            [0.1, 0.1],
            [0.2, 0.1],
            [0.1, 0.2],
            [0.8, 0.1],
            [0.9, 0.1],
            [0.8, 0.2],
        ]),
        triangles: vec![[0, 1, 2], [3, 4, 5]],
    };
    let atlas = atlas::pack(&mesh, 128, 64, 8, 200.)?;
    ensure!(atlas.charts.len() == 2, "expected two adjacent charts");
    let material = Material {
        colour_map: Some(Map {
            width: 64,
            height: 1,
            rgba: (0..64)
                .flat_map(|x| {
                    if x < 32 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    }
                })
                .collect(),
        }),
        ..Material::default()
    };
    let chain = atlas.rebake_chain(&material, 3)?;
    let refusal = atlas.rebake_chain(&material, 4).unwrap_err().to_string();
    ensure!(
        refusal.contains("overlapping chart footprints"),
        "unexpected fourth-mip refusal: {refusal}"
    );
    let maps: Vec<_> = chain
        .levels()
        .iter()
        .map(|m| m.colour_map.as_ref().unwrap())
        .collect();
    let image = super::upload(device, queue, &maps, true, true);
    let bounded = super::explicit_upload(device, queue, &maps, true)?;
    let samplers = [
        bounded.sampler,
        device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_max_clamp: 7.,
            anisotropy_clamp: 1,
            ..Default::default()
        }),
    ];
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("implicit atlas LOD"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> }
@group(0) @binding(0) var image:texture_2d<f32>;
@group(0) @binding(1) var sampling:sampler;
@group(0) @binding(2) var<uniform> chart:vec4<f32>;
@vertex fn vertex(@builtin(vertex_index) i:u32)->Vertex {
    let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0))[i];
    var out:Vertex;
    out.position=vec4<f32>(p,0.0,1.0);
    out.uv=chart.xy+vec2<f32>((p.x+1.0)*0.5,(1.0-p.y)*0.5)*chart.zw;
    return out;
}
@fragment fn fragment(in:Vertex)->@location(0) vec4<f32> {
    return textureSample(image,sampling,in.uv);
}
"#
            .into(),
        ),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut pixels = 0;
    let mut controls = 0;
    for chart in &atlas.charts {
        let transform: [f32; 4] = [
            (chart.source_min[0] * chart.uv_scale[0] + chart.uv_offset[0]) as f32,
            (chart.source_min[1] * chart.uv_scale[1] + chart.uv_offset[1]) as f32,
            ((chart.source_max[0] - chart.source_min[0]) * chart.uv_scale[0]) as f32,
            ((chart.source_max[1] - chart.source_min[1]) * chart.uv_scale[1]) as f32,
        ];
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&transform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        for size in [64, 8, 2, 1] {
            for (mode, sampler) in samplers.iter().enumerate() {
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&image),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform.as_entire_binding(),
                        },
                    ],
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                    pass.set_pipeline(&pipeline);
                    pass.set_bind_group(0, &bind, &[]);
                    pass.set_viewport(0., 0., size as f32, size as f32, 0., 1.);
                    pass.draw(0..3, 0..1);
                }
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &target,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &readback,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(256),
                            rows_per_image: Some(64),
                        },
                    },
                    wgpu::Extent3d {
                        width: 64,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(30)),
                })?;
                rx.recv_timeout(std::time::Duration::from_secs(1))??;
                {
                    let bytes = readback.slice(..).get_mapped_range()?;
                    for y in 0..size {
                        for x in 0..size {
                            let i = (y * 256 + x * 4) as usize;
                            let pixel = &bytes[i..i + 4];
                            if mode == 1 && size <= 2 {
                                ensure!(
                                    pixel[1] > 32,
                                    "implicit LOD failed to expose sentinel at {size} pixels"
                                );
                                controls += 1;
                            } else {
                                let expected = if chart.source_min[0] < 0.5 {
                                    [255, 0, 0, 255]
                                } else {
                                    [0, 0, 255, 255]
                                };
                                ensure!(
                                    pixel == expected,
                                    "chart contamination at size {size}, mode {mode}: {pixel:?}"
                                );
                            }
                            pixels += 1;
                        }
                    }
                }
                readback.unmap();
            }
        }
    }
    println!(
        "PASS: rasterized adjacent charts, {pixels} pixels at 64/8/2/1 viewport sizes; implicit LOD, bounded chart colours; {controls} unbounded sentinel pixels detected"
    );
    Ok(())
}
