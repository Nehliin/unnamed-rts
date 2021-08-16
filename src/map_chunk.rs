use anyhow::{anyhow, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::components::Transform;

pub const CHUNK_SIZE: i32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkIndex(usize);

impl ChunkIndex {
    pub fn new(x: i32, y: i32) -> Result<Self> {
        if x >= 0 && y >= 0 && CHUNK_SIZE > x && CHUNK_SIZE > y {
            Ok(ChunkIndex((y * CHUNK_SIZE + x) as usize))
        } else {
            Err(anyhow!("Invalid chunk index: x: {}, y: {}", x, y))
        }
    }

    #[inline]
    pub const fn to_coords(&self) -> (i32, i32) {
        let x = self.0 as i32 % CHUNK_SIZE;
        let y = (self.0 as i32 - x) / CHUNK_SIZE;
        (x, y)
    }

    pub fn strict_neighbours(&self) -> impl Iterator<Item = ChunkIndex> {
        let (x, y) = self.to_coords();
        let n_indexes = [
            ChunkIndex::new(x, y + 1),
            ChunkIndex::new(x - 1, y),
            ChunkIndex::new(x + 1, y),
            ChunkIndex::new(x, y - 1),
        ];
        IntoIterator::into_iter(n_indexes).filter_map(|idx| idx.ok())
    }

    pub fn all_neighbours(&self) -> impl Iterator<Item = ChunkIndex> {
        let (x, y) = self.to_coords();
        let n_indexes = [
            ChunkIndex::new(x - 1, y + 1),
            ChunkIndex::new(x, y + 1),
            ChunkIndex::new(x + 1, y + 1),
            ChunkIndex::new(x - 1, y),
            ChunkIndex::new(x, y),
            ChunkIndex::new(x + 1, y),
            ChunkIndex::new(x - 1, y - 1),
            ChunkIndex::new(x, y - 1),
            ChunkIndex::new(x + 1, y - 1),
        ];
        IntoIterator::into_iter(n_indexes).filter_map(|idx| idx.ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapChunk<T: Debug + Send> {
    tiles: Vec<T>,
    transform: Transform,
}

impl<T: Debug + Send> MapChunk<T> {
    /// Init the grid with values
    pub fn new(transform: Transform, init: impl Fn(u32, u32) -> T + Sync) -> Self {
        let tiles = (0..CHUNK_SIZE * CHUNK_SIZE)
            .into_par_iter()
            .map(|i| {
                let x = i % CHUNK_SIZE;
                let y = (i - x) / CHUNK_SIZE;
                init(x as u32, y as u32)
            })
            .collect();
        MapChunk { tiles, transform }
    }

    pub fn from_parts(tiles: Vec<T>, transform: Transform) -> Self {
        debug_assert!(
            (CHUNK_SIZE * CHUNK_SIZE) as usize == tiles.len(),
            "The Chunk must be size: {}, but is {}",
            CHUNK_SIZE,
            tiles.len()
        );
        MapChunk { tiles, transform }
    }

    /// Get the grid transfrom
    #[inline]
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    /// Get a mutable tile from a given ChunkIndex
    #[inline]
    pub fn tile_mut(&mut self, idx: ChunkIndex) -> &mut T {
        // SAFETY: ChunkIndex enforces the index is within bounds
        unsafe { self.tiles.get_unchecked_mut(idx.0) }
    }

    /// Get a immutable tile from a given ChunkIndex
    #[inline]
    pub fn tile(&self, idx: ChunkIndex) -> &T {
        // SAFETY: ChunkIndex enforces the index is within bounds
        unsafe { self.tiles.get_unchecked(idx.0) }
    }

    #[inline]
    pub fn tiles(&self) -> &[T] {
        &self.tiles
    }

    #[inline]
    pub fn tiles_mut(&mut self) -> &mut Vec<T> {
        &mut self.tiles
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.tiles.iter()
    }

    #[inline]
    pub fn indicies() -> impl Iterator<Item = ChunkIndex> {
        (0..(CHUNK_SIZE * CHUNK_SIZE) as usize).map(ChunkIndex)
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.tiles.iter_mut()
    }
}
