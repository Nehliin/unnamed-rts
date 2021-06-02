use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

use crate::components::Transform;
#[repr(C)]
#[derive(Debug, Default, Pod, Zeroable, Deserialize, Serialize, Clone, Copy)]
pub struct TileVertex {
    position: Vec3,
    uv: Vec2,
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TileType {
    Flat,
    RampTop,
    RampBottom,
    RampRight,
    RampLeft,
    CornerConcaveRT,
    CornerConvexRT,
    CornerConcaveLT,
    CornerConvexLT,
    CornerConcaveRB,
    CornerConvexRB,
    CornerConcaveLB,
    CornerConvexLB,
}

impl Default for TileType {
    fn default() -> Self {
        TileType::Flat
    }
}

const VERTECIES_PER_TILE: usize = 9;
const INDICIES_PER_TILE: usize = 24;
pub const TILE_WIDTH: f32 = 2.0;
pub const TILE_HEIGHT: f32 = 2.0;
// Z coords per tile?

#[derive(Debug, Copy, Clone)]
/// Describes the Tile edge vertices
enum TileEdge {
    TopLeft = 0,
    TopMiddle = 1,
    TopRight = 2,
    MiddleLeft = 3,
    MiddleRight = 5,
    BottomLeft = 6,
    BottomMiddle = 7,
    BottomRight = 8,
}

#[derive(Debug)]
/// Describes a tile together with an edge vertex relative to the tile
/// Used to list adjacent tiles to a given corner
struct EdgeAdjacentTile {
    edge: TileEdge,
    tile_idx: Option<usize>,
}

/*
 *   *------*------*
 *   |\   1 | 3   /|
 *   |  \   |   /  |
 *   | 2  \ | /  4 |
 *   *------*------*
 *   | 5  / | \  7 |
 *   |  /   |   \  |
 *   |/  6  | 8   \|
 *   *------*------*
 */
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Tile {
    pub verticies: [TileVertex; VERTECIES_PER_TILE],
    pub indicies: [u32; INDICIES_PER_TILE],
    pub tile_type: TileType,
    pub base_height: f32,
    // ramp height
}

impl Tile {
    pub fn new(top_left_corner: Vec3, start_idx: u32, size: u32) -> Self {
        let height = size as f32 * TILE_HEIGHT;
        let width = size as f32 * TILE_WIDTH;
        // TODO change order of this to make indices closer to each other
        let verticies = [
            // Top left
            TileVertex {
                position: top_left_corner,
                uv: Vec2::new(top_left_corner.x / width, top_left_corner.z / height),
            },
            // Top middle
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH / 2.0,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).x / width,
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).z / height,
                ),
            },
            // Top right
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH).x / width,
                    (top_left_corner + Vec3::X * TILE_WIDTH).z / height,
                ),
            },
            // Middle left
            TileVertex {
                position: top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0,
                uv: Vec2::new(
                    (top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0).x / width,
                    (top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0).z / height,
                ),
            },
            // Middle middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0)).x
                        / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0)).z
                        / height,
                ),
            },
            // Middle right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0)).z / height,
                ),
            },
            // Bottom left
            TileVertex {
                position: top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
            // Bottom middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
            // Bottom right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
        ];
        #[rustfmt::skip]
        let indicies = [
            // 1
            start_idx, start_idx + 4, start_idx + 1,
            // 2
            start_idx, start_idx + 3, start_idx + 4,
            // 3
            start_idx + 1, start_idx + 4, start_idx + 2,
            // 4
            start_idx + 2, start_idx + 4, start_idx + 5,
            // 5
            start_idx + 4, start_idx + 3, start_idx + 6,
            // 6
            start_idx + 6, start_idx + 7, start_idx + 4,
            // 7
            start_idx + 4, start_idx + 8, start_idx + 5,
            // 8
            start_idx + 4, start_idx + 7, start_idx + 8,
        ];
        Tile {
            verticies,
            indicies,
            tile_type: TileType::Flat,
            base_height: 0.0,
        }
    }

    fn set_height(&mut self, height: f32) {
        self.base_height = height;
        self.verticies
            .iter_mut()
            .for_each(|vert| vert.position.y = height);
    }
}

pub fn generate_tiles(size: u32) -> Vec<Tile> {
    let mut tiles = Vec::with_capacity((size * size) as usize);
    let mut index = 0;
    for x in 0..size {
        for z in 0..size {
            tiles.push(Tile::new(
                Vec3::new(TILE_WIDTH * x as f32, 0.0, TILE_HEIGHT * z as f32),
                index,
                size,
            ));
            index += VERTECIES_PER_TILE as u32;
        }
    }
    tiles
}

