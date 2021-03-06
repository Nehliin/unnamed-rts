use crate::graphics::vertex_buffers::*;
use anyhow::Result;
use bytemuck::bytes_of;
use crevice::std430::AsStd430;
use crevice::std430::Std430;
use glam::*;
use gltf::{accessor::util::ItemIter, mesh::util::ReadTexCoords};
use log::info;
use once_cell::sync::OnceCell;
use std::{borrow::Cow, ops::Range, path::Path, time::Instant};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, Device, Queue, RenderPass,
};

use crate::assets::AssetLoader;

use super::obj_model::{InstanceData, MeshVertex};

#[derive(Debug)]
pub struct GltfMesh {
    pub vertex_buffer: ImmutableVertexData<MeshVertex>,
    pub index_buffer: Buffer,
    num_indicies: u32,
    local_transform: Mat4, // pre calc if not needed in animations
    material: GltfMaterial,
}

#[derive(Debug)]
pub struct MaterialTexture {
    pub sampler: wgpu::Sampler,
    pub view: wgpu::TextureView,
}

impl MaterialTexture {
    pub fn new(device: &Device, textures: &[wgpu::Texture], gltf_texture: &gltf::Texture) -> Self {
        let gltf_sampler = gltf_texture.sampler();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Material texture sampler"),
            address_mode_u: match gltf_sampler.wrap_s() {
                gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_v: match gltf_sampler.wrap_t() {
                gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: gltf_sampler
                .mag_filter()
                .map(|filter| match filter {
                    gltf::texture::MagFilter::Nearest => wgpu::FilterMode::Nearest,
                    gltf::texture::MagFilter::Linear => wgpu::FilterMode::Linear,
                })
                .unwrap_or(wgpu::FilterMode::Linear),
            min_filter: gltf_sampler
                .min_filter()
                .map(|filter| match filter {
                    gltf::texture::MinFilter::Nearest => wgpu::FilterMode::Nearest,
                    gltf::texture::MinFilter::Linear => wgpu::FilterMode::Linear,
                    _ => wgpu::FilterMode::Linear,
                })
                .unwrap_or(wgpu::FilterMode::Linear),
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        let view = textures
            .get(gltf_texture.source().index())
            .unwrap()
            .create_view(&wgpu::TextureViewDescriptor::default());
        MaterialTexture { sampler, view }
    }
}

fn white_texture(device: &Device, queue: &Queue) -> &'static MaterialTexture {
    static DEFAULT_TEXTURE: OnceCell<MaterialTexture> = OnceCell::new();
    DEFAULT_TEXTURE.get_or_init(|| {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("White placeholder texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // Wasteful format?
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let texutre_copy_view = wgpu::TextureCopyView {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        };
        let texture_data_layout = wgpu::TextureDataLayout {
            offset: 0,
            bytes_per_row: 4,
            rows_per_image: 0,
        };

        queue.write_texture(
            texutre_copy_view,
            &[255, 255, 255, 255],
            texture_data_layout,
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("White sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0, // related to mipmaps
            lod_max_clamp: 100.0,  // related to mipmaps
            compare: None,
            ..Default::default()
        });

        MaterialTexture { view, sampler }
    })
}

#[derive(Debug)]
pub struct GltfMaterial {
    pub base_color_texture: Option<MaterialTexture>,
    pub metallic_roughness_texture: Option<MaterialTexture>,
    pub factors: GltfMaterialFactors,
    factor_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

#[derive(Debug, AsStd430)]
pub struct GltfMaterialFactors {
    pub base_color_factor: mint::Vector4<f32>,
    pub metallic_factor: f32,
    pub rougness_factor: f32,
}

impl GltfMaterial {
    pub fn new(
        device: &Device,
        queue: &Queue,
        gltf_material: &gltf::Material,
        textures: &[wgpu::Texture],
    ) -> Self {
        let pbr_metallic_roughness = gltf_material.pbr_metallic_roughness();
        let base_color_texture = pbr_metallic_roughness
            .base_color_texture()
            .map(|texture_info| {
                let texture = texture_info.texture();
                MaterialTexture::new(device, &textures, &texture)
            });
        let metallic_roughness_texture =
            pbr_metallic_roughness
                .metallic_roughness_texture()
                .map(|texture_info| {
                    let texture = texture_info.texture();
                    MaterialTexture::new(device, &textures, &texture)
                });
        let factors = GltfMaterialFactors {
            rougness_factor: pbr_metallic_roughness.roughness_factor(),
            metallic_factor: pbr_metallic_roughness.metallic_factor(),
            base_color_factor: pbr_metallic_roughness.base_color_factor().into(),
        };
        let factor_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Material factor buffer"),
            contents: factors.as_std430().as_bytes(),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });
        let placeholder = white_texture(device, queue);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::get_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &base_color_texture.as_ref().unwrap_or(placeholder).view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(
                        &base_color_texture.as_ref().unwrap_or(placeholder).sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &metallic_roughness_texture
                            .as_ref()
                            .unwrap_or(placeholder)
                            .view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(
                        &metallic_roughness_texture
                            .as_ref()
                            .unwrap_or(placeholder)
                            .sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &factor_buffer,
                        offset: 0,
                        size: None,
                    },
                },
            ],
            label: Some("Material bindgroup"),
        });

        GltfMaterial {
            base_color_texture,
            metallic_roughness_texture,
            factors,
            bind_group,
            factor_buffer,
        }
    }

    pub fn get_layout(device: &Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(move || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    // base color texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // metallic roughness texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // material factors
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("PbrMaterial bind group layout"),
            })
        })
    }
}

