use crate::assets::AssetLoader;

use super::{
    simple_texture::SimpleTexture,
    texture::{LoadableTexture, TextureData},
    vertex_buffers::{ImmutableVertexData, MutableVertexData, VertexBuffer, VertexBufferData},
};
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector3};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferAddress, Device, Queue, RenderPass, VertexAttribute, VertexFormat,
};

// Todo make it dynamically growable
const INSTANCE_BUFFER_SIZE: u64 = 16_000;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

impl VertexBuffer for MeshVertex {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute] {
        &[
            VertexAttribute {
                offset: 0,
                format: VertexFormat::Float3,
                shader_location: 0,
            },
            VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                format: VertexFormat::Float3,
                shader_location: 1,
            },
            VertexAttribute {
                offset: (std::mem::size_of::<[f32; 3]>() * 2) as BufferAddress,
                format: VertexFormat::Float2,
                shader_location: 2,
            },
        ]
    }
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
//TODO: The perspective part isn't needed here
pub struct InstanceData {
    model_matrix: [[f32; 4]; 4],
}

impl InstanceData {
    pub fn new(model_matrix: Matrix4<f32>) -> Self {
        InstanceData {
            model_matrix: model_matrix.into(),
        }
    }
}

impl Default for InstanceData {
    fn default() -> Self {
        InstanceData {
            model_matrix: Matrix4::identity().into(),
        }
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
                shader_location: 3,
            },
            VertexAttribute {
                offset: ROW_SIZE,
                format: VertexFormat::Float4,
                shader_location: 4,
            },
            VertexAttribute {
                offset: ROW_SIZE * 2,
                format: VertexFormat::Float4,
                shader_location: 5,
            },
            VertexAttribute {
                offset: ROW_SIZE * 3,
                format: VertexFormat::Float4,
                shader_location: 6,
            },
        ]
    }
}
// TODO: This should be its own texture type
#[derive(Debug)]
pub struct Material {
    pub diffuse_texture: TextureData<SimpleTexture>,
    pub specular_texture: TextureData<SimpleTexture>,
}
#[derive(Debug)]
pub struct Mesh {
    pub vertex_buffer: ImmutableVertexData<MeshVertex>,
    pub index_buffer: Buffer,
    pub material: usize,
    pub num_indexes: u32,
}
#[derive(Debug)]
pub struct Model {
    pub instance_buffer: MutableVertexData<InstanceData>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub min_position: Vector3<f32>,
    pub max_position: Vector3<f32>,
}

