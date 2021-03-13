use std::borrow::Cow;

use wgpu::{Device, Queue};

pub struct TextureContent<'a> {
    pub label: Option<&'static str>,
    pub format: gltf::image::Format,
    pub bytes: Cow<'a, [u8]>,
    pub stride: u32,
    pub size: wgpu::Extent3d,
}

impl<'a> TextureContent<'a> {
    fn to_wgpu_format(&self, srgb: bool) -> wgpu::TextureFormat {
        match self.format {
            gltf::image::Format::R8 => wgpu::TextureFormat::R8Unorm,
            gltf::image::Format::R8G8 => wgpu::TextureFormat::Rg8Unorm,
            gltf::image::Format::R8G8B8 => {
                if srgb {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Rgba8Unorm
                }
            }
            gltf::image::Format::R8G8B8A8 => {
                if srgb {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Rgba8Unorm
                }
            }
            gltf::image::Format::B8G8R8 => {
                if srgb {
                    wgpu::TextureFormat::Bgra8UnormSrgb
                } else {
                    wgpu::TextureFormat::Bgra8Unorm
                }
            }
            gltf::image::Format::B8G8R8A8 => {
                if srgb {
                    wgpu::TextureFormat::Bgra8UnormSrgb
                } else {
                    wgpu::TextureFormat::Bgra8Unorm
                }
            }
            gltf::image::Format::R16 => wgpu::TextureFormat::R16Float,
            gltf::image::Format::R16G16 => wgpu::TextureFormat::Rg16Float,
            gltf::image::Format::R16G16B16 => wgpu::TextureFormat::Rgba16Float,
            gltf::image::Format::R16G16B16A16 => wgpu::TextureFormat::Rgba16Float,
        }
    }
}

impl<'a> From<&'a gltf::image::Data> for TextureContent<'a> {
    fn from(image_data: &'a gltf::image::Data) -> Self {
        let size = wgpu::Extent3d {
            width: image_data.width,
            height: image_data.height,
            depth: 1,
        };
        let label = Some("GltfTexture");
        match image_data.format {
            gltf::image::Format::R8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 1,
            },
            gltf::image::Format::R8G8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 2,
            },
            gltf::image::Format::R8G8B8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Owned({
                    // TODO: This might be very ineffective
                    let mut converted =
                        Vec::with_capacity(image_data.pixels.len() / 3 + image_data.pixels.len());
                    image_data.pixels.chunks_exact(3).for_each(|chunk| {
                        converted.extend(chunk);
                        converted.push(255);
                    });
                    converted
                }),
                stride: 4,
            },
            gltf::image::Format::R8G8B8A8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::B8G8R8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Owned({
                    // TODO: This might be very ineffective might be better to pre alloc
                    let mut converted =
                        Vec::with_capacity(image_data.pixels.len() / 3 + image_data.pixels.len());
                    image_data.pixels.chunks_exact(3).for_each(|chunk| {
                        converted.extend(chunk);
                        converted.push(255);
                    });
                    converted
                }),
                stride: 4,
            },
            gltf::image::Format::B8G8R8A8 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::R16 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 2,
            },
            gltf::image::Format::R16G16 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::R16G16B16 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Owned({
                    // TODO: This might be very ineffective might be better to pre alloc
                    let mut converted =
                        Vec::with_capacity(image_data.pixels.len() / 6 + image_data.pixels.len());
                    image_data.pixels.chunks_exact(6).for_each(|chunk| {
                        converted.extend(chunk);
                        converted.push(255);
                        converted.push(255);
                    });
                    converted
                }),
                stride: 8,
            },
            gltf::image::Format::R16G16B16A16 => TextureContent {
                label,
                size,
                format: image_data.format,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 8,
            },
        }
    }
}

pub fn allocate_simple_texture(
    device: &Device,
    queue: &Queue,
    content: &TextureContent<'_>,
    srgb: bool,
) -> wgpu::Texture {
    let TextureContent {
        label,
        format: _,
        stride,
        size,
        bytes,
    } = content;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: *label,
        size: *size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: content.to_wgpu_format(srgb),
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
    });
    let texutre_copy_view = wgpu::TextureCopyView {
        texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
    };
    let texture_data_layout = wgpu::TextureDataLayout {
        offset: 0,
        bytes_per_row: stride * size.width,
        rows_per_image: 0,
    };
    queue.write_texture(texutre_copy_view, &bytes, texture_data_layout, *size);
    texture
}
