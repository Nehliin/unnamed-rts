use bytemuck::{Pod, Zeroable};
use crossbeam_channel::Sender;
use unnamed_rts::resources::WindowSize;
use wgpu::{
    include_spirv,
    util::{BufferInitDescriptor, DeviceExt},
    CommandBuffer, Device,
};


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
    texture_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group: Option<wgpu::BindGroup>,
    texture_version: Option<u64>,
    _next_user_texture_id: u64,
    pending_user_textures: Vec<(u64, egui::Texture)>,
    user_textures: Vec<Option<wgpu::BindGroup>>,
    pub command_sender: Sender<CommandBuffer>,
}

impl UiPass {
    pub fn new(device: &Device, command_sender: Sender<CommandBuffer>) -> UiPass {
        let vs_module = device.create_shader_module(&include_spirv!("../shaders/ui.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("../shaders/ui.frag.spv"));

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
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_buffer.buffer,
                        offset: 0,
                        size: None,
                    },
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("egui_texture_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 5 * 4,
                    step_mode: wgpu::InputStepMode::Vertex,
                    // 0: vec2 position
                    // 1: vec2 texture coordinates
                    // 2: uint color
                    attributes: &wgpu::vertex_attr_array![0 => Float2, 1 => Float2, 2 => Uint],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    color_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
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
            texture_bind_group_layout,
            texture_version: None,
            texture_bind_group: None,
            _next_user_texture_id: 0,
            pending_user_textures: Vec::new(),
            user_textures: Vec::new(),
            command_sender,
        }
    }
    pub fn execute(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_attachment: &wgpu::TextureView,
        paint_jobs: &[egui::paint::PaintJob],
        screen_descriptor: &WindowSize,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui_render_pass"),
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: color_attachment,
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

        for (((clip_rect, triangles), vertex_buffer), index_buffer) in paint_jobs
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
            let clip_min_x = egui::clamp(clip_min_x, 0.0..=physical_width as f32);
            let clip_min_y = egui::clamp(clip_min_y, 0.0..=physical_height as f32);
            let clip_max_x = egui::clamp(clip_max_x, clip_min_x..=physical_width as f32);
            let clip_max_y = egui::clamp(clip_max_y, clip_min_y..=physical_height as f32);

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

            pass.set_bind_group(1, self.get_texture_bind_group(triangles.texture_id), &[]);

            pass.set_index_buffer(index_buffer.buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_vertex_buffer(0, vertex_buffer.buffer.slice(..));
            pass.draw_indexed(0..triangles.indices.len() as u32, 0, 0..1);
        }

        pass.pop_debug_group();
    }

    fn get_texture_bind_group(&self, texture_id: egui::TextureId) -> &wgpu::BindGroup {
        match texture_id {
            egui::TextureId::Egui => self
                .texture_bind_group
                .as_ref()
                .expect("egui texture was not set before the first draw"),
            egui::TextureId::User(id) => {
                let id = id as usize;
                self.user_textures
                    .get(id)
                    .unwrap_or_else(|| panic!("user texture {} not found", id))
                    .as_ref()
                    .unwrap_or_else(|| panic!("user texture {} freed", id))
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
        // we need to convert the texture into rgba format
        let mut pixels = Vec::new();
        pixels.reserve(4 * pixels.len());
        for &alpha in egui_texture.pixels.iter() {
            pixels.extend(egui::Color32::from_white_alpha(alpha).to_array().iter());
        }
        let egui_texture = egui::Texture {
            version: egui_texture.version,
            width: egui_texture.width,
            height: egui_texture.height,
            pixels,
        };
        let bind_group = self.egui_texture_to_wgpu(device, queue, &egui_texture, "egui");

        self.texture_version = Some(egui_texture.version);
        self.texture_bind_group = Some(bind_group);
    }

    /// Updates the user textures that the app allocated. Should be called before `execute()`.
    pub fn update_user_textures(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let pending_user_textures = std::mem::take(&mut self.pending_user_textures);
        for (id, texture) in pending_user_textures {
            let bind_group = self.egui_texture_to_wgpu(
                device,
                queue,
                &texture,
                format!("user_texture{}", id).as_str(),
            );
            self.user_textures.push(Some(bind_group));
        }
    }

    fn egui_texture_to_wgpu(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        egui_texture: &egui::Texture,
        label: &str,
    ) -> wgpu::BindGroup {
        let size = wgpu::Extent3d {
            width: egui_texture.width as u32,
            height: egui_texture.height as u32,
            depth: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(format!("{}_texture", label).as_str()),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        queue.write_texture(
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            egui_texture.pixels.as_slice(),
            wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: (egui_texture.pixels.len() / egui_texture.height) as u32,
                rows_per_image: egui_texture.height as u32,
            },
            size,
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(format!("{}_texture_bind_group", label).as_str()),
            layout: &self.texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            }],
        });

        bind_group
    }

    /// Uploads the uniform, vertex and index data used by the render pass. Should be called before `execute()`.
    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paint_jobs: &[egui::paint::PaintJob],
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

        for (i, (_, triangles)) in paint_jobs.iter().enumerate() {
            let data: &[u8] = bytemuck::cast_slice(&triangles.indices);
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

            let data: &[u8] = as_byte_slice(&triangles.vertices);
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

    /// Updates the buffers used by egui. Will properly re-size the buffers if needed.
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
