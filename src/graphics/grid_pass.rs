use crate::application::DebugMenueSettings;

use super::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
};
use crossbeam_channel::Sender;
use legion::*;
use wgpu::include_spirv;

#[system]
pub fn draw(
    #[state] pass: &GridPass,
    #[resource] debug_settings: &DebugMenueSettings,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] current_frame: &wgpu::SwapChainTexture,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Debug pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Debug pass"),
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
    render_pass.push_debug_group("Grid pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &pass.camera_bind_group, &[]);
    // This is kindof hacky because the render pass isn't actually needed here
    // but the main loop expects an command encoder or it will freeze so until
    // that is changed this will have to do
    if debug_settings.show_grid {
        render_pass.draw(0..6, 0..1);
    }
    render_pass.pop_debug_group();
    drop(render_pass);
    pass.command_sender.send(encoder.finish()).unwrap();
}

// Render pass to show "editor/debug" grid http://asliceofrendering.com/scene%20helper/2020/01/05/InfiniteGrid/
pub struct GridPass {
    render_pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl GridPass {
    pub fn new(
        device: &wgpu::Device,
        camera: &Camera,
        command_sender: Sender<wgpu::CommandBuffer>,
    ) -> GridPass {
        let vs_module = device.create_shader_module(&include_spirv!("shaders/grid.vert.spv"));
        let fs_module = device.create_shader_module(&include_spirv!("shaders/grid.frag.spv"));

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
                label: Some("Debug pass pipeline layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: "main",
                buffers: &[],
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

        GridPass {
            render_pipeline,
            camera_bind_group,
            command_sender,
        }
    }
}
