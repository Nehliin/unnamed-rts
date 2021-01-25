use super::{simple_texture::SimpleTexture, texture::{LoadableTexture, TextureData}, vertex_buffers::{ImmutableVertexData, MutableVertexData, VertexBuffer}};
use anyhow::Result;
use crevice::std140::AsStd140;
use nalgebra::{Matrix4, Vector3};
use std::path::Path;
use wgpu::{Buffer, BufferAddress, Device, Queue, VertexAttributeDescriptor, VertexFormat, util::{BufferInitDescriptor, DeviceExt}};

// Todo make it dynamically growable
const INSTANCE_BUFFER_SIZE: u64 = 16_000;
#[derive(Debug, AsStd140)]
pub struct MeshVertex {
    position: mint::Vector3<f32>,
    normal: mint::Vector3<f32>,
    tex_coords: mint::Vector2<f32>,
}

impl VertexBuffer for MeshVertex {
    const STEP_MODE: wgpu::InputStepMode = wgpu::InputStepMode::Vertex;

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttributeDescriptor] {
         &[
            VertexAttributeDescriptor {
                offset: 0,
                format: VertexFormat::Float3,
                shader_location: 0,
            },
            VertexAttributeDescriptor {
                offset: std::mem::size_of::<Vector3<f32>>() as BufferAddress,
                format: VertexFormat::Float3,
                shader_location: 1,
            },
            VertexAttributeDescriptor {
                offset: (std::mem::size_of::<Vector3<f32>>() * 2) as BufferAddress,
                format: VertexFormat::Float2,
                shader_location: 2,
            },
        ]
    }
}

#[derive(Debug, Clone, AsStd140)]
pub struct InstanceData {
    model_matrix: mint::ColumnMatrix4<f32>,
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

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttributeDescriptor] {
        &[
            VertexAttributeDescriptor {
                offset: 0,
                format: VertexFormat::Float4,
                shader_location: 3,
            },
            VertexAttributeDescriptor {
                offset: ROW_SIZE,
                format: VertexFormat::Float4,
                shader_location: 4,
            },
            VertexAttributeDescriptor {
                offset: ROW_SIZE * 2,
                format: VertexFormat::Float4,
                shader_location: 5,
            },
            VertexAttributeDescriptor {
                offset: ROW_SIZE * 3,
                format: VertexFormat::Float4,
                shader_location: 6,
            },
        ]
    }
}
// TODO: This should be its own texture type
pub struct Material {
    pub diffuse_texture: TextureData<SimpleTexture>,
    pub specular_texture: TextureData<SimpleTexture>,
}

pub struct Mesh {
    pub vertex_buffer: ImmutableVertexData<MeshVertex>,
    pub index_buffer: Buffer,
    pub material: usize,
    pub num_indexes: u32,
}

pub struct Model {
    pub instance_buffer: MutableVertexData<InstanceData>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
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

        let mut materials = Vec::with_capacity(obj_materials.len());

        for material in obj_materials {
            let diffuse_path = material.diffuse_texture;
            let mut specular_path = material.specular_texture;
            //let ambient_path = material.ambient_texture; TODO: Should this be handled?
            if specular_path.is_empty() {
                specular_path = diffuse_path.clone(); // TODO: WORST HACK EVER
            }
            let diffuse_texture =
                SimpleTexture::load_texture(&device, queue, current_folder.join(diffuse_path))?;
            let specular_texture =
                SimpleTexture::load_texture(&device, queue, current_folder.join(specular_path))?;

            materials.push(Material {
                diffuse_texture,
                specular_texture,
            });
        }

        let mut meshes = Vec::new();
        for m in obj_models {
            let mut vertices = Vec::new();
            for i in 0..m.mesh.positions.len() / 3 {
                vertices.push(MeshVertex {
                    position: mint::Vector3 {
                        x: m.mesh.positions[i * 3],
                        y: m.mesh.positions[i * 3 + 1],
                        z: m.mesh.positions[i * 3 + 2],
                    },
                    tex_coords: mint::Vector2 {
                        x: m.mesh.texcoords[i * 2],
                        y: m.mesh.texcoords[i * 2 + 1],
                    },
                    normal: mint::Vector3 {
                        x: m.mesh.normals[i * 3],
                        y: m.mesh.normals[i * 3 + 1],
                        z: m.mesh.normals[i * 3 + 2],
                    },
                });
            }
            let vertex_buffer = VertexBuffer::allocate_immutable_buffer(device, &vertices);

            let indicies = unsafe {
                std::slice::from_raw_parts(
                    m.mesh.indices.as_ptr() as *const u8,
                    m.mesh.indices.len() * 4,
                )
            };

            let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Index buffer"),
                usage: wgpu::BufferUsage::INDEX,
                contents: &indicies
            });

            meshes.push(Mesh {
                vertex_buffer,
                index_buffer,
                material: m.mesh.material_id.unwrap_or(0),
                num_indexes: m.mesh.indices.len() as u32,
            });
        }
        let instance_buffer_len = INSTANCE_BUFFER_SIZE as usize / std::mem::size_of::<InstanceData>();
        println!("INSTANCE BUFFER LEN: {}", instance_buffer_len);
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        Ok(Model {
            meshes,
            materials,
            instance_buffer,
        })
    }
}
