use crate::components::Transform;
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

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

/// Describes the Tile edge vertices
#[derive(Debug, Copy, Clone, PartialEq)]
enum TileEdge {
    TopLeft = 0,
    TopMiddle = 1,
    TopRight = 2,
    MiddleLeft = 3,
    MiddleMiddle = 4,
    MiddleRight = 5,
    BottomLeft = 6,
    BottomMiddle = 7,
    BottomRight = 8,
}

/// Describes a tile together with an edge vertex relative to the tile
/// Used to list adjacent tiles to a given corner
#[derive(Debug)]
struct EdgeAdjacentTile {
    adj_edge: TileEdge,
    tile_idx: Option<usize>,
}

/// Describes a edge together with an list of edge adjacent tiles
#[derive(Debug)]
struct EdgeAdjacentList {
    edge: TileEdge,
    list: Vec<EdgeAdjacentTile>,
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
    pub height_diff: i32,
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
            height_diff: 0,
        }
    }

    #[inline]
    fn middle_height(&self) -> f32 {
        self.verticies[TileEdge::MiddleMiddle as usize].position.y
    }

    fn set_height(&mut self, height: f32) {
        // TODO: Don't set type here
        self.tile_type = TileType::Flat;
        self.height_diff = 0;
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

    fn determine_corner_height(&self, adjacent: &[EdgeAdjacentTile], lowered: bool) -> f32 {
        adjacent
            .iter()
            .filter_map(|adj| {
                if let Some(tile_idx) = adj.tile_idx {
                    self.tiles
                        .get(tile_idx)
                        .map(|tile| tile.verticies[adj.adj_edge as usize].position.y)
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
            .expect("Tile max corner height couldn't be determined")
    }

    fn edge_adj_list(&self, edge: TileEdge, tile_idx: usize) -> EdgeAdjacentList {
        assert!(
            edge != TileEdge::MiddleMiddle,
            "Adjacency list for middle vertex have no meaning"
        );
        let (x, y) = self
            .tile_index_to_coords(tile_idx)
            .expect("Tile Index isn't valid");
        let (x, y) = (x as i32, y as i32);
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

        // Get all vertex adjacent tiles to all of the target tile edge
        match edge {
            TileEdge::TopLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        tile_idx: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        tile_idx: left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        tile_idx: top,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        tile_idx: top_left,
                    },
                ],
            },
            TileEdge::TopMiddle => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomMiddle,
                        tile_idx: top,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopMiddle,
                        tile_idx: target,
                    },
                ],
            },
            TileEdge::TopRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        tile_idx: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        tile_idx: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        tile_idx: top_right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        tile_idx: top,
                    },
                ],
            },
            TileEdge::MiddleLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleRight,
                        tile_idx: left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleLeft,
                        tile_idx: target,
                    },
                ],
            },
            TileEdge::MiddleMiddle => {
                error!("No adjacent tiles to middle middle vertex!");
                panic!("TODO");
            }
            TileEdge::MiddleRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleLeft,
                        tile_idx: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleRight,
                        tile_idx: target,
                    },
                ],
            },
            TileEdge::BottomLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        tile_idx: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        tile_idx: bottom_left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        tile_idx: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        tile_idx: left,
                    },
                ],
            },
            TileEdge::BottomMiddle => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopMiddle,
                        tile_idx: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomMiddle,
                        tile_idx: target,
                    },
                ],
            },
            TileEdge::BottomRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        tile_idx: bottom_right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        tile_idx: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        tile_idx: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        tile_idx: target,
                    },
                ],
            },
        }
    }

    // marching squares func
    fn smooth_edges(&mut self, adjacent: &EdgeAdjacentList, lowered: bool) {
        adjacent.list.iter().for_each(|adj| {
            if let Some(tile_idx) = adj.tile_idx {
                // 1. get all corer height cmp with min and max
                // 2. marching squares
                let top_left_height = self.determine_corner_height(
                    &self.edge_adj_list(TileEdge::TopLeft, tile_idx).list,
                    lowered,
                );
                let top_right_height = self.determine_corner_height(
                    &self.edge_adj_list(TileEdge::TopRight, tile_idx).list,
                    lowered,
                );
                let bottom_left_height = self.determine_corner_height(
                    &self.edge_adj_list(TileEdge::BottomLeft, tile_idx).list,
                    lowered,
                );
                let bottom_right_height = self.determine_corner_height(
                    &self.edge_adj_list(TileEdge::BottomRight, tile_idx).list,
                    lowered,
                );

                let mut heights = [
                    top_left_height as i32,
                    top_right_height as i32,
                    bottom_left_height as i32,
                    bottom_right_height as i32,
                ];
                // unwrap because iter isn't empty
                let max_height = *heights.iter().max().unwrap();
                let min_height = *heights.iter().min().unwrap();
                // TODO Clean it up
                let heights = heights
                    .iter_mut()
                    .map(|height| {
                        if *height != max_height {
                            *height = min_height;
                        }
                        *height
                    })
                    .collect::<Vec<_>>();

                let index = if heights[0] == max_height { 1 << 3 } else { 0 }
                    | if heights[1] == max_height { 1 << 2 } else { 0 }
                    | if heights[2] == max_height { 1 << 1 } else { 0 }
                    | if heights[3] == max_height { 1 << 0 } else { 0 };
                let tile_type = [
                    TileType::Flat,
                    TileType::CornerConcaveLT,
                    TileType::CornerConcaveRT,
                    TileType::RampBottom,
                    TileType::CornerConcaveLB,
                    TileType::RampRight,
                    TileType::RampRight,
                    TileType::CornerConvexLT,
                    TileType::CornerConcaveRB,
                    TileType::RampLeft,
                    TileType::RampLeft,
                    TileType::CornerConvexRT,
                    TileType::RampTop,
                    TileType::CornerConvexRB,
                    TileType::CornerConvexLB,
                    TileType::Flat,
                ];

                let new_height = self.determine_corner_height(&adjacent.list, lowered);
                if let Some(tile) = self.tiles.get_mut(tile_idx) {
                    tile.verticies[adj.adj_edge as usize].position.y = new_height;
                    tile.tile_type = tile_type[index];
                    tile.height_diff = max_height - min_height;
                } else {
                    error!(
                        "Tile index for adjacent tile isn't valid, index: {}",
                        tile_idx
                    );
                }
            }
        })
    }

    pub fn set_tile_height(&mut self, x: u32, y: u32, height: f32) {
        // Update
        let is_lowered;
        if let Some(tile) = self.tile_mut(x, y) {
            is_lowered = height < tile.middle_height();
            tile.set_height(height);
        } else {
            return;
        }
        if let Some(tile_idx) = self.tile_index(x as i32, y as i32) {
            self.smooth_edges(&self.edge_adj_list(TileEdge::TopLeft, tile_idx), is_lowered);
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::TopRight, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::BottomLeft, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::BottomRight, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::MiddleLeft, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::MiddleRight, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::TopMiddle, tile_idx),
                is_lowered,
            );
            self.smooth_edges(
                &self.edge_adj_list(TileEdge::BottomMiddle, tile_idx),
                is_lowered,
            );
        } else {
            error!("Invalid tile coordinates given to set_tile_height");
        }
    }

    pub fn tile_mut(&mut self, x: u32, y: u32) -> Option<&mut Tile> {
        if let Some(index) = self.tile_index(x as i32, y as i32) {
            self.tiles.get_mut(index)
        } else {
            None
        }
    }

    pub fn tile(&self, x: u32, y: u32) -> Option<&Tile> {
        if let Some(index) = self.tile_index(x as i32, y as i32) {
            self.tiles.get(index)
        } else {
            None
        }
    }

    /// Returns the index to the given tile where (0,0) corresponds to the top left corner
    #[inline]
    pub fn tile_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            None
        } else {
            let (x, y) = (x as u32, y as u32);
            // tiles are column ordered
            Some((x * self.size + y) as usize)
        }
    }

    /// Return the x,y coordinates from a given tile index
    #[inline]
    pub fn tile_index_to_coords(&self, tile_idx: usize) -> Option<(u32, u32)> {
        if self.tiles.len() <= tile_idx {
            None
        } else {
            let y = tile_idx as u32 % self.size;
            let x = (tile_idx as u32 - y) / self.size;
            Some((x, y))
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn should_revers_tile_index() {
        let map = TileMap::new("TestMap".into(), 256, Transform::default());
        let idx = map.tile_index(13, 7).unwrap();
        let (x, y) = map.tile_index_to_coords(idx).unwrap();
        assert_eq!((x, y), (13, 7));

        let idx = map.tile_index(0, 255).unwrap();
        let (x, y) = map.tile_index_to_coords(idx).unwrap();
        assert_eq!((x, y), (0, 255));

        let idx = map.tile_index(255, 255).unwrap();
        let (x, y) = map.tile_index_to_coords(idx).unwrap();
        assert_eq!((x, y), (255, 255));

        let idx = map.tile_index(255, 0).unwrap();
        let (x, y) = map.tile_index_to_coords(idx).unwrap();
        assert_eq!((x, y), (255, 0));
    }

    #[test]
    fn should_not_reverse_invalid_index() {
        let map = TileMap::new("TestMap".into(), 256, Transform::default());
        assert!(map.tile_index_to_coords(256 * 256).is_none())
    }
}