pub struct TextureInfo<'a> {
    format: wgpu::TextureFormat,
    bytes: Cow<'a, [u8]>,
    bytes_per_row: u32,
}

impl<'a> From<&'a gltf::image::Data> for TextureInfo<'a> {
    fn from(image_data: &'a gltf::image::Data) -> Self {
        // TODO: handle Srgb
        match image_data.format {
            gltf::image::Format::R8 => TextureInfo {
                format: wgpu::TextureFormat::R8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 1,
            },
            gltf::image::Format::R8G8 => TextureInfo {
                format: wgpu::TextureFormat::Rg8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 2,
            },
            gltf::image::Format::R8G8B8 => TextureInfo {
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
                bytes_per_row: 4,
            },
            gltf::image::Format::R8G8B8A8 => TextureInfo {
                format: wgpu::TextureFormat::Rgba8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 4,
            },
            gltf::image::Format::B8G8R8 => TextureInfo {
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
                bytes_per_row: 4,
            },
            gltf::image::Format::B8G8R8A8 => TextureInfo {
                format: wgpu::TextureFormat::Bgra8Unorm,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 4,
            },
            gltf::image::Format::R16 => TextureInfo {
                format: wgpu::TextureFormat::R16Float,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 2,
            },
            gltf::image::Format::R16G16 => TextureInfo {
                format: wgpu::TextureFormat::Rg16Float,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 4,
            },
            gltf::image::Format::R16G16B16 => TextureInfo {
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
                bytes_per_row: 8,
            },
            gltf::image::Format::R16G16B16A16 => TextureInfo {
                format: wgpu::TextureFormat::Rgba16Float,
                bytes: Cow::Borrowed(&image_data.pixels),
                bytes_per_row: 8,
            },
        }
    }
}

fn allocate_texture(
    device: &Device,
    queue: &Queue,
    image_data: &gltf::image::Data,
) -> wgpu::Texture {
    let TextureInfo {
        format,
        bytes_per_row,
        bytes,
    } = TextureInfo::from(image_data);
    let (width, height) = (image_data.width, image_data.height);
    let size = wgpu::Extent3d {
        width,
        height,
        depth: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Gltf Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
    });
    let texutre_copy_view = wgpu::TextureCopyView {
        texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
    };
    let texture_data_layout = wgpu::TextureDataLayout {
        offset: 0,
        bytes_per_row: bytes_per_row * width, //TODO: Will probably break
        rows_per_image: 0,
    };
    queue.write_texture(texutre_copy_view, &bytes, texture_data_layout, size);
    texture
}

