use super::{
    camera::Camera,
    common::DEPTH_FORMAT,
    gltf::{GltfModel, InstanceData, MeshVertex, INSTANCE_BUFFER_LEN},
    vertex_buffers::{MutableVertexData, VertexBuffer},
};
use crate::assets::*;
use crate::components::{Selectable, Transform};
use crossbeam_channel::Sender;
use glam::{Mat4, Vec3};
use legion::{world::SubWorld, *};
use wgpu::include_spirv;

use super::common::DepthTexture;

#[system]
#[allow(clippy::clippy::too_many_arguments)]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &SelectionPass,
    #[resource] queue: &wgpu::Queue,
    #[resource] asset_storage: &Assets<GltfModel>,
    #[resource] depth_texture: &DepthTexture,
    #[resource] device: &wgpu::Device,
    #[resource] current_frame: &wgpu::SwapChainTexture,
    #[resource] camera: &Camera,
    query: &mut Query<(&Transform, &Selectable, &Handle<GltfModel>)>,
) {
    // update selected units instance buffer
    query.par_for_each_chunk(world, |chunk| {
        let (transforms, selectable, _) = chunk.get_components();
        // DON'T USE A VEC HERE FOR GODS SAKE
        let model_matrices = transforms
            .iter()
            .zip(selectable)
            .filter(|(_, selectable)| selectable.is_selected)
            .map(|(trans, _)| {
                InstanceData::new(trans.get_model_matrix() * Mat4::from_scale(Vec3::splat(1.01)))
            })
            .collect::<Vec<InstanceData>>();

        pass.instance_buffer.update(queue, &model_matrices);
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Selection pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Selection render pass"),
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
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: false,
            }),
        }),
    });
    render_pass.push_debug_group("Selection pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &camera.bind_group(), &[]);
    let mut query = <(Read<Transform>, Read<Selectable>, Read<Handle<GltfModel>>)>::query();
    query.for_each_chunk(world, |chunk| {
        let (_, selectable, models) = chunk.get_components();
        if let Some(model) = models.get(0) {
            let model = asset_storage.get(model).unwrap();
            let count = selectable
                .iter()
                .filter(|selectable| selectable.is_selected)
                .count();
            model.draw_with_instance_buffer(
                &mut render_pass,
                &pass.instance_buffer,
                0..count as u32,
            );
        }
    });
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

pub struct SelectionPass {
    render_pipeline: wgpu::RenderPipeline,
    // This should be handled better
    instance_buffer: MutableVertexData<InstanceData>,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl SelectionPass {
    pub fn new(
        device: &wgpu::Device,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> SelectionPass {
        let vs_module = device.create_shader_module(&include_spirv!("shaders/model.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("shaders/flat_color.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Selection pipeline layout"),
                bind_group_layouts: &[Camera::get_or_create_layout(device)],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Selection pipeline"),
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
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState {
                    front: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    back: wgpu::StencilFaceState {
                        compare: wgpu::CompareFunction::NotEqual,
                        fail_op: wgpu::StencilOperation::Keep,
                        depth_fail_op: wgpu::StencilOperation::Keep,
                        pass_op: wgpu::StencilOperation::Replace,
                    },
                    read_mask: 0xFF,
                    write_mask: 0x00, // Disable stencil buffer writes
                },
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        let instance_buffer_len = INSTANCE_BUFFER_LEN / std::mem::size_of::<InstanceData>();
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        SelectionPass {
            render_pipeline,
            command_sender,
            instance_buffer,
        }
    }
}
