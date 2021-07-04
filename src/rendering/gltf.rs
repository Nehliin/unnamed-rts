use super::texture::*;
use crate::assets::AssetLoader;
use crate::rendering::vertex_buffers::*;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use crevice::std430::AsStd430;
use crevice::std430::Std430;
use glam::*;
use gltf::{accessor::util::ItemIter, mesh::util::ReadTexCoords};
use log::info;
use once_cell::sync::OnceCell;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use std::{
    borrow::Cow,
    ops::Range,
    path::Path,
    sync::atomic::{AtomicI32, Ordering},
    time::Instant,
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferAddress, Device, Queue, RenderPass, VertexAttribute, VertexFormat,
};

pub const INSTANCE_BUFFER_LEN: usize = 4000;
#[derive(Debug)]
pub struct GltfMesh {
    vertex_buffer: ImmutableVertexData<MeshVertex>,
    index_buffer: Buffer,
    num_indicies: u32,
    local_transform: Mat4, // pre calc if not needed in animations
    material: PbrMaterial,
}
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tanget: Vec3,
    pub tang_handeness: f32,
    pub tex_coords: Vec2,
}

impl VertexBuffer for MeshVertex {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            VertexAttribute {
                offset: 0,
                format: VertexFormat::Float32x3,
                shader_location: 0,
            },
            VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                format: VertexFormat::Float32x3,
                shader_location: 1,
            },
            VertexAttribute {
                offset: (std::mem::size_of::<[f32; 3]>() * 2) as BufferAddress,
                format: VertexFormat::Float32x3,
                shader_location: 2,
            },
            VertexAttribute {
                offset: (std::mem::size_of::<[f32; 3]>() * 3) as BufferAddress,
                format: VertexFormat::Float32,
                shader_location: 3,
            },
            VertexAttribute {
                offset: (std::mem::size_of::<[f32; 3]>() * 3 + std::mem::size_of::<f32>())
                    as BufferAddress,
                format: VertexFormat::Float32x2,
                shader_location: 4,
            },
        ]
    }
}
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
//TODO: The perspective part isn't needed here
pub struct InstanceData {
    model: Mat4,
    normal_matrix: Mat3,
    _pad: Vec3,
}

impl InstanceData {
    pub fn new(model: Mat4) -> Self {
        let sub_mat = Mat3::from_mat4(model);
        let normal_matrix = sub_mat.inverse().transpose();
        InstanceData {
            model,
            normal_matrix,
            _pad: Vec3::ZERO,
        }
    }
}

const SIZE_VEC4: BufferAddress = (std::mem::size_of::<Vec4>()) as BufferAddress;
const SIZE_VEC3: BufferAddress = (std::mem::size_of::<Vec3>()) as BufferAddress;

impl VertexBuffer for InstanceData {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Instance;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            VertexAttribute {
                offset: 0,
                format: VertexFormat::Float32x4,
                shader_location: 5,
            },
            VertexAttribute {
                offset: SIZE_VEC4,
                format: VertexFormat::Float32x4,
                shader_location: 6,
            },
            VertexAttribute {
                offset: SIZE_VEC4 * 2,
                format: VertexFormat::Float32x4,
                shader_location: 7,
            },
            VertexAttribute {
                offset: SIZE_VEC4 * 3,
                format: VertexFormat::Float32x4,
                shader_location: 8,
            },
            VertexAttribute {
                offset: SIZE_VEC4 * 4,
                format: VertexFormat::Float32x3,
                shader_location: 9,
            },
            VertexAttribute {
                offset: SIZE_VEC4 * 4 + SIZE_VEC3,
                format: VertexFormat::Float32x3,
                shader_location: 10,
            },
            VertexAttribute {
                offset: SIZE_VEC4 * 4 + SIZE_VEC3 * 2,
                format: VertexFormat::Float32x3,
                shader_location: 11,
            },
        ]
    }
}
#[derive(Debug)]
struct PbrMaterialTexture {
    sampler: wgpu::Sampler,
    view: wgpu::TextureView,
    texture: wgpu::Texture,
}

