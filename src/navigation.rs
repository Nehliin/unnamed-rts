use std::{cmp::Reverse, collections::BinaryHeap};

use glam::Vec3A;

use crate::{
    map_chunk::{ChunkIndex, MapChunk},
    tilemap::Tile,
};

#[derive(Debug, Clone, Copy)]
pub struct FlowTile {
    pub distance: u32,
    pub direction: Vec3A, // TODO: this is the only thing needed
    pos: ChunkIndex,
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
    pub grid: MapChunk<FlowTile>,
    pub target: ChunkIndex,
}

impl FlowField {
    pub fn new(target: ChunkIndex, tilemap: &MapChunk<Tile>) -> Self {
        let distance_grid = generate_distance_field(tilemap, target);
        let flow_grid = generate_flow_direction(&distance_grid, tilemap);
        FlowField {
            grid: flow_grid,
            target,
        }
    }
}

// TODO: more sofistication here
fn temp_cost(n_tile: &Tile, current_tile: &Tile) -> u32 {
    // check tile type etc
    let height_diff = (n_tile.middle_height() - current_tile.middle_height()).abs();
    height_diff as u32 + 1
}

fn generate_distance_field(
    source_tilemap: &MapChunk<Tile>,
    target: ChunkIndex,
) -> MapChunk<FlowTile> {
    // Flood fill alogrithm
    let mut flow_tiles: MapChunk<FlowTile> =
        MapChunk::new(*source_tilemap.transform(), |x, y| FlowTile {
            distance: u32::MAX,
            direction: Vec3A::ZERO,
            pos: ChunkIndex::new(x as i32, y as i32).unwrap(),
        });
    let mut to_visit = BinaryHeap::new();
    to_visit.push(Reverse(FlowTile {
        distance: 0,
        direction: Vec3A::ZERO,
        pos: target,
    }));
    while let Some(Reverse(flow_tile)) = to_visit.pop() {
        let neighbours = flow_tile.pos.strict_neighbours();
        neighbours.for_each(|neighbour| {
            let n_tile = flow_tiles.tile_mut(neighbour);
            if n_tile.distance == u32::MAX {
                n_tile.distance = flow_tile.distance
                    + temp_cost(
                        source_tilemap.tile(neighbour),
                        source_tilemap.tile(flow_tile.pos),
                    );
                to_visit.push(Reverse(*n_tile));
            }
        });
    }
    flow_tiles
}

// TODO rename stuff
fn generate_flow_direction(
    source_flow_grid: &MapChunk<FlowTile>,
    source_tilemap: &MapChunk<Tile>,
) -> MapChunk<FlowTile> {
    let tiles = source_flow_grid
        .iter()
        .map(|tile| {
            let n_tiles = tile.pos.all_neighbours();
            let min_n = n_tiles
                .map(|n_idx| source_flow_grid.tile(n_idx))
                .min_by_key(|n_tile| n_tile.distance)
                .unwrap();
            let min_pos_height = source_tilemap.tile(min_n.pos).middle_height();
            let current_tile_pos_height = source_tilemap.tile(tile.pos).middle_height();
            let (min_n_x, min_n_y) = min_n.pos.to_coords();
            let min_pos = Vec3A::new(min_n_x as f32, min_pos_height, min_n_y as f32);
            let (tile_x, tile_y) = tile.pos.to_coords();
            let current_tile_pos =
                Vec3A::new(tile_x as f32, current_tile_pos_height, tile_y as f32);
            let direction = current_tile_pos - min_pos;
            let direction = direction.normalize_or_zero();
            FlowTile { direction, ..*tile }
        })
        .collect::<Vec<FlowTile>>();
    MapChunk::from_parts(tiles, *source_flow_grid.transform())
}
