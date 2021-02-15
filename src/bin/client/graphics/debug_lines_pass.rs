use super::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
    model::{InstanceData, Model},
    vertex_buffers::{ImmutableVertexData, VertexBuffer, VertexBufferData},
};
use unnamed_rts::components::Transform;
use crate::{
    client_systems::DebugMenueSettings,
    assets::{Assets, Handle},
};
use crossbeam_channel::Sender;
use legion::*;
use nalgebra::Vector3;
use std::collections::HashMap;
use wgpu::{include_spirv, SwapChainTexture};
use world::SubWorld;

#[derive(Debug, Default)]
// This should be refactored to be component based instead of using this resource
pub struct BoundingBoxMap {
    vertex_info_map: HashMap<Handle<Model>, ImmutableVertexData<BoxVert>>,
}

#[system]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
// maybe handle rotation here at some point, currently just using AABB
pub fn update_bounding_boxes(
    world: &SubWorld,
    #[resource] bounding_box_map: &mut BoundingBoxMap,
    #[resource] device: &wgpu::Device,
    #[resource] asset_storage: &Assets<Model>,
) {
    let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();
    query.for_each_chunk(world, |chunk| {
        let model_handle = &chunk.component_slice::<Handle<Model>>().unwrap()[0];
        let model = asset_storage.get(&model_handle).unwrap();
        if !bounding_box_map.vertex_info_map.contains_key(&model_handle) {
            let buffer = calc_buffer(&model.min_position, &model.max_position);
            bounding_box_map.vertex_info_map.insert(
                model_handle.clone(),
                VertexBuffer::allocate_immutable_buffer(&device, &buffer),
            );
        }
    });
}

#[allow(clippy::clippy::too_many_arguments)]
#[system]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn draw(
    world: &SubWorld,
    #[state] pass: &DebugLinesPass,
    #[resource] bounding_box_map: &BoundingBoxMap,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] asset_storage: &Assets<Model>,
    #[resource] current_frame: &SwapChainTexture,
    #[resource] debug_settings: &DebugMenueSettings,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Debug lines encoder"),
    });

    let pipeline = &pass.render_pipeline;

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Debug lines pass"),
        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
            attachment: &current_frame.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        }],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
            attachment: &depth_texture.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            }),
            stencil_ops: None,
        }),
    });
    render_pass.push_debug_group("Debug lines debug group");
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, &pass.camera_bind_group, &[]);
    if debug_settings.show_bounding_boxes {
        let mut query = <(Read<Transform>, Read<Handle<Model>>)>::query();

        query.for_each_chunk(world, |chunk| {
            let transforms = chunk.component_slice::<Transform>().unwrap();
            let model_handle = &chunk.component_slice::<Handle<Model>>().unwrap()[0];
            let model = asset_storage.get(&model_handle).unwrap();
            let buffer = bounding_box_map.vertex_info_map.get(&model_handle).unwrap();
            render_pass.set_vertex_buffer(0, model.instance_buffer.slice(..));
            render_pass.set_vertex_buffer(1, buffer.slice(..));
            render_pass.draw(0..24, 0..transforms.len() as u32);
        });
    }
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

fn calc_buffer(min_pos: &Vector3<f32>, max_pos: &Vector3<f32>) -> Vec<BoxVert> {
    let height = max_pos.y - min_pos.y;
    let depth = max_pos.z - min_pos.z;
    let widht = max_pos.x - min_pos.x;

    vec![
        //Base
        *min_pos,
        Vector3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vector3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        *min_pos,
        //top
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z),
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z),
        // connecting lines
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z),
        Vector3::new(min_pos.x, min_pos.y, min_pos.z),
        Vector3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        Vector3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vector3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
    ]
    .iter()
    .map(|vec| BoxVert {
        position: [vec.x, vec.y, vec.z],
    })
    .collect::<Vec<_>>()
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BoxVert {
    position: [f32; 3],
}

impl VertexBuffer for BoxVert {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float3,
            offset: 0,
            shader_location: 0,
        }]
    }
}

pub struct DebugLinesPass {
    render_pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl DebugLinesPass {
    pub fn new(
        device: &wgpu::Device,
        camera: &Camera,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> DebugLinesPass {
        let vs_module =
            device.create_shader_module(&include_spirv!("shaders/debug_lines.vert.spv"));
        let fs_module =
            device.create_shader_module(&include_spirv!("shaders/debug_lines.frag.spv"));

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
                label: Some("Debug lines pass pipeline layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debuglines pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[InstanceData::get_descriptor(), BoxVert::get_descriptor()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    color_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: wgpu::CullMode::None,
                polygon_mode: wgpu::PolygonMode::Line,
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        DebugLinesPass {
            camera_bind_group,
            command_sender,
            render_pipeline,
        }
    }
}
