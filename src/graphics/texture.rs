use anyhow::Result;
use std::{marker::PhantomData, path::Path};

pub trait TextureShaderLayout: 'static {
    const VISIBILITY: wgpu::ShaderStage;
    fn get_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout;
}

pub struct TextureData<T: TextureShaderLayout> {
    pub _marker: PhantomData<T>,
    pub bind_group: wgpu::BindGroup,
    // unclear if default view for multilayered textures
    // should be separated from invidual layer views
    // could maybe be separate texture data type?
    pub views: Vec<wgpu::TextureView>,
    pub sampler: wgpu::Sampler,
    pub texture: wgpu::Texture,
}

impl<T: TextureShaderLayout> TextureData<T> {
    pub fn new(
        bind_group: wgpu::BindGroup,
        texture: wgpu::Texture,
        views: Vec<wgpu::TextureView>,
        sampler: wgpu::Sampler,
    ) -> Self {
        TextureData {
            bind_group,
            texture,
            views,
            sampler,
            _marker: PhantomData::default(),
        }
    }
    // if the TextureData type would contain information about
    // if the texture is multilayered or not this could be done in
    // a nicer way. Might lead to less control though so I'll begin with this.
    // Another option is to deref down to the texture itself
    #[inline]
    pub fn create_new_view(&self, desc: &wgpu::TextureViewDescriptor) -> wgpu::TextureView {
        self.texture.create_view(desc)
    }
}

pub trait LoadableTexture: Sized + TextureShaderLayout {
    fn load_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: impl AsRef<Path>,
    ) -> Result<TextureData<Self>>;
}

pub trait Texture: Sized {
    fn allocate_texture(device: &wgpu::Device) -> TextureData<Self>
    where
        Self: TextureShaderLayout;
}