impl PbrMaterialTexture {
    pub fn new(
        device: &Device,
        queue: &Queue,
        texture_content: &TextureContent<'_>,
        sampler_info: &gltf::texture::Sampler,
        srgb: bool,
    ) -> Self {
        let texture = allocate_simple_texture(device, queue, &texture_content, srgb);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PbrMaterial texture sampler"),
            address_mode_u: match sampler_info.wrap_s() {
                gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_v: match sampler_info.wrap_t() {
                gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
            },
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: sampler_info
                .mag_filter()
                .map(|filter| match filter {
                    gltf::texture::MagFilter::Nearest => wgpu::FilterMode::Nearest,
                    gltf::texture::MagFilter::Linear => wgpu::FilterMode::Linear,
                })
                .unwrap_or(wgpu::FilterMode::Linear),
            min_filter: sampler_info
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        PbrMaterialTexture {
            sampler,
            view,
            texture,
        }
    }
}

#[derive(Debug)]
pub struct PbrMaterial {
    base_color_texture: Option<PbrMaterialTexture>,
    metallic_roughness_texture: Option<PbrMaterialTexture>,
    factors: PbrMaterialFactors,
    factor_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

#[derive(Debug, AsStd430)]
struct PbrMaterialFactors {
    base_color_factor: mint::Vector4<f32>,
    metallic_factor: f32,
    rougness_factor: f32,
    occulusion_strenght: f32,
    normal_scale: f32,
}

impl PbrMaterial {
    pub fn new(
        device: &Device,
        queue: &Queue,
        gltf_material: &gltf::Material,
        textures: &[TextureContent<'_>],
    ) -> Self {
        let pbr_metallic_roughness = gltf_material.pbr_metallic_roughness();
        let base_color_texture = pbr_metallic_roughness
            .base_color_texture()
            .map(|texture_info| {
                let gltf_texture = texture_info.texture();
                PbrMaterialTexture::new(
                    device,
                    queue,
                    &textures[gltf_texture.index()],
                    &gltf_texture.sampler(),
                    true,
                )
            });
        let metallic_roughness_texture =
            pbr_metallic_roughness
                .metallic_roughness_texture()
                .map(|texture_info| {
                    let gltf_texture = texture_info.texture();
                    PbrMaterialTexture::new(
                        device,
                        queue,
                        &textures[gltf_texture.index()],
                        &gltf_texture.sampler(),
                        false,
                    )
                });
        let occulusion_texture = gltf_material.occlusion_texture().map(|texture_info| {
            let gltf_texture = texture_info.texture();
            PbrMaterialTexture::new(
                device,
                queue,
                &textures[gltf_texture.index()],
                &gltf_texture.sampler(),
                false,
            )
        });
        let normal_texture = gltf_material.normal_texture().map(|texture_info| {
            let gltf_texture = texture_info.texture();
            PbrMaterialTexture::new(
                device,
                queue,
                &textures[gltf_texture.index()],
                &gltf_texture.sampler(),
                false,
            )
        });
        let factors = PbrMaterialFactors {
            rougness_factor: pbr_metallic_roughness.roughness_factor(),
            metallic_factor: pbr_metallic_roughness.metallic_factor(),
            base_color_factor: pbr_metallic_roughness.base_color_factor().into(),
            occulusion_strenght: gltf_material
                .occlusion_texture()
                .map(|occlusion_tex| occlusion_tex.strength())
                .unwrap_or(1.0),
            normal_scale: gltf_material
                .normal_texture()
                .map(|normal_tex| normal_tex.scale())
                .unwrap_or(1.0),
        };
        let factor_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("PbrMaterial factor buffer"),
            contents: factors.as_std430().as_bytes(),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });
        let placeholder = get_white_placeholder_texture(device, queue);
        let normal_map_placeholder = get_normal_placeholder_texture(device, queue);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::get_or_create_layout(device),
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
                    resource: wgpu::BindingResource::TextureView(
                        &occulusion_texture.as_ref().unwrap_or(placeholder).view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(
                        &occulusion_texture.as_ref().unwrap_or(placeholder).sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &normal_texture
                            .as_ref()
                            .unwrap_or(normal_map_placeholder)
                            .view,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(
                        &normal_texture
                            .as_ref()
                            .unwrap_or(normal_map_placeholder)
                            .sampler,
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &factor_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
            label: Some("PbrMaterial bindgroup"),
        });

        PbrMaterial {
            base_color_texture,
            metallic_roughness_texture,
            factors,
            factor_buffer,
            bind_group,
        }
    }

