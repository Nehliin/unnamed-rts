use anyhow::Result;
use fxhash::FxHashMap;
use legion::*;
use log::{error, info};
use std::{collections::VecDeque, marker::PhantomData, sync::atomic::Ordering};
use std::{
    fmt::Debug,
    hash::Hash,
    path::{Path, PathBuf},
};
use std::{hash::Hasher, sync::atomic::AtomicU32};
use wgpu::{Device, Queue};
// This derive only works for T: Debug
#[derive(Debug)]
pub struct Handle<T: AssetLoader> {
    id: u32,
    _marker: PhantomData<T>,
}

impl<T: AssetLoader> Handle<T> {
    pub fn get_id(&self) -> u32 {
        self.id
    }
    // Unsafe because this may not be backed by anything in the asset storage.
    // It doesn't actually risk any memory issues but might break semantics in a bad way
    pub(crate) unsafe fn new_raw_handle(id: u32) -> Self {
        Handle {
            id,
            _marker: PhantomData::default(),
        }
    }
}

/*
 These needs to be manually implemented to avoid
 adding the requirement that T implement these
*/
impl<T: AssetLoader> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: AssetLoader> PartialEq for Handle<T> {
    fn eq(&self, other: &Handle<T>) -> bool {
        self.id == other.id
    }
}

impl<T: AssetLoader> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle {
            id: self.id,
            _marker: PhantomData::default(),
        }
    }
}

impl<T: AssetLoader> Copy for Handle<T> {}

impl<T: AssetLoader> Eq for Handle<T> {}

static mut CURRENT_ID: AtomicU32 = AtomicU32::new(0);
// vagely inspired by bevy
pub trait AssetLoader: Sized {
    fn load(path: &Path, device: &Device, queue: &Queue) -> Result<Self>;
    fn extensions() -> &'static [&'static str];
}

#[derive(Debug)]
pub struct Assets<T: AssetLoader> {
    storage: FxHashMap<Handle<T>, T>,
    gpu_load_queue: VecDeque<(Handle<T>, PathBuf)>,
}

impl<T: AssetLoader> Default for Assets<T> {
    fn default() -> Self {
        Assets {
            storage: FxHashMap::default(),
            gpu_load_queue: VecDeque::default(),
        }
    }
}

impl<T: AssetLoader> Assets<T> {
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    pub fn insert(&mut self, asset: T) -> Handle<T> {
        let handle = Handle {
            // Safe because of atomics
            id: unsafe { CURRENT_ID.fetch_add(1, Ordering::AcqRel) },
            _marker: PhantomData::default(),
        };
        self.storage.insert(handle, asset);
        handle
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<Handle<T>> {
        let mut pathbuf = PathBuf::from("assets");
        pathbuf.push(path.as_ref());

        assert!(
            T::extensions()
                .iter()
                .any(|ext| *ext == pathbuf.extension().unwrap()),
            "Unexpected file extension"
        );
        // hack to check if file exists
        info!("Check if file exists");
        let _ = std::fs::File::open(&pathbuf)?;
        info!("File exist");
        let handle = Handle {
            // Safe because of atomics
            id: unsafe { CURRENT_ID.fetch_add(1, Ordering::AcqRel) },
            _marker: PhantomData::default(),
        };
        self.gpu_load_queue.push_back((handle, pathbuf));
        Ok(handle)
    }

    pub fn load_immediate(
        &mut self,
        path: impl AsRef<Path>,
        device: &Device,
        queue: &Queue,
    ) -> Result<T> {
        let mut pathbuf = PathBuf::from("assets");
        pathbuf.push(path.as_ref());

        assert!(
            T::extensions()
                .iter()
                .any(|ext| *ext == pathbuf.extension().unwrap()),
            "Unexpected file extension"
        );
        T::load(&pathbuf, device, queue)
    }

    #[inline]
    fn clear_load_queue_impl(
        load_queue: &VecDeque<(Handle<T>, PathBuf)>,
        storage: &mut FxHashMap<Handle<T>, T>,
        device: &Device,
        queue: &Queue,
    ) -> Result<()> {
        for (handle, path_buf) in load_queue.iter() {
            info!("Loading: {:?}", path_buf.as_os_str());
            let asset = T::load(path_buf, device, queue)?;
            storage.insert(*handle, asset);
        }
        Ok(())
    }

    fn clear_load_queue(&mut self, device: &Device, queue: &Queue) -> Result<()> {
        Self::clear_load_queue_impl(&self.gpu_load_queue, &mut self.storage, device, queue)?;
        self.gpu_load_queue.clear();
        Ok(())
    }
}

#[system]
pub fn asset_load<T: AssetLoader + 'static>(
    #[resource] device: &Device,
    #[resource] queue: &Queue,
    #[resource] asset_storage: &mut Assets<T>,
) {
    if let Err(err) = asset_storage.clear_load_queue(device, queue) {
        error!(
            "Failed to clear load queue for asset type: {}, with error: {}",
            std::any::type_name::<T>(),
            err
        );
    }
}