#[derive(Debug)]
pub struct GltfModel {
    pub meshes: Vec<GltfMesh>,
    pub textures: Vec<wgpu::Texture>,
    pub instance_buffer: MutableVertexData<InstanceData>,
}

impl GltfModel {
    fn load(device: &Device, queue: &Queue, path: impl AsRef<Path>) -> Result<GltfModel> {
        let gltf_start = Instant::now();
        let (gltf, buffers, images) = gltf::import(path)?;
        let start = Instant::now();
        let textures = images
            .iter()
            .map(|image| allocate_texture(device, queue, image))
            .collect::<Vec<_>>();
        let mut meshes = Vec::new();
        gltf.nodes()
            .filter(|node| node.mesh().is_some())
            .for_each(|node| {
                let (position, rotation, scaled) = node.transform().decomposed();
                let local_transform = Mat4::from_scale_rotation_translation(
                    scaled.into(),
                    rotation.into(),
                    position.into(),
                );
                let mesh = node.mesh().unwrap();
                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                    let tex_coords_iter = reader
                        .read_tex_coords(0)
                        .unwrap_or_else(|| {
                            ReadTexCoords::F32(gltf::accessor::Iter::Standard(ItemIter::new(
                                // 2 f32s
                                &[0; 8], 8,
                            )))
                        })
                        .into_f32()
                        .cycle();
                    let vertices = reader
                        .read_positions()
                        .unwrap()
                        .zip(reader.read_normals().unwrap())
                        .zip(tex_coords_iter)
                        .map(|((pos, norm), tex)| {
                            // fixa normals?
                            let position = local_transform * Vec4::new(pos[0], pos[1], pos[2], 1.0);
                            MeshVertex {
                                position: position.into(),
                                normal: Vec3::new(norm[0], norm[1], norm[2]),
                                tex_coords: Vec2::new(tex[0], tex[1]),
                            }
                        })
                        .collect::<Vec<_>>();
                    let vertex_buffer = VertexBuffer::allocate_immutable_buffer(device, &vertices);
                    let indicies = reader
                        .read_indices()
                        .unwrap()
                        .into_u32()
                        .collect::<Vec<u32>>();
                    let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("Index buffer"),
                        usage: wgpu::BufferUsage::INDEX,
                        contents: bytemuck::cast_slice(&indicies),
                    });
                    meshes.push(GltfMesh {
                        vertex_buffer,
                        local_transform,
                        index_buffer,
                        material: GltfMaterial::new(
                            device,
                            queue,
                            &primitive.material(),
                            &textures,
                        ),
                        num_indicies: indicies.len() as u32,
                    });
                }
            });
        info!(
            "Glft Load: {}, Loadtime: {}",
            gltf_start.elapsed().as_secs_f32(),
            start.elapsed().as_secs_f32()
        );
        let instance_buffer_len = 4000 * std::mem::size_of::<InstanceData>();
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        Ok(GltfModel {
            meshes,
            instance_buffer,
            textures,
        })
    }

    pub fn draw<'a, 'b>(&'a self, render_pass: &mut RenderPass<'b>, instances: Range<u32>)
    where
        'a: 'b,
    {
        self.meshes.iter().for_each(|mesh| {
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, &mesh.material.bind_group, &[]);
            render_pass.draw_indexed(0..mesh.num_indicies, 0, instances.clone());
        });
    }
}

impl AssetLoader for GltfModel {
    fn load(path: &std::path::PathBuf, device: &Device, queue: &Queue) -> Result<Self> {
        GltfModel::load(device, queue, path.as_path())
    }

    fn extensions() -> &'static [&'static str] {
        &["gltf", "glb"]
    }
}
