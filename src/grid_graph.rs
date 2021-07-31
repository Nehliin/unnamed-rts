use rayon::prelude::*;

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::components::Transform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileGrid<T: Debug + Send> {
    tiles: Vec<T>,
    transform: Transform,
    size: u32,
}

impl<T: Debug + Send> TileGrid<T> {
    /// Init the grid with values
    pub fn new(size: u32, transform: Transform, init: impl Fn(u32, u32) -> T + Sync) -> Self {
        let tiles = (0..size * size)
            .into_par_iter()
            .map(|i| {
                let x = i % size;
                let y = (i - x) / size;
                init(x, y)
            })
            .collect();
        TileGrid {
            tiles,
            transform,
            size,
        }
    }

    pub fn from_parts(side_size: u32, tiles: Vec<T>, transform: Transform) -> Self {
        debug_assert!(
            (side_size * side_size) as usize == tiles.len(),
            "The Grid must be square"
        );
        TileGrid {
            tiles,
            transform,
            size: side_size,
        }
    }

    /// Get the grid transfrom
    #[inline]
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    /// Get one of the grid sides size
    #[inline]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Returns the index to the given tile where (0,0) corresponds to the top left corner
    fn tile_index(&self, x: i32, y: i32) -> Option<usize> {
        if self.valid_position(x, y) {
            // tiles are row ordered
            let size = self.size as i32;
            Some((y * size + x) as usize)
        } else {
            None
        }
    }

    #[inline]
    pub fn valid_position(&self, x: i32, y: i32) -> bool {
        let size = self.size as i32;
        x >= 0 && y >= 0 && size > x && size > y
    }

    /// Get a mutable tile if the position is valid
    #[inline]
    pub fn tile_mut(&mut self, x: i32, y: i32) -> Option<&mut T> {
        if let Some(index) = self.tile_index(x, y) {
            Some(unsafe { self.tiles.get_unchecked_mut(index) })
        } else {
            None
        }
    }

    /// Get a immutable tile if the index is valid
    #[inline]
    pub fn tile_from_index(&self, idx: usize) -> &T {
        self.tiles.get(idx).expect("TileIndex is invalid")
    }

    /// Get a mutable tile if the index is valid
    #[inline]
    pub fn tile_mut_from_index(&mut self, idx: usize) -> &mut T {
        self.tiles.get_mut(idx).expect("TileIndex is invalid")
    }

    /// Get a immutable tile if the position is valid
    #[inline]
    pub fn tile(&self, x: i32, y: i32) -> Option<&T> {
        // SAFETY: The tile index is checked to be inbounds
        self.tile_index(x, y)
            .map(|idx| unsafe { self.tiles.get_unchecked(idx) })
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
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.tiles.iter_mut()
    }

    #[inline]
    pub fn strict_neighbours(&self, x: i32, y: i32) -> Vec<usize> {
        let n_indexes = [
            self.tile_index(x, y + 1),
            self.tile_index(x - 1, y),
            self.tile_index(x + 1, y),
            self.tile_index(x, y - 1),
        ];
        IntoIterator::into_iter(n_indexes)
            .filter_map(|idx| idx)
            .collect()
    }

    #[inline]
    pub fn all_neighbours(&self, x: i32, y: i32) -> Vec<usize> {
        let n_indexes = [
            self.tile_index(x - 1, y + 1),
            self.tile_index(x, y + 1),
            self.tile_index(x + 1, y + 1),
            self.tile_index(x - 1, y),
            self.tile_index(x, y),
            self.tile_index(x + 1, y),
            self.tile_index(x - 1, y - 1),
            self.tile_index(x, y - 1),
            self.tile_index(x + 1, y - 1),
        ];
        IntoIterator::into_iter(n_indexes)
            .filter_map(|idx| idx)
            .collect()
    }
}