impl Model {
    pub fn load(device: &Device, queue: &Queue, path: impl AsRef<Path>) -> Result<Self> {
        let (obj_models, obj_materials) = tobj::load_obj(path.as_ref(), true)?;
        let current_folder = path.as_ref().parent().unwrap_or_else(|| {
            panic!(
                "There must exist a parent folder for object {:?}",
                path.as_ref()
            )
        });

        let materials = obj_materials
            .par_iter()
            .map(|material| {
                let diffuse_path = &material.diffuse_texture;
                let mut specular_path = &material.specular_texture;
                //let ambient_path = material.ambient_texture; TODO: Should this be handled?
                if specular_path.is_empty() {
                    specular_path = &material.diffuse_texture; // TODO: WORST HACK EVER
                }
                let diffuse_texture =
                    SimpleTexture::load_texture(&device, queue, current_folder.join(diffuse_path))
                        .unwrap_or_else(|_| {
                            panic!(
                                "Texture {} not found, should use default in the future",
                                diffuse_path
                            )
                        });
                let specular_texture =
                    SimpleTexture::load_texture(&device, queue, current_folder.join(specular_path))
                        .unwrap_or_else(|_| {
                            panic!(
                                "Texture {} not found, should use default in the future",
                                diffuse_path
                            )
                        });
                Material {
                    diffuse_texture,
                    specular_texture,
                }
            })
            .collect::<Vec<_>>();
        let min_position = [
            AtomicI32::new(i32::MAX),
            AtomicI32::new(i32::MAX),
            AtomicI32::new(i32::MAX),
        ];
        let max_position = [
            AtomicI32::new(i32::MIN),
            AtomicI32::new(i32::MIN),
            AtomicI32::new(i32::MIN),
        ];
        let meshes = obj_models
            .par_iter()
            .map(|m| {
                let vertices = (0..m.mesh.positions.len() / 3)
                    .into_par_iter()
                    .map(|i| {
                        max_position[0]
                            .fetch_max(m.mesh.positions[i * 3].ceil() as i32, Ordering::AcqRel);
                        max_position[1].fetch_max(
                            m.mesh.positions[i * 3 + 1].ceil() as i32,
                            Ordering::AcqRel,
                        );
                        max_position[2].fetch_max(
                            m.mesh.positions[i * 3 + 2].ceil() as i32,
                            Ordering::AcqRel,
                        );
                        min_position[0]
                            .fetch_min(m.mesh.positions[i * 3].floor() as i32, Ordering::AcqRel);
                        min_position[1].fetch_min(
                            m.mesh.positions[i * 3 + 1].floor() as i32,
                            Ordering::AcqRel,
                        );
                        min_position[2].fetch_min(
                            m.mesh.positions[i * 3 + 2].floor() as i32,
                            Ordering::AcqRel,
                        );
                        MeshVertex {
                            position: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                            tex_coords: [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]],
                            normal: [
                                m.mesh.normals[i * 3],
                                m.mesh.normals[i * 3 + 1],
                                m.mesh.normals[i * 3 + 2],
                            ],
                        }
                    })
                    .collect::<Vec<_>>();
                let vertex_buffer = VertexBuffer::allocate_immutable_buffer(device, &vertices);

                let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Index buffer"),
                    usage: wgpu::BufferUsage::INDEX,
                    contents: bytemuck::cast_slice(&m.mesh.indices),
                });
                Mesh {
                    vertex_buffer,
                    index_buffer,
                    material: m.mesh.material_id.unwrap_or(0),
                    num_indexes: m.mesh.indices.len() as u32,
                }
            })
            .collect::<Vec<_>>();

        let instance_buffer_len =
            INSTANCE_BUFFER_SIZE as usize / std::mem::size_of::<InstanceData>();
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        Ok(Model {
            meshes,
            materials,
            instance_buffer,
            min_position: Vector3::new(
                            min_position[0].load(Ordering::Acquire) as f32,
                            min_position[1].load(Ordering::Acquire) as f32,
                            min_position[2].load(Ordering::Acquire) as f32,
                        ),
            max_position: Vector3::new(
                            max_position[0].load(Ordering::Acquire) as f32,
                            max_position[1].load(Ordering::Acquire) as f32,
                            max_position[2].load(Ordering::Acquire) as f32,
                        ),
        })
    }
}
pub trait DrawModel<'b> {
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instance_buffer: &'b MutableVertexData<InstanceData>,
        instances: Range<u32>,
    );

    fn draw_untextured(&mut self, model: &'b Model, instances: Range<u32>);

    fn draw_model_instanced(&mut self, model: &'b Model, instances: Range<u32>);
}

impl<'a, 'b> DrawModel<'b> for RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instance_buffer: &'b MutableVertexData<InstanceData>,
        instances: Range<u32>,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_vertex_buffer(1, instance_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(1, &material.diffuse_texture.bind_group, &[]);
        self.set_bind_group(2, &material.specular_texture.bind_group, &[]);
        self.draw_indexed(0..mesh.num_indexes, 0, instances);
    }

    fn draw_untextured(&mut self, model: &'b Model, instances: Range<u32>) {
        let instance_buffer = &model.instance_buffer;
        for mesh in &model.meshes {
            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_vertex_buffer(1, instance_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_indexed(0..mesh.num_indexes, 0, instances.clone());
        }
    }

    fn draw_model_instanced(&mut self, model: &'b Model, instances: Range<u32>) {
        let instance_buffer = &model.instance_buffer;
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(mesh, material, instance_buffer, instances.clone());
        }
    }
}

impl AssetLoader for Model {
    fn load(path: &PathBuf, device: &Device, queue: &Queue) -> Result<Model> {
        Model::load(device, queue, path)
    }

    fn extension() -> &'static str {
        "obj"
    }
}
