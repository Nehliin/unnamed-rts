use std::borrow::Cow;

use crate::assets::*;
use crate::components::{Selectable, Transform};
use crate::engine::FrameTexture;
use crate::rendering::{
    camera::Camera,
    common::DEPTH_FORMAT,
    gltf::{GltfModel, InstanceData, MeshVertex},
    mesh_instance_buffer_cache::MeshInstanceBufferCache,
    vertex_buffers::VertexData,
};
use crossbeam_channel::Sender;
use glam::{Affine3A, Vec3};
use legion::{world::SubWorld, *};

use crate::rendering::common::DepthTexture;

#[system]
#[allow(clippy::too_many_arguments)]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &mut SelectionPass,
    #[resource] queue: &wgpu::Queue,
    #[resource] asset_storage: &mut Assets<GltfModel>,
    #[resource] depth_texture: &DepthTexture,
    #[resource] device: &wgpu::Device,
    #[resource] current_frame: &FrameTexture,
    #[resource] camera: &Camera,
    query: &mut Query<(&Transform, &Selectable, &Handle<GltfModel>)>,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Selection pass encoder"),
    });
    let mut instance_data = std::mem::take(&mut pass.instance_data);
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
    render_pass.set_bind_group(0, camera.bind_group(), &[]);

    instance_data.evict_stale(asset_storage);
    query
        .iter(world)
        .filter(|(_, selectable, _)| selectable.is_selected)
        .for_each(|(transform, _, model_handle)| {
            let model = asset_storage.get(model_handle).unwrap();
            instance_data.put(device, model_handle, model, |mesh| Transform {
                matrix: transform.matrix
                    * *mesh.local_transform()
                    * Affine3A::from_scale(Vec3::splat(1.01)),
            })
        });
    for (mesh, buffer) in instance_data.iter_mut(asset_storage) {
        buffer.update(device, queue);
        mesh.draw_with_instance_buffer(&mut render_pass, buffer);
    }

    render_pass.pop_debug_group();
    drop(render_pass);
    pass.instance_data = instance_data;
    pass.command_sender.send(encoder.finish()).unwrap();
}

pub struct SelectionPass {
    render_pipeline: wgpu::RenderPipeline,
    instance_data: MeshInstanceBufferCache,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl SelectionPass {
    pub fn new(
        device: &wgpu::Device,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> SelectionPass {
        // TODO: Share this with the modle pass
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Selection(model) shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/model.wgsl"))),
        });

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
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[MeshVertex::descriptor(), InstanceData::descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "flat_main",
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
        SelectionPass {
            render_pipeline,
            instance_data: Default::default(),
            command_sender,
        }
    }
}
