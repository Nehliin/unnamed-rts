use crate::graphics::vertex_buffers::*;
use anyhow::Result;
use glam::*;
use log::info;
use std::{ops::Range, path::Path};
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
    // material
}

#[derive(Debug)]
pub struct GltfModel {
    pub meshes: Vec<GltfMesh>,
    pub instance_buffer: MutableVertexData<InstanceData>,
}

impl GltfModel {
    fn load(device: &Device, queue: &Queue, path: impl AsRef<Path>) -> Result<GltfModel> {
        let (gltf, buffers, images) = gltf::import(path)?;
        let mut meshes = Vec::new();
        // Make it paralell
        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                // let tex_coords_iter = reader.read_tex_coords(0).unwrap();
                let vertices = reader
                    .read_positions()
                    .unwrap()
                    .zip(reader.read_normals().unwrap())
                    .map(|(pos, norm)| MeshVertex {
                        position: Vec3::new(pos[0], pos[1], pos[2]),
                        normal: Vec3::new(norm[0], norm[1], norm[2]),
                        tex_coords: Vec2::splat(0.0),
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
                    index_buffer,
                    num_indicies: indicies.len() as u32,
                });
            }
        }
        let instance_buffer_len = 4000 * std::mem::size_of::<InstanceData>();
        let buffer_data = vec![InstanceData::default(); instance_buffer_len];
        let instance_buffer = VertexBuffer::allocate_mutable_buffer(device, &buffer_data);
        Ok(GltfModel {
            meshes,
            instance_buffer,
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