    pub fn get_or_create_layout(device: &Device) -> &'static wgpu::BindGroupLayout {
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
                    // occulusion texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // normal texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // material factors
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
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

fn get_white_placeholder_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> &'static PbrMaterialTexture {
    static PLACEHOLDER_TEXTURE: OnceCell<PbrMaterialTexture> = OnceCell::new();
    PLACEHOLDER_TEXTURE.get_or_init(|| -> PbrMaterialTexture {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let content = TextureContent {
            label: Some("White placeholder texture"),
            bytes: Cow::Owned(vec![255, 255, 255, 255]),
            size,
            stride: 4,
            format: wgpu::TextureFormat::Rgba8Unorm,
        };
        let texture = allocate_simple_texture(device, queue, &content, false);
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
        PbrMaterialTexture {
            sampler,
            view,
            texture,
        }
    })
}

fn get_normal_placeholder_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> &'static PbrMaterialTexture {
    static PLACEHOLDER_TEXTURE: OnceCell<PbrMaterialTexture> = OnceCell::new();
    PLACEHOLDER_TEXTURE.get_or_init(|| {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let content = TextureContent {
            label: Some("Normal map placeholder texture"),
            bytes: Cow::Owned(vec![128, 128, 255, 255]),
            size,
            stride: 4,
            format: wgpu::TextureFormat::Rgba8Unorm,
        };
        let texture = allocate_simple_texture(device, queue, &content, false);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Normal map placeholder sampler"),
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
        PbrMaterialTexture {
            view,
            sampler,
            texture,
        }
    })
}

#[derive(Debug)]
pub struct GltfModel {
    pub meshes: Vec<GltfMesh>,
    pub instance_buffer: MutableVertexData<InstanceData>,
    pub min_vertex: Vec3,
    pub max_vertex: Vec3,
}

impl GltfModel {
    fn load(device: &Device, queue: &Queue, path: impl AsRef<Path>) -> Result<GltfModel> {
        let gltf_start = Instant::now();
        let (gltf, buffers, images) = gltf::import(path)?;
        let gltf_load_time = gltf_start.elapsed().as_secs_f32();
        let start = Instant::now();
        let texture_content = images
            .par_iter()
            .map(TextureContent::from)
            .collect::<Vec<_>>();
        let min_vertex = [
            AtomicI32::new(i32::MAX),
            AtomicI32::new(i32::MAX),
            AtomicI32::new(i32::MAX),
        ];
        let max_vertex = [
            AtomicI32::new(i32::MIN),
            AtomicI32::new(i32::MIN),
            AtomicI32::new(i32::MIN),
        ];
        let meshes = gltf.nodes()
            .par_bridge()
            .filter(|node| node.mesh().is_some())
            .map(|node| {
                let (position, rotation, scaled) = node.transform().decomposed();
                let local_transform = Mat4::from_scale_rotation_translation(
                    scaled.into(),
                    Quat::from_vec4(rotation.into()),
                    position.into(),
                );
                let mesh = node.mesh().unwrap();
                mesh.primitives().par_bridge().map( |primitive| {
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
                        .expect("Mesh must have vertecies")
                        .zip(reader.read_normals().expect("Mesh must have normals"))
                        .zip(reader.read_tangents().expect("TODO: compute tangents"))
                        .zip(tex_coords_iter)
                        .map(|(((pos, norm), tan), tex)| {
                            max_vertex[0].fetch_max(pos[0].ceil() as i32, Ordering::AcqRel);
                            max_vertex[1].fetch_max(pos[1].ceil() as i32, Ordering::AcqRel);
                            max_vertex[2].fetch_max(pos[2].ceil() as i32, Ordering::AcqRel);
                            min_vertex[0].fetch_min(pos[0].floor() as i32, Ordering::AcqRel);
                            min_vertex[1].fetch_min(pos[1].floor() as i32, Ordering::AcqRel);
                            min_vertex[2].fetch_min(pos[2].floor() as i32, Ordering::AcqRel);
                            // TODO: Apply local transform? 
                            let position = /*local_transform */ Vec4::new(pos[0], pos[1], pos[2], 1.0);
                            MeshVertex {
                                position: position.into(),
                                normal: Vec3::new(norm[0], norm[1], norm[2]),
                                tanget: Vec3::new(tan[0], tan[1], tan[2]),
                                tang_handeness: tan[3],
                                tex_coords: Vec2::new(tex[0], tex[1]),
                            }
                        })
                        .collect::<Vec<_>>();
                    let vertex_buffer = VertexBuffer::allocate_immutable_buffer(device, &vertices);
                    let indicies = reader
                        .read_indices()
                        .expect("Mesh must have indicies")
                        .into_u32()
                        .collect::<Vec<u32>>();
                    let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("Index buffer"),
                        usage: wgpu::BufferUsage::INDEX,
                        contents: bytemuck::cast_slice(&indicies),
                    });
                    GltfMesh {
                        vertex_buffer,
                        local_transform,
                        index_buffer,
                        material: PbrMaterial::new(device, queue, &primitive.material(), &texture_content),
                        num_indicies: indicies.len() as u32,
                    }
                })
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Vec<_>>();

        info!(
            "Glft Load: {}, Loadtime: {}",
            gltf_load_time,
            start.elapsed().as_secs_f32()
        );
        let instance_buffer_len = INSTANCE_BUFFER_LEN * std::mem::size_of::<InstanceData>();
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        Ok(GltfModel {
            meshes,
            instance_buffer,
            min_vertex: Vec3::new(
                min_vertex[0].load(Ordering::Acquire) as f32,
                min_vertex[1].load(Ordering::Acquire) as f32,
                min_vertex[2].load(Ordering::Acquire) as f32,
            ),
            max_vertex: Vec3::new(
                max_vertex[0].load(Ordering::Acquire) as f32,
                max_vertex[1].load(Ordering::Acquire) as f32,
                max_vertex[2].load(Ordering::Acquire) as f32,
            ),
        })
    }

    pub fn draw_instanced<'a, 'b>(&'a self, render_pass: &mut RenderPass<'b>, instances: Range<u32>)
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

    pub fn draw_with_instance_buffer<'a, 'b>(
        &'a self,
        render_pass: &mut RenderPass<'b>,
        instance_buffer: &'b MutableVertexData<InstanceData>,
        instances: Range<u32>,
    ) where
        'a: 'b,
    {
        self.meshes.iter().for_each(|mesh| {
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, &mesh.material.bind_group, &[]);
            render_pass.draw_indexed(0..mesh.num_indicies, 0, instances.clone());
        });
    }
}

impl AssetLoader for GltfModel {
    fn load(path: &Path, device: &Device, queue: &Queue) -> Result<Self> {
        GltfModel::load(device, queue, path)
    }

    fn extensions() -> &'static [&'static str] {
        &["gltf", "glb"]
    }
}