// Use const generics here for size perhaps
#[derive(Debug, Serialize, Deserialize)]
pub struct TileMap {
    name: String,
    tiles: Vec<Tile>,
    size: u32,
    transform: Transform,
}

impl TileMap {
    pub fn new(name: String, size: u32, transform: Transform) -> Self {
        let tiles = generate_tiles(size);
        TileMap {
            name,
            tiles,
            size,
            transform,
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let map_file = std::fs::File::open(path)?;
        let map = bincode::deserialize_from(map_file)?;
        Ok(map)
    }

    /// Get a reference to the tile map's name.
    #[inline(always)]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the tile map's size.
    #[inline(always)]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get a reference to the tile map's transform.
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    fn smooth_edges(&mut self, adjacent: &[EdgeAdjacentTile], lowered: bool) {
        // itertools
        let new_height = adjacent
            .iter()
            .filter_map(|adj| {
                if let Some(tile_idx) = adj.tile_idx {
                    self.tiles
                        .get(tile_idx)
                        .map(|tile| tile.verticies[adj.edge as usize].position.y)
                } else {
                    None
                }
            })
            .max_by(|x, y| {
                if lowered {
                    (*y as i32).cmp(&(*x as i32))
                } else {
                    (*x as i32).cmp(&(*y as i32))
                }
            })
            .expect("Tile max corner height couldn't be determined");
        adjacent.iter().for_each(|adj| {
            if let Some(tile_idx) = adj.tile_idx {
                if let Some(height) = self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[adj.edge as usize].position.y)
                {
                    *height = new_height;
                }
            }
        })
    }

    pub fn set_tile_height(&mut self, x: u32, y: u32, height: f32) {
        // Update
        let is_lowered;
        if let Some(tile) = self.tile_mut(x, y) {
            is_lowered = height < tile.base_height;
            tile.set_height(height);
        } else {
            return;
        }
        let (x, y) = (x as i32, y as i32);
        // marching squares here later on
        let target = self.tile_index(x, y);
        let left = self.tile_index(x - 1, y);
        let right = self.tile_index(x + 1, y);
        let bottom = self.tile_index(x, y + 1);
        // remember 0,0 is the top left corner
        let top = self.tile_index(x, y - 1);
        let top_left = self.tile_index(x - 1, y - 1);
        let top_right = self.tile_index(x + 1, y - 1);
        let bottom_left = self.tile_index(x - 1, y + 1);
        let bottom_right = self.tile_index(x + 1, y + 1);

        // List all vertex adjacent tiles to all of the target tiles' edges
        // top_left_adj refers to all tiles adjacent to the target tiles top_left corner vertex
        let top_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: top,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: top_left,
            },
        ];
        let top_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: top_right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: top,
            },
        ];
        let bottom_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: bottom_left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: left,
            },
        ];
        let bottom_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: bottom_right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: target,
            },
        ];
        let middle_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::MiddleRight,
                tile_idx: left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::MiddleLeft,
                tile_idx: target,
            },
        ];
        let middle_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::MiddleLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::MiddleRight,
                tile_idx: target,
            },
        ];
        let middle_top_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::BottomMiddle,
                tile_idx: top,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopMiddle,
                tile_idx: target,
            },
        ];
        let middle_bottom_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopMiddle,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomMiddle,
                tile_idx: target,
            },
        ];
        self.smooth_edges(top_left_adj, is_lowered);
        self.smooth_edges(top_right_adj, is_lowered);
        self.smooth_edges(bottom_left_adj, is_lowered);
        self.smooth_edges(bottom_right_adj, is_lowered);
        self.smooth_edges(middle_left_adj, is_lowered);
        self.smooth_edges(middle_right_adj, is_lowered);
        self.smooth_edges(middle_top_adj, is_lowered);
        self.smooth_edges(middle_bottom_adj, is_lowered);
    }

    pub fn tile_mut(&mut self, x: u32, y: u32) -> Option<&mut Tile> {
        if let Some(index) = self.tile_index(x as i32, y as i32) {
            self.tiles.get_mut(index)
        } else {
            None
        }
    }

    #[inline(always)]
    /// Returns the index to the given tile where (0,0) corresponds to the top left corner
    pub fn tile_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            None
        } else {
            let (x, y) = (x as u32, y as u32);
            // tiles are column ordered
            Some((x * self.size + y) as usize)
        }
    }

    /// Get a reference to the tile map's tiles.
    #[inline(always)]
    pub fn tiles(&self) -> &[Tile] {
        self.tiles.as_slice()
    }

    /// Get mutable a reference to the tile map's tiles.
    #[inline(always)]
    pub fn tiles_mut(&mut self) -> &mut [Tile] {
        &mut self.tiles
    }

    #[inline(always)]
    pub fn set_tiles(&mut self, tiles: Vec<Tile>) {
        self.tiles = tiles;
    }
}
