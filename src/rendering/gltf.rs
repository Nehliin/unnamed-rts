use super::texture::*;
use crate::assets::AssetLoader;
use crate::components::Transform;
use crate::rendering::vertex_buffers::*;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use crevice::std430::AsStd430;
use crevice::std430::Std430;
use glam::*;
use gltf::buffer::Data;
use gltf::Primitive;
use gltf::{accessor::util::ItemIter, mesh::util::ReadTexCoords};
use log::info;
use once_cell::sync::OnceCell;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use std::{borrow::Cow, path::Path, time::Instant};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferAddress, Device, Queue, RenderPass, VertexAttribute, VertexFormat,
};

#[derive(Debug)]
pub struct GltfMesh {
    index: usize,
    vertex_buffer: ImmutableVertexBuffer<MeshVertex>,
    index_buffer: Buffer,
    num_indicies: u32,
    local_transform: Affine3A,
    min_vertex: Vec3,
    max_vertex: Vec3,
    material: PbrMaterial,
}

impl GltfMesh {
    /// Get a reference to the gltf mesh's index.
    pub fn index(&self) -> &usize {
        &self.index
    }

    /// Get a reference to the gltf mesh's local transform.
    pub fn local_transform(&self) -> &Affine3A {
        &self.local_transform
    }

    pub fn draw_with_instance_buffer<'a, 'b>(
        &'a self,
        render_pass: &mut RenderPass<'b>,
        instance_buffer: &'b MutableVertexBuffer<InstanceData>,
    ) where
        'a: 'b,
    {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(1, &self.material.bind_group, &[]);
        render_pass.draw_indexed(0..self.num_indicies, 0, 0..instance_buffer.size() as u32);
    }
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

impl VertexData for MeshVertex {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Vertex;

    fn attributes<'a>() -> &'a [wgpu::VertexAttribute] {
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
    pub fn new(model: &Transform) -> Self {
        let sub_mat = model.matrix.matrix3;
        let normal_matrix = sub_mat.inverse().transpose().into();
        InstanceData {
            model: model.matrix.into(),
            normal_matrix,
            _pad: Vec3::ZERO,
        }
    }
}

const SIZE_VEC4: BufferAddress = (std::mem::size_of::<Vec4>()) as BufferAddress;
const SIZE_VEC3: BufferAddress = (std::mem::size_of::<Vec3>()) as BufferAddress;

impl VertexData for InstanceData {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;

