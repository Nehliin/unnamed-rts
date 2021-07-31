use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{IVec2, Vec3A};

use crate::{grid_graph::TileGrid, tilemap::Tile};

#[derive(Debug, Default, Clone, Copy)]
pub struct FlowTile {
    pub distance: u32,
    direction: Vec3A,
    pos: IVec2,
}

impl PartialEq for FlowTile {
    fn eq(&self, other: &Self) -> bool {
        self.distance.eq(&other.distance)
    }
}

impl Eq for FlowTile {}

impl PartialOrd for FlowTile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.distance.partial_cmp(&other.distance)
    }
}

impl Ord for FlowTile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance.cmp(&other.distance)
    }
}

#[derive(Debug)]
pub struct FlowField {
    pub grid: TileGrid<FlowTile>,
    pub target: IVec2,
}

impl FlowField {
    pub fn new(x: i32, y: i32, tilemap: &TileGrid<Tile>) -> Self {
        let distance_grid = generate_distance_field(tilemap, IVec2::new(x, y));
        let flow_grid = generate_flow_direction(&distance_grid);

        FlowField {
            grid: flow_grid,
            target: IVec2::new(x, y),
        }
    }
}

fn generate_distance_field(source_tilemap: &TileGrid<Tile>, target: IVec2) -> TileGrid<FlowTile> {
    // Flood fill alogrithm
    let mut flow_tiles: TileGrid<FlowTile> = TileGrid::new(
        source_tilemap.size(),
        *source_tilemap.transform(),
        |x, y| FlowTile {
            distance: u32::MAX,
            direction: Vec3A::ZERO,
            pos: IVec2::new(x as i32, y as i32),
        },
    );
    let mut to_visit = BinaryHeap::new();
    to_visit.push(Reverse(FlowTile {
        distance: 0,
        direction: Vec3A::ZERO,
        pos: target,
    }));
    while let Some(flow_tile) = to_visit.pop() {
        let neighbours = flow_tiles.strict_neighbours(flow_tile.0.pos.x, flow_tile.0.pos.y);
        neighbours.iter().for_each(|neighbour| {
            let n_tile = flow_tiles.tile_mut_from_index(*neighbour);
            if n_tile.distance == u32::MAX {
                n_tile.distance = flow_tile.0.distance + 1;
                to_visit.push(Reverse(*n_tile));
            }
        });
    }
    flow_tiles
}

fn generate_flow_direction(source_grid: &TileGrid<FlowTile>) -> TileGrid<FlowTile> {
    let tiles = source_grid
        .iter()
        .map(|tile| {
            let n_tiles = source_grid.all_neighbours(tile.pos.x, tile.pos.y);
            let min_n = n_tiles
                .iter()
                .map(|n_idx| source_grid.tile_from_index(*n_idx))
                .min_by_key(|n_tile| {
                    std::cmp::max(n_tile.distance as i32 - tile.distance as i32, 0)
                })
                .unwrap();
            let direction = min_n.pos - tile.pos;
            let direction = Vec3A::new(direction.x as f32, 0.0, direction.y as f32);
            FlowTile { direction, ..*tile }
        })
        .collect::<Vec<FlowTile>>();
    TileGrid::from_parts(source_grid.size(), tiles, *source_grid.transform())
}
