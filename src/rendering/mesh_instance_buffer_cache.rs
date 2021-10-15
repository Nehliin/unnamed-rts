use super::{
    gltf::{GltfMesh, GltfModel, InstanceData},
    vertex_buffers::{MutableVertexBuffer, VertexData},
};
use crate::{
    assets::{Assets, Handle},
    components::Transform,
};
use fxhash::FxHashMap;

#[derive(Debug, Default)]
pub struct MeshInstanceBufferCache {
    cache: FxHashMap<(usize, Handle<GltfModel>), MutableVertexBuffer<InstanceData>>,
}

impl MeshInstanceBufferCache {
    pub fn evict_stale(&mut self, asset_storage: &Assets<GltfModel>) {
        // TODO: fix this when bump allocation is added a bit messy now
        self.cache
            .retain(|(_, handle), _| asset_storage.get(handle).is_some());
        for (_, buffer) in self.cache.iter_mut() {
            buffer.reset();
        }
    }

    pub fn put(
        &mut self,
        device: &wgpu::Device,
        model_handle: &Handle<GltfModel>,
        model: &GltfModel,
        transform_fn: impl Fn(&GltfMesh) -> Transform,
    ) {
        for mesh in &model.meshes {
            let key = (*mesh.index(), *model_handle);
            let buffer = self
                .cache
                .entry(key)
                .or_insert_with(|| VertexData::allocate_mutable_buffer_with_size(device, 32));
            let new_transform = transform_fn(mesh);
            buffer.write(InstanceData::new(&new_transform));
        }
    }

    pub fn get(
        &self,
        key: &(usize, Handle<GltfModel>),
    ) -> Option<&MutableVertexBuffer<InstanceData>> {
        self.cache.get(key)
    }

    pub fn iter_mut<'a>(
        &mut self,
        asset_storage: &'a Assets<GltfModel>,
    ) -> impl Iterator<Item = (&'a GltfMesh, &mut MutableVertexBuffer<InstanceData>)> {
        self.cache
            .iter_mut()
            .flat_map(move |((index, model_handle), buffer)| {
                asset_storage
                    .get(model_handle)
                    .map(|model| model.meshes.iter().find(|mesh| mesh.index() == index))
                    .flatten()
                    .map(|mesh| (mesh, buffer))
            })
    }
}