    fn attributes<'a>() -> &'a [wgpu::VertexAttribute] {
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
        let texture = allocate_simple_texture(device, queue, texture_content, srgb);
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
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let placeholder = get_white_placeholder_texture(device, queue);
        let normal_map_placeholder = get_normal_placeholder_texture(device, queue);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: Self::get_or_create_layout(device),
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
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // metallic roughness texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // occulusion texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // normal texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                    // material factors
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
    pub min_vertex: Vec3,
    pub max_vertex: Vec3,
}

struct GltfPrimitive {
    vertex_buffer: ImmutableVertexBuffer<MeshVertex>,
    index_buffer: Buffer,
    num_indicies: u32,
    material: PbrMaterial,
    min_vertex: Vec3,
    max_vertex: Vec3,
}

fn load_primitive(
    primitive: Primitive,
    device: &Device,
    queue: &Queue,
    buffers: &[Data],
    texture_content: &[TextureContent],
) -> GltfPrimitive {
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
    let tan_iter = reader
        .read_tangents()
        .unwrap_or_else(|| {
            // TODO: print file path here when utf8 file names are guarenteed
            warn!("Mesh loaded is missing tangents!",);
            gltf::accessor::Iter::Standard(ItemIter::new(&[0; 16], 16))
        })
        .cycle();
    let mut min_vertex = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max_vertex = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    let vertices = reader
        .read_positions()
        .expect("Mesh must have vertecies")
        .zip(reader.read_normals().expect("Mesh must have normals"))
        .zip(tan_iter)
        .zip(tex_coords_iter)
        .map(|(((pos, norm), tan), tex)| {
            let position = Vec3::new(pos[0], pos[1], pos[2]);
            *min_vertex = *min_vertex.min(position);
            *max_vertex = *max_vertex.max(position);
            MeshVertex {
                position,
                normal: Vec3::new(norm[0], norm[1], norm[2]),
                tanget: Vec3::new(tan[0], tan[1], tan[2]),
                tang_handeness: tan[3],
                tex_coords: Vec2::new(tex[0], tex[1]),
            }
        })
        .collect::<Vec<_>>();
    let vertex_buffer = VertexData::allocate_immutable_buffer(device, &vertices);
    let indicies = reader
        .read_indices()
        .expect("Mesh must have indicies")
        .into_u32()
        .collect::<Vec<u32>>();
    let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Index buffer"),
        usage: wgpu::BufferUsages::INDEX,
        contents: bytemuck::cast_slice(&indicies),
    });
    GltfPrimitive {
        vertex_buffer,
        index_buffer,
        material: PbrMaterial::new(device, queue, &primitive.material(), texture_content),
        num_indicies: indicies.len() as u32,
        min_vertex,
        max_vertex,
    }
}

fn load_meshes<'a>(
    nodes: impl Iterator<Item = gltf::Node<'a>>,
    parent_transfrom: Affine3A,
    device: &Device,
    queue: &Queue,
    buffers: &[Data],
    texture_content: &[TextureContent],
) -> Vec<GltfMesh> {
    let mut final_meshes = Vec::with_capacity(nodes.size_hint().0);
    for node in nodes {
        if let Some(mesh) = node.mesh() {
            let local_transform = Mat4::from_cols_array_2d(&node.transform().matrix());
            let local_transform = Affine3A::from_mat4(local_transform);
            let transform = parent_transfrom * local_transform;
            let mut meshes: Vec<GltfMesh> = mesh
                .primitives()
                .par_bridge()
                .map(|primitive| load_primitive(primitive, device, queue, buffers, texture_content))
                .map(|gltf_primitive| GltfMesh {
                    index: mesh.index(),
                    vertex_buffer: gltf_primitive.vertex_buffer,
                    index_buffer: gltf_primitive.index_buffer,
                    num_indicies: gltf_primitive.num_indicies,
                    local_transform: transform,
                    material: gltf_primitive.material,
                    min_vertex: gltf_primitive.min_vertex,
                    max_vertex: gltf_primitive.max_vertex,
                })
                .collect();
            final_meshes.append(&mut meshes);
            let mut children = load_meshes(
                node.children(),
                transform,
                device,
                queue,
                buffers,
                texture_content,
            );
            final_meshes.append(&mut children);
        } else {
            error!("Gltf load error: only mesh nodes are supported");
        }
    }
    final_meshes
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
        let meshes = load_meshes(
            gltf.nodes(),
            Affine3A::IDENTITY,
            device,
            queue,
            &buffers,
            &texture_content,
        );
        // TODO: use utf8 filenames
        let model_min_vertex = meshes
            .iter()
            .min_by(|m1, m2| m1.min_vertex.partial_cmp(&m2.min_vertex).unwrap())
            .map(|mesh| mesh.min_vertex)
            .expect("Can't find min vertex for model");
        // TODO: use utf8 filenames
        let model_max_vertex = meshes
            .iter()
            .min_by(|m1, m2| m1.max_vertex.partial_cmp(&m2.max_vertex).unwrap())
            .map(|mesh| mesh.max_vertex)
            .expect("Can't find max vertex for model");
        info!(
            "Glft Load: {}, Loadtime: {}",
            gltf_load_time,
            start.elapsed().as_secs_f32()
        );
        Ok(GltfModel {
            meshes,
            min_vertex: model_min_vertex,
            max_vertex: model_max_vertex,
        })
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
