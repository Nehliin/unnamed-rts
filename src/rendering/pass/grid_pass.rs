use std::borrow::Cow;

use crate::resources::DebugRenderSettings;

use crate::rendering::{
    camera::Camera,
    common::{DepthTexture, DEPTH_FORMAT},
};
use crossbeam_channel::Sender;
use legion::*;

#[system]
pub fn draw(
    #[resource] pass: &GridPass,
    #[resource] debug_settings: &DebugRenderSettings,
    #[resource] device: &wgpu::Device,
    #[resource] depth_texture: &DepthTexture,
    #[resource] camera: &Camera,
    #[resource] current_frame: &wgpu::SwapChainTexture,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Grid pass encoder"),
    });
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Grid pass"),
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
    render_pass.push_debug_group("Grid pass");
    render_pass.set_pipeline(&pass.render_pipeline);
    render_pass.set_bind_group(0, &camera.bind_group(), &[]);
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
    command_sender: Sender<wgpu::CommandBuffer>,
}

impl GridPass {
    pub fn new(device: &wgpu::Device, command_sender: Sender<wgpu::CommandBuffer>) -> GridPass {
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Grid shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/grid.wgsl"))),
            flags: wgpu::ShaderFlags::VALIDATION,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Grid pass pipeline layout"),
                bind_group_layouts: &[Camera::get_or_create_layout(&device)],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[],
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
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
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

        GridPass {
            render_pipeline,
            command_sender,
        }
    }
}
