use super::vertex_buffers::{ImmutableVertexData, VertexBuffer, VertexBufferData};
use super::{camera::Camera, common::DepthTexture};
use super::{
    common::{InstanceData, DEPTH_FORMAT},
    vertex_buffers::MutableVertexData,
};
use crate::components::Transform;
use crossbeam_channel::Sender;
use glam::Vec3;
use legion::{world::SubWorld, *};
use navmesh::NavMesh;
use wgpu::{
    include_spirv,
    util::{BufferInitDescriptor, DeviceExt},
};

pub struct DrawableNavMesh {
    _mesh: NavMesh,
    index_buffer: wgpu::Buffer,
    vertex_buffer: ImmutableVertexData<NavMeshVert>,
    num_indexes: u32,
}

impl DrawableNavMesh {
    pub fn new(device: &wgpu::Device, mesh: NavMesh) -> DrawableNavMesh {
        let num_indexes = (mesh.triangles().len() * 3) as u32;
        let indices = mesh
            .triangles()
            .iter()
            .map(|triangle| {
                std::array::IntoIter::new([triangle.first, triangle.second, triangle.third])
            })
            .flatten()
            .collect::<Vec<u32>>();
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("NavMesh Index buffer"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indices),
        });
        // Try to just cast this instead, it should be identical because it's repr(C)
        let verticies = mesh
            .vertices()
            .iter()
            .map(|vert| NavMeshVert {
                position: Vec3::new(vert.x, vert.y, vert.z),
            })
            .collect::<Vec<_>>();
        let vertex_buffer = VertexBuffer::allocate_immutable_buffer(device, &verticies);
        DrawableNavMesh {
            num_indexes,
            vertex_buffer,
            index_buffer,
            _mesh: mesh,
        }
    }
}

#[system]
#[allow(clippy::too_many_arguments)]
pub fn draw(
    world: &SubWorld,
    #[resource] pass: &NavMeshPass,
    #[resource] depth_texture: &DepthTexture,
    #[resource] current_frame: &wgpu::SwapChainTexture,
    #[resource] device: &wgpu::Device,
    #[resource] queue: &wgpu::Queue,
    #[resource] camera: &Camera,
    query: &mut Query<(&Transform, &DrawableNavMesh)>,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("NavMesh pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("NavMesh render pass"),
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
    render_pass.push_debug_group("NavMesh pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &camera.bind_group(), &[]);
    query.for_each(world, |(transfrom, navmesh)| {
        // only storage for a single instance data entry in the buffer
        // because the navmesh can't be drawn instanced since each navmesh have
        // unique vertex buffer
        pass.instance_buffer
            .update(queue, &[InstanceData::new(transfrom.get_model_matrix())]);
        render_pass.set_vertex_buffer(0, pass.instance_buffer.slice(..));
        render_pass.set_vertex_buffer(1, navmesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(navmesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..navmesh.num_indexes, 0, 0..1);
    });
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct NavMeshVert {
    position: Vec3,
}

impl VertexBuffer for NavMeshVert {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;
    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float3,
            offset: 0,
            shader_location: 0,
        }]
    }
}

pub struct NavMeshPass {
    render_pipeline: wgpu::RenderPipeline,
    command_sender: Sender<wgpu::CommandBuffer>,
    // TODO: fix instance buffer handling overall, this isn't great
    instance_buffer: MutableVertexData<InstanceData>,
}

impl NavMeshPass {
    pub fn new(device: &wgpu::Device, command_sender: Sender<wgpu::CommandBuffer>) -> NavMeshPass {
        // Maybe reuse the debug lines shaders if the colors can become configurable
        let vs_module = device.create_shader_module(&include_spirv!("shaders/navmesh.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("shaders/navmesh.frag.spv"));

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("NavMesh pass pipeline layout"),
                bind_group_layouts: &[Camera::get_or_create_layout(device)],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NavMesh pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[
                    InstanceData::get_descriptor(),
                    NavMeshVert::get_descriptor(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    color_blend: wgpu::BlendState::REPLACE,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: wgpu::CullMode::None,
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
        // TODO: get rid of this hardcoded crap
        let buffer_data = vec![InstanceData::default(); 1];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        NavMeshPass {
            command_sender,
            render_pipeline,
            instance_buffer,
        }
    }
}
