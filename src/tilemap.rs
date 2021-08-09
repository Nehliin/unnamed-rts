use crate::{components::Transform, grid_graph::TileGrid};
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Vec2, Vec3};
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
pub const TILE_WIDTH: f32 = 1.0;
pub const TILE_HEIGHT: f32 = 1.0;
// Z coords per tile?

/// Describes the Tile edge vertices
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TileEdge {
    TopLeft = 0,
    TopMiddle = 1,
    TopRight = 2,
    MiddleLeft = 3,
    MiddleRight = 5,
    BottomLeft = 6,
    BottomMiddle = 7,
    BottomRight = 8,
}

const TILE_MIDDLE_VERTEX_INDEX: usize = 4;

/// Describes a tile together with an edge vertex relative to the tile
/// Used to list adjacent tiles to a given corner
#[derive(Debug)]
struct EdgeAdjacentTile {
    adj_edge: TileEdge,
    pos: IVec2,
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
    pub fn new(top_left_corner: Vec3, start_idx: u32, grid_size: u32) -> Self {
        let height = grid_size as f32 * TILE_HEIGHT;
        let width = grid_size as f32 * TILE_WIDTH;
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
    pub fn middle_height(&self) -> f32 {
        self.verticies[TILE_MIDDLE_VERTEX_INDEX].position.y
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

pub fn generate_grid(size: u32, transform: Transform) -> TileGrid<Tile> {
    let grid = TileGrid::new(size, transform, |x, y| {
        let tile = Tile::new(
            Vec3::new(TILE_WIDTH * x as f32, 0.0, TILE_HEIGHT * y as f32),
            (y * size + x) * VERTECIES_PER_TILE as u32,
            size,
        );
        tile
    });
    grid
}

// Use const generics here for size perhaps
#[derive(Debug, Serialize, Deserialize)]
pub struct TileMap {
    pub name: String,
    pub grid: TileGrid<Tile>,
}

impl TileMap {
    pub fn new(name: String, size: u32, transform: Transform) -> Self {
        TileMap {
            name,
            grid: generate_grid(size, transform),
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let map_file = std::fs::File::open(path)?;
        let map = bincode::deserialize_from(map_file)?;
        Ok(map)
    }

    fn determine_corner_height(&self, adjacent: &[EdgeAdjacentTile], lowered: bool) -> f32 {
        adjacent
            .iter()
            .filter_map(|adj| {
                self.grid
                    .tile(adj.pos.x, adj.pos.y)
                    .map(|tile| tile.verticies[adj.adj_edge as usize].position.y)
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

    fn edge_adj_list(&self, edge: TileEdge, x: i32, y: i32) -> EdgeAdjacentList {
        let target = IVec2::new(x, y);
        let left = IVec2::new(x - 1, y);
        let right = IVec2::new(x + 1, y);
        let bottom = IVec2::new(x, y + 1);
        // remember 0,0 is the top left corner
        let top = IVec2::new(x, y - 1);
        let top_left = IVec2::new(x - 1, y - 1);
        let top_right = IVec2::new(x + 1, y - 1);
        let bottom_left = IVec2::new(x - 1, y + 1);
        let bottom_right = IVec2::new(x + 1, y + 1);

        // Get all vertex adjacent tiles to all of the target tile edge
        match edge {
            TileEdge::TopLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        pos: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        pos: left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        pos: top,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        pos: top_left,
                    },
                ],
            },
            TileEdge::TopMiddle => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomMiddle,
                        pos: top,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopMiddle,
                        pos: target,
                    },
                ],
            },
            TileEdge::TopRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        pos: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        pos: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        pos: top_right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        pos: top,
                    },
                ],
            },
            TileEdge::MiddleLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleRight,
                        pos: left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleLeft,
                        pos: target,
                    },
                ],
            },
            TileEdge::MiddleRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleLeft,
                        pos: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::MiddleRight,
                        pos: target,
                    },
                ],
            },
            TileEdge::BottomLeft => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        pos: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        pos: bottom_left,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        pos: target,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        pos: left,
                    },
                ],
            },
            TileEdge::BottomMiddle => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopMiddle,
                        pos: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomMiddle,
                        pos: target,
                    },
                ],
            },
            TileEdge::BottomRight => EdgeAdjacentList {
                edge,
                list: vec![
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopLeft,
                        pos: bottom_right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::TopRight,
                        pos: bottom,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomLeft,
                        pos: right,
                    },
                    EdgeAdjacentTile {
                        adj_edge: TileEdge::BottomRight,
                        pos: target,
                    },
                ],
            },
        }
    }

    // marching squares func
    fn smooth_edges(&mut self, adjacent: &EdgeAdjacentList, lowered: bool) {
        for adj in adjacent.list.iter() {
            if !self.grid.valid_position(adj.pos.x, adj.pos.y) {
                continue;
            }
            // 1. get all corer height cmp with min and max
            // 2. marching squares
            let top_left_height = self.determine_corner_height(
                &self
                    .edge_adj_list(TileEdge::TopLeft, adj.pos.x, adj.pos.y)
                    .list,
                lowered,
            );
            let top_right_height = self.determine_corner_height(
                &self
                    .edge_adj_list(TileEdge::TopRight, adj.pos.x, adj.pos.y)
                    .list,
                lowered,
            );
            let bottom_left_height = self.determine_corner_height(
                &self
                    .edge_adj_list(TileEdge::BottomLeft, adj.pos.x, adj.pos.y)
                    .list,
                lowered,
            );
            let bottom_right_height = self.determine_corner_height(
                &self
                    .edge_adj_list(TileEdge::BottomRight, adj.pos.x, adj.pos.y)
                    .list,
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
            if let Some(tile) = self.grid.tile_mut(adj.pos.x, adj.pos.y) {
                tile.verticies[adj.adj_edge as usize].position.y = new_height;
                tile.tile_type = tile_type[index];
                tile.height_diff = max_height - min_height;
            } else {
                error!(
                    "Tile index for adjacent tile isn't valid, index: {}",
                    adj.pos
                );
            }
        }
    }

    pub fn set_tile_height(&mut self, x: i32, y: i32, height: f32) {
        // Update
        let is_lowered;
        if let Some(tile) = self.grid.tile_mut(x, y) {
            is_lowered = height < tile.middle_height();
            tile.set_height(height);
        } else {
            error!("Invalid tile coordinates given to set_tile_height");
            return;
        }
        self.smooth_edges(&self.edge_adj_list(TileEdge::TopLeft, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::TopRight, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::BottomLeft, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::BottomRight, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::MiddleLeft, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::MiddleRight, x, y), is_lowered);
        self.smooth_edges(&self.edge_adj_list(TileEdge::TopMiddle, x, y), is_lowered);
        self.smooth_edges(
            &self.edge_adj_list(TileEdge::BottomMiddle, x, y),
            is_lowered,
        );
    }
}
