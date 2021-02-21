use crossbeam_channel::Sender;
use legion::{world::SubWorld, *};
use unnamed_rts::components::{Transform};
use wgpu::include_spirv;

use crate::assets::{Assets, Handle};

use super::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
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
    #[resource] queue: &wgpu::Queue,
    #[resource] asset_storage: &Assets<Model>,
) {
    let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();

    query.par_for_each_chunk(world, |chunk| {
        let (transforms, models) = chunk.get_components();
        if let Some(model) = models.get(0) {
            // DON'T USE A VEC HERE FOR GODS SAKE
            let model_matrices = transforms
                .iter()
                .map(|trans| InstanceData::new(trans.get_model_matrix()))
                .collect::<Vec<InstanceData>>();
            let instance_buffer = &asset_storage.get(model).unwrap().instance_buffer;
            instance_buffer.update(queue, &model_matrices);
        }
    });
}

#[system]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn draw(
    world: &SubWorld,
    #[state] pass: &ModelPass,
    #[resource] asset_storage: &Assets<Model>,
    #[resource] depth_texture: &DepthTexture,
    #[resource] device: &wgpu::Device,
    #[resource] current_frame: &wgpu::SwapChainTexture,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Model pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Model render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
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
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
            attachment: &depth_texture.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1),
                store: true,
            }),
        }),
    });
    render_pass.push_debug_group("Model pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &pass.camera_bind_group, &[]);
    let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();
    query.for_each_chunk(world, |chunk| {
        let (transforms, models) = chunk.get_components();
        if let Some(model) = models.get(0) {
            let model = asset_storage.get(model).unwrap();
            render_pass.draw_model_instanced(model, 0..transforms.len() as u32);
        }
    });
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

pub struct ModelPass {
    render_pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl ModelPass {
    pub fn new(
        device: &wgpu::Device,
        camera: &Camera,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> ModelPass {
        let vs_module = device.create_shader_module(&include_spirv!("shaders/model.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("shaders/model.frag.spv"));
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: Camera::get_binding_type(),
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bindgroup"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.get_binding_resource(),
            }],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Model pipeline layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    SimpleTexture::get_layout(&device),
                    SimpleTexture::get_layout(&device),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Model pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[MeshVertex::get_descriptor(), InstanceData::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[wgpu::TextureFormat::Bgra8UnormSrgb.into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: wgpu::CullMode::Back,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::Always,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    read_mask: 0x00,
                    write_mask: 0xFF,
                },
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        ModelPass {
            render_pipeline,
            camera_bind_group,
            command_sender,
        }
    }
}
