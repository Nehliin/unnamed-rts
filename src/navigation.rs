use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{IVec2, Vec3A};

use crate::{grid_graph::TileGrid, tilemap::Tile};

#[derive(Debug, Default, Clone, Copy)]
pub struct FlowTile {
    pub distance: u32,
    pub direction: Vec3A, // TODO: this is the only thing needed
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
        let flow_grid = generate_flow_direction(&distance_grid, &tilemap);

        FlowField {
            grid: flow_grid,
            target: IVec2::new(x, y),
        }
    }
}

fn temp_cost(n_tile: &Tile, current_tile: &Tile) -> u32 {
    // check tile type etc
    let height_diff = (n_tile.middle_height() - current_tile.middle_height()).abs();
    height_diff as u32 + 1
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
                n_tile.distance = flow_tile.0.distance
                    + temp_cost(
                        source_tilemap.tile_from_index(*neighbour),
                        source_tilemap
                            .tile(flow_tile.0.pos.x, flow_tile.0.pos.y)
                            .unwrap(),
                    );
                to_visit.push(Reverse(*n_tile));
            }
        });
    }
    flow_tiles
}

// TODO rename stuff
fn generate_flow_direction(
    source_flow_grid: &TileGrid<FlowTile>,
    source_tilemap: &TileGrid<Tile>,
) -> TileGrid<FlowTile> {
    let tiles = source_flow_grid
        .iter()
        .map(|tile| {
            let n_tiles = source_flow_grid.all_neighbours(tile.pos.x, tile.pos.y);
            let min_n = n_tiles
                .iter()
                .map(|n_idx| source_flow_grid.tile_from_index(*n_idx))
                .min_by_key(|n_tile| n_tile.distance)
                .unwrap();
            let min_pos_height = source_tilemap
                .tile(min_n.pos.x, min_n.pos.y)
                .unwrap()
                .middle_height();
            let current_tile_pos_height = source_tilemap
                .tile(tile.pos.x, tile.pos.y)
                .unwrap()
                .middle_height();
            let min_pos = Vec3A::new(min_n.pos.x as f32, min_pos_height, min_n.pos.y as f32);
            let current_tile_pos = Vec3A::new(
                tile.pos.x as f32,
                current_tile_pos_height,
                tile.pos.y as f32,
            );
            let direction = current_tile_pos - min_pos;
            let direction = direction.normalize_or_zero();
            FlowTile { direction, ..*tile }
        })
        .collect::<Vec<FlowTile>>();
    TileGrid::from_parts(
        source_flow_grid.size(),
        tiles,
        *source_flow_grid.transform(),
    )
}
