pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
#[derive(Debug)]
pub struct DepthTexture {
    texture: wgpu::Texture,
    // unclear if this should be stored here instead of the render passes
    // but this simplifies rezising
    pub view: wgpu::TextureView,
}

impl DepthTexture {
    pub fn new(device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) -> DepthTexture {
        let desc = Self::create_texture_descriptor(sc_desc);
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        DepthTexture {
            texture,
            view,
        }
    }

    fn create_texture_descriptor(sc_desc: &wgpu::SwapChainDescriptor) -> wgpu::TextureDescriptor {
        wgpu::TextureDescriptor {
            label: Some("Depth texture"),
            size: wgpu::Extent3d {
                width: sc_desc.width,
                height: sc_desc.height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) {
        self.texture = device.create_texture(&Self::create_texture_descriptor(sc_desc));
        self.view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}
