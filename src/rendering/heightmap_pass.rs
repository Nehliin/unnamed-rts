#![allow(dead_code)]
use super::{
    camera::Camera, common::DepthTexture, common::DEPTH_FORMAT, texture::update_texture_data,
};
use super::{gltf::InstanceData, vertex_buffers::ImmutableVertexData};
use crate::{assets::AssetLoader, components::Transform};
use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use crossbeam_channel::Sender;
use glam::Vec2;
use legion::{self, *};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use super::{
    texture::{allocate_simple_texture, TextureContent},
    vertex_buffers::{MutableVertexData, VertexBuffer, VertexBufferData},
};
#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy)]
pub struct MapVertex {
    position: Vec2,
    tex_coords: Vec2,
}

impl VertexBuffer for MapVertex {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                format: wgpu::VertexFormat::Float32x2,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<Vec2>() as u64,
                format: wgpu::VertexFormat::Float32x2,
                shader_location: 1,
            },
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerilizableHeightMap {
    size: u32,
    name: String,
    displacement_buffer: Vec<u8>,
    color_buffer: Vec<u8>,
    transform: Transform,
}

#[derive(Debug)]
pub struct HeightMap<'a> {
    name: String,
    vertex_buffer: ImmutableVertexData<MapVertex>,
    index_buffer: wgpu::Buffer,
    num_indexes: u32,
    displacement_texture: wgpu::Texture,
    displacement_content: TextureContent<'a>,
    color_texture: wgpu::Texture,
    color_content: TextureContent<'a>,
    decal_layer_texture: wgpu::Texture,
    decal_layer_content: TextureContent<'a>,
    needs_decal_update: bool,
    needs_color_displacement_update: bool,
    bind_group: wgpu::BindGroup,
    transform: Transform,
    // TODO remove
    instance_buffer: MutableVertexData<InstanceData>,
    size: u32,
}

fn create_vertecies(size: u32) -> (Vec<MapVertex>, Vec<u32>) {
    let mut vertecies = Vec::with_capacity((size * size) as usize);
    let mut indicies: Vec<u32> = Vec::with_capacity((size * size) as usize);
    for i in 0..size {
        for j in 0..size {
            let index = vertecies.len() as u32;
            let a = MapVertex {
                position: Vec2::new(i as f32, j as f32),
                tex_coords: Vec2::new(i as f32 / size as f32, j as f32 / size as f32),
            };
            let b = MapVertex {
                position: Vec2::new(1.0 + i as f32, j as f32),
                tex_coords: Vec2::new((1.0 + i as f32) / size as f32, j as f32 / size as f32),
            };
            let c = MapVertex {
                position: Vec2::new(1.0 + i as f32, 1.0 + j as f32),
                tex_coords: Vec2::new(
                    (1.0 + i as f32) / size as f32,
                    (1.0 + j as f32) / size as f32,
                ),
            };
            let d = MapVertex {
                position: Vec2::new(i as f32, 1.0 + j as f32),
                tex_coords: Vec2::new(i as f32 / size as f32, (1.0 + j as f32) / size as f32),
            };
            vertecies.push(a);
            vertecies.push(b);
            vertecies.push(c);
            vertecies.push(d);
            indicies.push(index);
            indicies.push(index + 1);
            indicies.push(index + 2);
            indicies.push(index);
            indicies.push(index + 2);
            indicies.push(index + 3);
        }
    }
    (vertecies, indicies)
}

