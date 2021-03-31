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
