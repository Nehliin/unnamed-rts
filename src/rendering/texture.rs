use rayon::prelude::*;
use std::{borrow::Cow, num::NonZeroU32};
use wgpu::{Device, Queue};

#[derive(Debug, Clone)]
pub struct TextureContent<'a> {
    pub label: Option<&'static str>,
    pub format: wgpu::TextureFormat,
    pub bytes: Cow<'a, [u8]>,
    pub stride: u32,
    pub size: wgpu::Extent3d,
}
// FIXME
impl<'a> TextureContent<'a> {
    // This can be a simple 4x4 tileset that's repeated
    pub fn checkerd(size: u32, resolution: usize) -> Self {
        let stride = 4;
        let img_size = wgpu::Extent3d {
            width: size * resolution as u32,
            height: size * resolution as u32,
            depth_or_array_layers: 1,
        };
        // construct checkered content
        let mut bytes: Vec<u8> = vec![0; (img_size.width * img_size.height) as usize * stride];
        bytes
            .par_chunks_exact_mut(size as usize * stride * resolution)
            .enumerate()
            .for_each(|(x, chunk)| {
                chunk
                    .chunks_exact_mut(stride)
                    .enumerate()
                    .for_each(|(y, texel)| {
                        if (x + y) % 2 == 0 {
                            texel.fill(128);
                        } else {
                            texel.fill(64)
                        }
                    });
            });
        TextureContent {
            label: Some("Checkered default texture"),
            format: wgpu::TextureFormat::Rgba8Unorm,
            bytes: Cow::Owned(bytes),
            stride: stride as u32,
            size: img_size,
        }
    }

    pub fn black(width: u32, height: u32) -> Self {
        let stride = 4;
        let img_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        TextureContent {
            label: Some("Black/empty default texture"),
            format: wgpu::TextureFormat::Rgba8Unorm,
            bytes: Cow::Owned(vec![0; (width * height) as usize * stride]),
            stride: stride as u32,
            size: img_size,
        }
    }
}

fn to_srgb(format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8UnormSrgb,
        _ => format,
    }
}

impl<'a> From<&'a egui::Texture> for TextureContent<'a> {
    fn from(egui_texture: &'a egui::Texture) -> Self {
        let size = wgpu::Extent3d {
            width: egui_texture.width as u32,
            height: egui_texture.height as u32,
            depth_or_array_layers: 1,
        };
        let bytes: Vec<u8> = egui_texture
            .srgba_pixels()
            .map(|px| std::array::IntoIter::new([px.r(), px.g(), px.b(), px.a()]))
            .flatten()
            .collect();
        TextureContent {
            label: Some("egui_texture"),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            bytes: Cow::Owned(bytes),
            stride: 4,
            size,
        }
    }
}

impl<'a> From<&'a gltf::image::Data> for TextureContent<'a> {
    fn from(image_data: &'a gltf::image::Data) -> Self {
        let size = wgpu::Extent3d {
            width: image_data.width,
            height: image_data.height,
            depth_or_array_layers: 1,
        };
        let label = Some("GltfTexture");
        match image_data.format {
            gltf::image::Format::R8 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::R8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 1,
            },
            gltf::image::Format::R8G8 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::Rg8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 2,
            },
            gltf::image::Format::R8G8B8 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::Rgba8Unorm,
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
                format: wgpu::TextureFormat::Rgba8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::B8G8R8 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::Bgra8Unorm,
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
                format: wgpu::TextureFormat::Bgra8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::R16 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::R16Float,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 2,
            },
            gltf::image::Format::R16G16 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::Rg16Float,
                bytes: Cow::Borrowed(&image_data.pixels),
                stride: 4,
            },
            gltf::image::Format::R16G16B16 => TextureContent {
                label,
                size,
                format: wgpu::TextureFormat::Rgba16Float,
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
                format: wgpu::TextureFormat::Rgba16Float,
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
        format,
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
        format: if srgb { to_srgb(*format) } else { *format },
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
    });
    let texutre_copy_view = wgpu::ImageCopyTexture {
        texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
    };
    let texture_data_layout = wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: NonZeroU32::new(stride * size.width),
        rows_per_image: NonZeroU32::new(size.height),
    };
    queue.write_texture(texutre_copy_view, &bytes, texture_data_layout, *size);
    texture
}

pub fn update_texture_data(
    content: &TextureContent<'_>,
    allocated_texture: &wgpu::Texture,
    queue: &Queue,
) {
    let texture_data_layout = wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: NonZeroU32::new(content.stride * content.size.width),
        rows_per_image: NonZeroU32::new(content.size.height),
    };
    let texture_view = wgpu::ImageCopyTexture {
        texture: allocated_texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
    };
    queue.write_texture(
        texture_view,
        &content.bytes,
        texture_data_layout,
        content.size,
    )
}