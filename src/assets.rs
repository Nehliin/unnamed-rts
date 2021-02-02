use anyhow::Result;
use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    sync::atomic::Ordering,
};
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

impl<T: AssetLoader> Eq for Handle<T> {}

static mut CURRENT_ID: AtomicU32 = AtomicU32::new(0);
// vagely inspired by bevy
pub trait AssetLoader: Sized {
    fn load(path: &PathBuf, device: &Device, queue: &Queue) -> Result<Self>;
    fn extension() -> &'static str;
}

pub struct Assets<T: AssetLoader> {
    storage: HashMap<Handle<T>, T>,
    gpu_load_queue: VecDeque<(Handle<T>, PathBuf)>,
}

impl<T: AssetLoader> Assets<T> {
    pub fn new() -> Assets<T> {
        Assets {
            storage: HashMap::default(),
            gpu_load_queue: VecDeque::default(),
        }
    }

    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<Handle<T>> {
        let pathbuf = PathBuf::from(path.as_ref());

        assert!(
            pathbuf.extension().unwrap() == T::extension(),
            "Unexpected file extension"
        );
        let handle = Handle {
            // Safe because of atomics
            id: unsafe { CURRENT_ID.fetch_add(1, Ordering::AcqRel) },
            _marker: PhantomData::default(),
        };
        self.gpu_load_queue.push_back((handle.clone(), pathbuf));
        Ok(handle)
    }

    #[inline]
    fn clear_load_queue_impl(
        load_queue: &VecDeque<(Handle<T>, PathBuf)>,
        storage: &mut HashMap<Handle<T>, T>,
        device: &Device,
        queue: &Queue,
    ) -> Result<()> {
        for (handle, path_buf) in load_queue.iter() {
            let asset = T::load(path_buf, device, queue)?;
            storage.insert(handle.clone(), asset);
        }
        Ok(())
    }

    pub(crate) fn clear_load_queue(&mut self, device: &Device, queue: &Queue) -> Result<()> {
        Self::clear_load_queue_impl(&self.gpu_load_queue, &mut self.storage, device, queue)?;
        self.gpu_load_queue.clear();
        Ok(())
    }
}