impl<'a> HeightMap<'a> {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: String,
        size: u32,
        transform: Transform,
    ) -> HeightMap<'a> {
        let texture_size = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        let texels = vec![0; (size * size) as usize];
        let texture = TextureContent {
            label: Some("Displacement map"),
            format: wgpu::TextureFormat::R8Unorm,
            bytes: Cow::Owned(texels),
            stride: 1,
            size: texture_size,
        };
        HeightMap::from_textures(
            device,
            queue,
            name,
            size,
            texture,
            TextureContent::checkerd(size),
            transform,
        )
    }

    pub fn from_serialized(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        serialized_height_map: SerilizableHeightMap,
    ) -> HeightMap<'a> {
        let SerilizableHeightMap {
            size,
            name,
            displacement_buffer,
            color_buffer,
            transform,
        } = serialized_height_map;
        let texture_size = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        let displacement_content = TextureContent {
            label: Some("Displacement map"),
            format: wgpu::TextureFormat::R8Unorm,
            bytes: Cow::Owned(displacement_buffer),
            stride: 1,
            size: texture_size,
        };
        let color_content = TextureContent {
            label: Some("Color texture map"),
            format: wgpu::TextureFormat::Rgba8Unorm,
            bytes: Cow::Owned(color_buffer),
            stride: 4,
            size: texture_size,
        };
        HeightMap::from_textures(
            device,
            queue,
            name,
            size,
            displacement_content,
            color_content,
            transform,
        )
    }

    pub fn from_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: String,
        size: u32,
        displacement_content: TextureContent<'a>,
        color_content: TextureContent<'a>,
        transform: Transform,
    ) -> HeightMap<'a> {
        let (vertecies, indicies) = create_vertecies(size);
        let num_indexes = indicies.len() as u32;
        let vertex_buffer = MapVertex::allocate_immutable_buffer(device, &vertecies);
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Heightmap Index buffer"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indicies),
        });
        let instance_buffer = InstanceData::allocate_mutable_buffer(
            device,
            &[InstanceData::new(transform.get_model_matrix())],
        );
        let displacement_texture =
            allocate_simple_texture(device, queue, &displacement_content, false);
        let color_texture = allocate_simple_texture(device, queue, &color_content, false);
        let decal_layer_content = TextureContent::black(size);
        let decal_layer_texture =
            allocate_simple_texture(device, queue, &decal_layer_content, false);

        let displacement_view =
            displacement_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let color_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Heightmap color texture sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        let decal_view = decal_layer_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::get_or_create_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&displacement_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&decal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&color_sampler),
                },
            ],
            label: Some("HeightMap bindgroup"),
        });
        HeightMap {
            name,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indexes,
            displacement_texture,
            displacement_content,
            color_texture,
            color_content,
            decal_layer_content,
            decal_layer_texture,
            needs_decal_update: false,
            needs_color_displacement_update: false,
            transform,
            bind_group,
            size,
        }
    }

    pub fn to_serializable(&self) -> SerilizableHeightMap {
        let displacement_buffer = self.displacement_content.bytes.clone().into_owned();
        let color_buffer = self.color_content.bytes.clone().into_owned();
        SerilizableHeightMap {
            size: self.size,
            name: self.name.clone(),
            displacement_buffer,
            color_buffer,
            transform: self.transform,
        }
    }

    pub fn update_heightmap_data(&mut self, queue: &wgpu::Queue) {
        if self.needs_color_displacement_update {
            update_texture_data(
                &self.displacement_content,
                &self.displacement_texture,
                queue,
            );
            update_texture_data(&self.color_content, &self.color_texture, queue);
            self.needs_color_displacement_update = false;
        }

        if self.needs_decal_update {
            update_texture_data(&self.decal_layer_content, &self.decal_layer_texture, queue);
            self.needs_decal_update = false;
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn get_size(&self) -> u32 {
        self.size
    }

    pub fn get_transform(&self) -> &Transform {
        &self.transform
    }

    pub fn get_displacement_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_color_displacement_update = true;
        (
            self.displacement_content.stride,
            self.displacement_content.bytes.to_mut(),
        )
    }

    pub fn get_color_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_color_displacement_update = true;
        (self.color_content.stride, self.color_content.bytes.to_mut())
    }

    pub fn get_decal_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_decal_update = true;
        (
            self.decal_layer_content.stride,
            self.decal_layer_content.bytes.to_mut(),
        )
    }

    pub fn get_or_create_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(move || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                ],
                label: Some("HeightMap bind group layout"),
            })
        })
    }
}

impl AssetLoader for HeightMap<'_> {
    fn load(
        path: &std::path::Path,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<Self> {
        let map_file = std::fs::File::open(path)?;
        let seriliziable_map = bincode::deserialize_from(map_file)
            .with_context(|| "HeightMap file is invalid or from older map editor version")?;
        Ok(HeightMap::from_serialized(device, queue, seriliziable_map))
    }

    fn extensions() -> &'static [&'static str] {
        &["map"]
    }
}

pub struct HeightMapPass {
    render_pipeline: wgpu::RenderPipeline,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl HeightMapPass {
    pub fn new(
        device: &wgpu::Device,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> HeightMapPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Heightmap shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/heightmap.wgsl"))),
            flags: wgpu::ShaderFlags::VALIDATION,
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("HeightMap pipeline layout"),
                bind_group_layouts: &[
                    &HeightMap::get_or_create_layout(device),
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
                buffers: &[MapVertex::get_descriptor(), InstanceData::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[wgpu::TextureFormat::Bgra8UnormSrgb.into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
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

        HeightMapPass {
            render_pipeline,
            command_sender,
        }
    }
}

#[system]
pub fn update(#[resource] queue: &wgpu::Queue, #[resource] height_map: &mut HeightMap) {
    height_map.update_heightmap_data(queue);
}

#[system]
pub fn draw(
    #[resource] pass: &HeightMapPass,
    #[resource] current_frame: &wgpu::SwapChainTexture,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] camera: &Camera,
    #[resource] height_map: &HeightMap,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("HeightMap pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("HeightMap render pass"),
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
    render_pass.push_debug_group("HeightMap pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_vertex_buffer(0, height_map.vertex_buffer.slice(..));
    render_pass.set_index_buffer(height_map.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.set_vertex_buffer(1, height_map.instance_buffer.slice(..));
    render_pass.set_bind_group(0, &height_map.bind_group, &[]);
    render_pass.set_bind_group(1, &camera.bind_group(), &[]);
    render_pass.draw_indexed(0..height_map.num_indexes, 0, 0..1);
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}
