use std::{cmp::Reverse, collections::BinaryHeap};

use glam::Vec2;

use crate::{
    map_chunk::{ChunkIndex, MapChunk, CHUNK_SIZE},
    tilemap::{Tile, TileType},
};

/// Contains positional info + distance so it can be stored in a BinaryHeap
#[derive(Debug, Clone, Copy)]
struct PositionalDistanceTile {
    distance: u32,
    pos: ChunkIndex,
}

impl PartialEq for PositionalDistanceTile {
    fn eq(&self, other: &Self) -> bool {
        self.distance.eq(&other.distance)
    }
}

impl Eq for PositionalDistanceTile {}

impl PartialOrd for PositionalDistanceTile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.distance.partial_cmp(&other.distance)
    }
}

impl Ord for PositionalDistanceTile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance.cmp(&other.distance)
    }
}

type DistanceField = MapChunk<Option<u32>>;

#[derive(Debug, Clone, Copy)]
pub struct FlowTile {
    pub direction: Vec2,
}

#[derive(Debug)]
pub struct FlowField {
    pub chunk: MapChunk<FlowTile>,
    pub target: ChunkIndex,
}

impl FlowField {
    pub fn new(target: ChunkIndex, tilemap: &MapChunk<Tile>) -> Self {
        let distance_grid = generate_distance_field(tilemap, target);
        let flow_grid = generate_flow_direction(&distance_grid);
        FlowField {
            chunk: flow_grid,
            target,
        }
    }
}

fn calc_distance(n_tile: &Tile, _current_tile: &Tile) -> Option<u32> {
    if n_tile.tile_type != TileType::Flat && n_tile.height_diff > 1 {
        return None;
    }
    // need to get direction to be able to look at tile types
    Some(1)
}

fn generate_distance_field(source_tilemap: &MapChunk<Tile>, target: ChunkIndex) -> DistanceField {
    // Flood fill alogrithm
    let mut distance_field: DistanceField = MapChunk::from_parts(
        vec![None; (CHUNK_SIZE * CHUNK_SIZE) as usize],
        *source_tilemap.transform(), //TODO: This has no meaning here
    );
    // Target have no cost
    *distance_field.tile_mut(target) = Some(0);
    let mut to_visit = BinaryHeap::new();
    to_visit.push(Reverse(PositionalDistanceTile {
        distance: 0,
        pos: target,
    }));
    // Fill the distance field
    while let Some(Reverse(prev_tile)) = to_visit.pop() {
        for neighbour in prev_tile.pos.strict_neighbours() {
            // Distance from neighbour to target
            let n_distance = distance_field.tile_mut(neighbour);
            let dist_to_n = calc_distance(
                source_tilemap.tile(neighbour),
                source_tilemap.tile(prev_tile.pos),
            );
            // If the tile previously hasn't been visited + there exists a path between them
            if let (None, Some(dist_to_n)) = (&n_distance, dist_to_n) {
                // Previously not visited node
                let new_distance = prev_tile.distance + dist_to_n;
                // Update distance field
                *n_distance = Some(new_distance);
                // Continue fill algo based on distance cost
                to_visit.push(Reverse(PositionalDistanceTile {
                    distance: new_distance,
                    pos: neighbour,
                }));
            }
        }
    }
    distance_field
}

fn generate_flow_direction(distance_field: &DistanceField) -> MapChunk<FlowTile> {
    let tiles = DistanceField::indicies()
        .map(|current_idx| {
            // For each tile find neighbour index with lowest cost to target
            if let Some(n_closest) = current_idx
                .all_neighbours()
                .flat_map(|n_idx| distance_field.tile(n_idx).map(|distance| (n_idx, distance)))
                .min_by_key(|(_, distance)| *distance)
                .map(|(n_idx, _)| n_idx)
            {
                let (closest_n_x, closest_n_y) = n_closest.to_coords();
                let closest_pos = Vec2::new(closest_n_x as f32, closest_n_y as f32);
                let (current_x, current_y) = current_idx.to_coords();
                let current_pos = Vec2::new(current_x as f32, current_y as f32);
                let direction = current_pos - closest_pos;
                let direction = direction.normalize_or_zero();
                FlowTile { direction }
            } else {
                // The tile doesn't have a path to the target
                FlowTile {
                    direction: Vec2::ZERO,
                }
            }
        })
        .collect::<Vec<FlowTile>>();
    MapChunk::from_parts(tiles, *distance_field.transform())
}
