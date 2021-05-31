use std::borrow::Cow;

use crossbeam_channel::Sender;
use glam::Vec3;
use legion::*;
use once_cell::sync::OnceCell;

use crate::{
    rendering::{camera::Camera, common::DEPTH_FORMAT, gltf::InstanceData},
    tilemap::{DrawableTileMap, TileVertex},
};

use super::{
    common::DepthTexture,
    vertex_buffers::{VertexBuffer, VertexBufferData},
};

impl VertexBuffer for TileVertex {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                format: wgpu::VertexFormat::Float32x3,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<Vec3>() as u64,
                format: wgpu::VertexFormat::Float32x2,
                shader_location: 1,
            },
        ]
    }
}

pub fn get_or_create_tilemap_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
    static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
    LAYOUT.get_or_init(move || {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        comparison: false,
                        filtering: false,
                    },
                    count: None,
                },
            ],
            label: Some("Tilemap bind group layout"),
        })
    })
}
pub struct TileMapPass {
    render_pipeline: wgpu::RenderPipeline,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl TileMapPass {
    pub fn new(device: &wgpu::Device, command_sender: Sender<wgpu::CommandBuffer>) -> TileMapPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/tilemap.wgsl"))),
            flags: wgpu::ShaderFlags::VALIDATION,
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tilemap pipeline layout"),
                bind_group_layouts: &[
                    &get_or_create_tilemap_layout(device),
                    Camera::get_or_create_layout(device),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Heightmap pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[TileVertex::get_descriptor(), InstanceData::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[wgpu::TextureFormat::Bgra8UnormSrgb.into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
        });

        TileMapPass {
            render_pipeline,
            command_sender,
        }
    }
}

#[system]
pub fn update(#[resource] queue: &wgpu::Queue, #[resource] tilemap: &mut DrawableTileMap) {
    tilemap.update(queue);
}

#[system]
pub fn draw(
    #[resource] pass: &TileMapPass,
    #[resource] current_frame: &wgpu::SwapChainTexture,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] camera: &Camera,
    #[resource] tile_map: &DrawableTileMap,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Tilemap pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Tilemap render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view: &current_frame.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        }],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_texture.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            }),
            stencil_ops: None,
        }),
    });
    render_pass.push_debug_group("Tilemap pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_vertex_buffer(0, tile_map.render_data.vertex_buffer.slice(..));
    render_pass.set_index_buffer(
        tile_map.render_data.index_buffer.slice(..),
        wgpu::IndexFormat::Uint32,
    );
    render_pass.set_vertex_buffer(1, tile_map.render_data.instance_buffer.slice(..));
    render_pass.set_bind_group(0, &tile_map.render_data.bind_group, &[]);
    render_pass.set_bind_group(1, &camera.bind_group(), &[]);
    render_pass.draw_indexed(0..tile_map.render_data.num_indexes, 0, 0..1);
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}
