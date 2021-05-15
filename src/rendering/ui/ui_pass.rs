use crate::{assets::Assets, rendering::texture::*, resources::WindowSize};
use bytemuck::{Pod, Zeroable};
use crossbeam_channel::Sender;
use std::{borrow::Cow, convert::TryInto};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    CommandBuffer, Device,
};

use super::ui_resources::UiTexture;

#[derive(Debug)]
enum BufferType {
    Uniform,
    Vertex,
    Index,
}

// Todo move this functionality into the vertex buffers
struct SizedBuffer {
    buffer: wgpu::Buffer,
    size: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct UniformBuffer {
    screen_size: [f32; 2],
}

pub struct UiPass {
    render_pipeline: wgpu::RenderPipeline,
    index_buffers: Vec<SizedBuffer>,
    vertex_buffers: Vec<SizedBuffer>,
    uniform_buffer: SizedBuffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group: Option<wgpu::BindGroup>,
    texture_version: Option<u64>,
    pub command_sender: Sender<CommandBuffer>,
}

impl UiPass {
    pub fn new(device: &Device, command_sender: Sender<CommandBuffer>) -> UiPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Ui shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/ui.wgsl"))),
            flags: wgpu::ShaderFlags::VALIDATION,
        });
        let uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("ui_uniform_buffer"),
            contents: bytemuck::cast_slice(&[UniformBuffer {
                screen_size: [0.0, 0.0],
            }]),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });
        let uniform_buffer = SizedBuffer {
            buffer: uniform_buffer,
            size: std::mem::size_of::<UniformBuffer>(),
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui_texture_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("egui_uniform_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                ],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer.buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui_pipeline_layout"),
            bind_group_layouts: &[
                &uniform_bind_group_layout,
                UiTexture::get_or_create_layout(device),
            ],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 5 * 4,
                    step_mode: wgpu::InputStepMode::Vertex,
                    // 0: vec2 position
                    // 1: vec2 texture coordinates
                    // 2: uint color
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::Zero,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        });

        UiPass {
            render_pipeline,
            vertex_buffers: Vec::with_capacity(64),
            index_buffers: Vec::with_capacity(64),
            uniform_buffer,
            uniform_bind_group,
            texture_version: None,
            texture_bind_group: None,
            command_sender,
        }
    }
    pub fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_attachment: &wgpu::TextureView,
        paint_jobs: &[egui::paint::ClippedMesh],
        ui_textures: &Assets<UiTexture>,
        screen_descriptor: &WindowSize,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui_render_pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: color_attachment,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        pass.push_debug_group("ui_pass");
        pass.set_pipeline(&self.render_pipeline);

        pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        let scale_factor = screen_descriptor.scale_factor;
        let physical_width = screen_descriptor.physical_width;
        let physical_height = screen_descriptor.physical_height;

        for ((egui::ClippedMesh(clip_rect, mesh), vertex_buffer), index_buffer) in paint_jobs
            .iter()
            .zip(self.vertex_buffers.iter())
            .zip(self.index_buffers.iter())
        {
            // TODO: feels like some of these checks can be removed
            // Transform clip rect to physical pixels.
            let clip_min_x = scale_factor * clip_rect.min.x;
            let clip_min_y = scale_factor * clip_rect.min.y;
            let clip_max_x = scale_factor * clip_rect.max.x;
            let clip_max_y = scale_factor * clip_rect.max.y;

            // Make sure clip rect can fit within an `u32`.
            let clip_min_x = clip_min_x.clamp(0.0, physical_width as f32);
            let clip_min_y = clip_min_y.clamp(0.0, physical_height as f32);
            let clip_max_x = clip_max_x.clamp(clip_min_x, physical_width as f32);
            let clip_max_y = clip_max_y.clamp(clip_min_y, physical_height as f32);

            let clip_min_x = clip_min_x.round() as u32;
            let clip_min_y = clip_min_y.round() as u32;
            let clip_max_x = clip_max_x.round() as u32;
            let clip_max_y = clip_max_y.round() as u32;

            let width = (clip_max_x - clip_min_x).max(1);
            let height = (clip_max_y - clip_min_y).max(1);

            // clip scissor rectangle to target size
            let x = clip_min_x.min(physical_width);
            let y = clip_min_y.min(physical_height);
            let width = width.min(physical_width - x);
            let height = height.min(physical_height - y);

            // skip rendering with zero-sized clip areas
            if width == 0 || height == 0 {
                continue;
            }

            pass.set_scissor_rect(x, y, width, height);

            pass.set_bind_group(
                1,
                self.get_texture_bind_group(mesh.texture_id, ui_textures),
                &[],
            );

            pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..));
            pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
        }

        pass.pop_debug_group();
    }

    fn get_texture_bind_group<'a>(
        &'a self,
        texture_id: egui::TextureId,
        ui_textures: &'a Assets<UiTexture<'a>>,
    ) -> &'a wgpu::BindGroup {
        match texture_id {
            egui::TextureId::Egui => self
                .texture_bind_group
                .as_ref()
                .expect("egui texture was not set before the first draw"),
            egui::TextureId::User(_) => {
                let handle = texture_id
                    .try_into()
                    .expect("Failed to convert ui texture to asset handle");
                ui_textures
                    .get(&handle)
                    .map(|tex| &tex.bind_group)
                    .unwrap_or_else(|| {
                        panic!("user texture {} not found in assets", handle.get_id())
                    })
            }
        }
    }

    /// Updates the texture used by egui for the fonts etc. Should be called before `execute()`.
    pub fn update_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        egui_texture: &egui::Texture,
    ) {
        // Don't update the texture if it hasn't changed.
        if self.texture_version == Some(egui_texture.version) {
            return;
        }
        let content = TextureContent::from(egui_texture);
        let ui_texture = UiTexture::new(device, queue, "egui", content);
        self.texture_version = Some(egui_texture.version);
        self.texture_bind_group = Some(ui_texture.bind_group);
    }

    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paint_jobs: &[egui::paint::ClippedMesh],
        screen_descriptor: &WindowSize,
    ) {
        let index_size = self.index_buffers.len();
        let vertex_size = self.vertex_buffers.len();

        let (logical_width, logical_height) = screen_descriptor.logical_size();

        self.update_buffer(
            device,
            queue,
            BufferType::Uniform,
            0,
            bytemuck::cast_slice(&[UniformBuffer {
                screen_size: [logical_width as f32, logical_height as f32],
            }]),
        );

        for (i, egui::ClippedMesh(_, mesh)) in paint_jobs.iter().enumerate() {
            let data: &[u8] = bytemuck::cast_slice(&mesh.indices);
            if i < index_size {
                self.update_buffer(device, queue, BufferType::Index, i, data)
            } else {
                let buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("egui_index_buffer"),
                    contents: data,
                    usage: wgpu::BufferUsage::INDEX | wgpu::BufferUsage::COPY_DST,
                });
                self.index_buffers.push(SizedBuffer {
                    buffer,
                    size: data.len(),
                });
            }

            let data: &[u8] = as_byte_slice(&mesh.vertices);
            if i < vertex_size {
                self.update_buffer(device, queue, BufferType::Vertex, i, data)
            } else {
                let buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("egui_vertex_buffer"),
                    contents: data,
                    usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                });

                self.vertex_buffers.push(SizedBuffer {
                    buffer,
                    size: data.len(),
                });
            }
        }
    }

    // Updates the buffers used by egui. Will properly re-size the buffers if needed.
    fn update_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer_type: BufferType,
        index: usize,
        data: &[u8],
    ) {
        let (buffer, storage, name) = match buffer_type {
            BufferType::Index => (
                &mut self.index_buffers[index],
                wgpu::BufferUsage::INDEX,
                "index",
            ),
            BufferType::Vertex => (
                &mut self.vertex_buffers[index],
                wgpu::BufferUsage::VERTEX,
                "vertex",
            ),
            BufferType::Uniform => (
                &mut self.uniform_buffer,
                wgpu::BufferUsage::UNIFORM,
                "uniform",
            ),
        };

        if data.len() > buffer.size {
            buffer.size = data.len();
            buffer.buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some(format!("egui_{}_buffer", name).as_str()),
                contents: bytemuck::cast_slice(data),
                usage: storage | wgpu::BufferUsage::COPY_DST,
            });
        } else {
            queue.write_buffer(&buffer.buffer, 0, data);
        }
    }
}

fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    let len = slice.len() * std::mem::size_of::<T>();
    let ptr = slice.as_ptr() as *const u8;
    unsafe { std::slice::from_raw_parts(ptr, len) }
}
