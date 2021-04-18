use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use wgpu::{BufferAddress, VertexAttribute, VertexFormat};

use super::vertex_buffers::VertexBuffer;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

#[derive(Debug)]
pub struct DepthTexture {
    texture: wgpu::Texture,
    // unclear if this should be stored here instead of the render passes
    // but this simplifies rezising
    pub view: wgpu::TextureView,
}

impl DepthTexture {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> DepthTexture {
        let desc = Self::create_texture_descriptor(width, height);
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        DepthTexture { texture, view }
    }

    fn create_texture_descriptor(width: u32, height: u32) -> wgpu::TextureDescriptor<'static> {
        wgpu::TextureDescriptor {
            label: Some("Depth texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.texture = device.create_texture(&Self::create_texture_descriptor(width, height));
        self.view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
//TODO: The perspective part isn't needed here
pub struct InstanceData {
    model_matrix: Mat4,
}

impl InstanceData {
    pub fn new(model_matrix: Mat4) -> Self {
        InstanceData { model_matrix }
    }
}

const ROW_SIZE: BufferAddress = (std::mem::size_of::<f32>() * 4) as BufferAddress;

impl VertexBuffer for InstanceData {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Instance;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            VertexAttribute {
                offset: 0,
                format: VertexFormat::Float4,
                shader_location: 5,
            },
            VertexAttribute {
                offset: ROW_SIZE,
                format: VertexFormat::Float4,
                shader_location: 6,
            },
            VertexAttribute {
                offset: ROW_SIZE * 2,
                format: VertexFormat::Float4,
                shader_location: 7,
            },
            VertexAttribute {
                offset: ROW_SIZE * 3,
                format: VertexFormat::Float4,
                shader_location: 8,
            },
        ]
    }
}
