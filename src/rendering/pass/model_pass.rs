use std::borrow::Cow;

use crate::assets::{Assets, Handle};
use crate::components::Transform;
use crate::engine::FrameTexture;
use crossbeam_channel::Sender;
use legion::{world::SubWorld, *};

use crate::rendering::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
    gltf::GltfModel,
    gltf::PbrMaterial,
    gltf::{InstanceData, MeshVertex},
    lights::LightUniformBuffer,
    vertex_buffers::VertexBuffer,
};

#[system]
pub fn update(
    world: &SubWorld,
    #[resource] queue: &wgpu::Queue,
    #[resource] asset_storage: &Assets<GltfModel>,
    query: &mut Query<(&Transform, &Handle<GltfModel>)>,
) {
    // TODO: Change this to something that actually works for different models
    // bumpallocation while retaining multithreading would be nice
    query.par_for_each_chunk(world, |chunk| {
        let (transforms, models) = chunk.get_components();
        // HACK
        if let Some(model) = models.get(0) {
            // DON'T USE A VEC HERE FOR GODS SAKE
            let model_matrices = transforms
                .iter()
                .map(|trans| InstanceData::new(trans))
                .collect::<Vec<InstanceData>>();
            let instance_buffer = &asset_storage.get(model).unwrap().instance_buffer;
            instance_buffer.update(queue, &model_matrices);
        }
    });
}

#[allow(clippy::too_many_arguments)]
#[system]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &ModelPass,
    #[resource] asset_storage: &Assets<GltfModel>,
    #[resource] depth_texture: &DepthTexture,
    #[resource] device: &wgpu::Device,
    #[resource] light_uniform: &LightUniformBuffer,
    #[resource] current_frame: &FrameTexture,
    #[resource] camera: &Camera,
    query: &mut Query<(&Transform, &Handle<GltfModel>)>,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Model pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Model render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view: &current_frame.view,
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
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_texture.view,
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
    render_pass.set_bind_group(0, camera.bind_group(), &[]);
    render_pass.set_bind_group(2, &light_uniform.bind_group, &[]);
    query.for_each_chunk(world, |chunk| {
        let (transforms, models) = chunk.get_components();
        if let Some(model) = models.get(0) {
            let model = asset_storage.get(model).unwrap();
            model.draw_instanced(&mut render_pass, 0..transforms.len() as u32);
        }
    });
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

pub struct ModelPass {
    render_pipeline: wgpu::RenderPipeline,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl ModelPass {
    pub fn new(device: &wgpu::Device, command_sender: Sender<wgpu::CommandBuffer>) -> ModelPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Model shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/model.wgsl"))),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Model pipeline layout"),
                bind_group_layouts: &[
                    Camera::get_or_create_layout(device),
                    PbrMaterial::get_or_create_layout(device),
                    LightUniformBuffer::get_or_create_layout(device),
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Model pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[MeshVertex::get_descriptor(), InstanceData::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[wgpu::TextureFormat::Bgra8UnormSrgb.into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
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
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        ModelPass {
            render_pipeline,
            command_sender,
        }
    }
}
