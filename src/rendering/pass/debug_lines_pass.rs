use std::borrow::Cow;

use crate::components::Transform;
use crate::engine::FrameTexture;
use crate::rendering::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
    gltf::GltfModel,
    gltf::InstanceData,
    vertex_buffers::{ImmutableVertexBuffer, VertexData},
};
use crate::{
    assets::{Assets, Handle},
    resources::DebugRenderSettings,
};
use crossbeam_channel::Sender;
use fxhash::FxHashMap;
use glam::Vec3;
use legion::*;
use world::SubWorld;

use super::model_pass::ModelPass;

#[derive(Debug, Default)]
pub struct BoundingBoxMap {
    vertex_info_map: FxHashMap<(usize, Handle<GltfModel>), ImmutableVertexBuffer<BoxVert>>,
}

// maybe handle rotation here at some point, currently just using AABB
#[system]
#[read_component(Handle<GltfModel>)]
pub fn update_bounding_boxes(
    world: &SubWorld,
    #[resource] bounding_box_map: &mut BoundingBoxMap,
    #[resource] device: &wgpu::Device,
    #[resource] asset_storage: &Assets<GltfModel>,
) {
    let mut query = <&Handle<GltfModel>>::query().filter(component::<Transform>());
    query.for_each(world, |model_handle| {
        let model = asset_storage.get(model_handle).unwrap();
        for mesh in &model.meshes {
            let key = &(*mesh.index(), *model_handle);
            if !bounding_box_map.vertex_info_map.contains_key(key) {
                let buffer = calc_buffer(&model.min_vertex, &model.max_vertex);
                bounding_box_map
                    .vertex_info_map
                    .insert(*key, VertexData::allocate_immutable_buffer(device, &buffer));
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
#[system]
#[read_component(Handle<GltfModel>)]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &DebugLinesPass,
    #[resource] model_pass: &ModelPass,
    #[resource] bounding_box_map: &BoundingBoxMap,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] current_frame: &FrameTexture,
    #[resource] debug_settings: &DebugRenderSettings,
    #[resource] asset_storage: &mut Assets<GltfModel>,
    #[resource] camera: &Camera,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Debug lines encoder"),
    });

    let pipeline = &pass.render_pipeline;

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Debug lines pass"),
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

    let mut query = <&Handle<GltfModel>>::query().filter(component::<Transform>());

    render_pass.push_debug_group("Debug lines debug group");
    render_pass.set_pipeline(pipeline);
    render_pass.set_bind_group(0, camera.bind_group(), &[]);
    if debug_settings.show_bounding_boxes {
        query.for_each(world, |model_handle| {
            let model = asset_storage.get(model_handle).unwrap();
            for mesh in &model.meshes {
                let key = &(*mesh.index(), *model_handle);
                let buffer = bounding_box_map.vertex_info_map.get(key).unwrap();
                let instance_buffer = model_pass.instance_data().get(key).unwrap();
                render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
                render_pass.set_vertex_buffer(1, buffer.slice(..));
                render_pass.draw(0..24, 0..instance_buffer.size() as u32);
            }
        });
    }
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

fn calc_buffer(min_pos: &Vec3, max_pos: &Vec3) -> Vec<BoxVert> {
    let height = max_pos.y - min_pos.y;
    let depth = max_pos.z - min_pos.z;
    let widht = max_pos.x - min_pos.x;

    vec![
        //Base
        *min_pos,
        Vec3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vec3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        *min_pos,
        //top
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z),
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z),
        // connecting lines
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z),
        Vec3::new(min_pos.x, min_pos.y, min_pos.z),
        Vec3::new(min_pos.x, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x, min_pos.y, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z),
        Vec3::new(min_pos.x + widht, min_pos.y + height, min_pos.z + depth),
        Vec3::new(min_pos.x + widht, min_pos.y, min_pos.z + depth),
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

impl VertexData for BoxVert {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;

    fn attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }]
    }
}

pub struct DebugLinesPass {
    render_pipeline: wgpu::RenderPipeline,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl DebugLinesPass {
    pub fn new(
        device: &wgpu::Device,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> DebugLinesPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Debug lines shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "shaders/debug_lines.wgsl"
            ))),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Debug lines pass pipeline layout"),
                bind_group_layouts: &[Camera::get_or_create_layout(device)],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debuglines pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[InstanceData::descriptor(), BoxVert::descriptor()],
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
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
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
            }),
            multisample: wgpu::MultisampleState::default(),
        });
        DebugLinesPass {
            render_pipeline,
            command_sender,
        }
    }
}
