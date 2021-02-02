use components::Transform;
use crevice::std140::AsStd140;
use crevice::std140::Std140;
use crossbeam_channel::Sender;
use legion::{world::SubWorld, *};
use wgpu::{
    include_spirv, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferAddress, BufferDescriptor,
    BufferUsage, CommandEncoderDescriptor, CompareFunction, CullMode, Device, Extent3d,
    PipelineLayoutDescriptor, Queue, RenderPassColorAttachmentDescriptor,
    RenderPassDepthStencilAttachmentDescriptor, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, ShaderStage, SwapChainDescriptor, SwapChainTexture, TextureDimension,
    TextureViewDescriptor,
};

use crate::assets::{Assets, Handle};
use crate::components;

use super::{
    camera::{Camera, CameraUniform},
    model::{DrawModel, InstanceData, MeshVertex, Model},
    simple_texture::SimpleTexture,
    texture::TextureShaderLayout,
    vertex_buffers::VertexBuffer,
};

#[system]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn update(
    world: &SubWorld,
    #[resource] queue: &Queue,
    #[resource] asset_storage: &Assets<Model>,
) {
    let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();

    query.par_for_each_chunk(world, |chunk| {
        let transforms = chunk.component_slice::<Transform>().unwrap();
        let model = &chunk.component_slice::<Handle<Model>>().unwrap()[0];
        // DON'T USE A VEC HERE FOR GODS SAKE
        let model_matrices = transforms
            .iter()
            .map(|trans| InstanceData::new(trans.get_model_matrix()))
            .collect::<Vec<InstanceData>>();

        let instance_buffer = &asset_storage.get(model).unwrap().instance_buffer;
        instance_buffer.update(queue, &model_matrices);
    });
}

#[system]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &ModelPass,
    #[resource] camera: &Camera,
    #[resource] queue: &Queue,
    #[resource] asset_storage: &Assets<Model>,
    #[resource] device: &Device,
    #[resource] current_frame: &SwapChainTexture,
) {
    let camera_uniform: CameraUniform = camera.clone().into();
    queue.write_buffer(
        &pass.camera_buffer,
        0,
        camera_uniform.as_std140().as_bytes(),
    );
    // UPDATE CAMERA UNIFORM HERE!
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Model pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("Model render pass"),
        color_attachments: &[RenderPassColorAttachmentDescriptor {
            attachment: &current_frame.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 1.0,
                }),
                store: true,
            },
        }],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachmentDescriptor {
            attachment: &pass.depth_texture_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        }),
    });
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &pass.camera_bind_group, &[]);
    let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();
    query.for_each_chunk(world, |chunk| {
        let transforms = chunk.component_slice::<Transform>().unwrap();
        let model = &chunk.component_slice::<Handle<Model>>().unwrap()[0];
        let model = asset_storage.get(model).unwrap();
        render_pass.draw_model_instanced(model, 0..transforms.len() as u32)
    });
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

pub struct ModelPass {
    render_pipeline: RenderPipeline,
    depth_texture: wgpu::Texture,
    depth_texture_view: wgpu::TextureView,
    // temporary
    camera_bind_group: BindGroup,
    camera_buffer: Buffer,
    command_sender: Sender<wgpu::CommandBuffer>,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub fn create_depth_texture(
    device: &wgpu::Device,
    sc_desc: &wgpu::SwapChainDescriptor,
) -> wgpu::Texture {
    let desc = wgpu::TextureDescriptor {
        label: None,
        size: Extent3d {
            width: sc_desc.width,
            height: sc_desc.height,
            depth: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
    };
    device.create_texture(&desc)
}

impl ModelPass {
    pub fn new(
        device: &Device,
        sc_desc: &SwapChainDescriptor,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> ModelPass {
        let vs_module = device.create_shader_module(&include_spirv!("shaders/model.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("shaders/model.frag.spv"));
        let depth_texture = create_depth_texture(device, sc_desc);
        let depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera buffer"),
            size: std::mem::size_of::<<CameraUniform as AsStd140>::Std140Type>() as u64,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Camera layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStage::VERTEX,
                    ty: BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Camera bindgroup"),
            layout: &camera_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer {
                    buffer: &camera_buffer,
                    offset: 0 as BufferAddress,
                    size: None,
                },
            }],
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Model pipeline layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                SimpleTexture::get_layout(&device),
                SimpleTexture::get_layout(&device),
            ], // Camera
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[MeshVertex::get_descriptor(), InstanceData::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[sc_desc.format.into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: CullMode::Back,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        ModelPass {
            render_pipeline,
            camera_bind_group,
            command_sender,
            camera_buffer,
            depth_texture,
            depth_texture_view,
        }
    }

    pub fn handle_resize(&mut self, device: &Device, sc_desc: &SwapChainDescriptor) {
        self.depth_texture = create_depth_texture(&device, &sc_desc);
        self.depth_texture_view = self
            .depth_texture
            .create_view(&TextureViewDescriptor::default());
    }
}
