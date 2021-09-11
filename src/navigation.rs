use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{Affine3A, Quat, Vec2, Vec3, Vec3A};

use crate::{
    components::{Transform, Velocity},
    map_chunk::{ChunkIndex, MapChunk, CHUNK_SIZE},
    resources::Time,
    tilemap::{Tile, TileType, TILE_HEIGHT, TILE_WIDTH},
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
    pub direction: Option<Vec2>,
}

#[derive(Debug)]
pub struct FlowField {
    chunk: MapChunk<FlowTile>,
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

    /// Returns normalized direction of the field at the given tile or Vec2::ZERO
    /// Direction is caculated using binary interpolation
    pub fn direction_at_pos(&self, x: f32, y: f32) -> Option<Vec2> {
        let (fx, fy) = (x.floor(), y.floor());

        // Check if the tile have a direction and it's a valid position
        match ChunkIndex::new(fx as i32, fy as i32) {
            Ok(index) if self.chunk.tile(index).direction.is_none() => return None,
            Ok(_) => {}
            Err(_) => return None,
        }

        let (x2, y2) = (fx + 1.0, fy - 1.0);
        let (x1, y1) = (fx - 1.0, fy + 1.0);
        // should Y really be inverted here?
        let denom = (x2 - x1) * (y1 - y2);
        let w11 = (x2 - x) * (y - y2) / denom;
        let w12 = (x2 - x) * (y1 - y) / denom;
        let w21 = (x - x1) * (y - y2) / denom;
        let w22 = (x - x1) * (y1 - y) / denom;

        let f_dir = |x: f32, y: f32| {
            ChunkIndex::new(x as i32, y as i32)
                .map(|idx| self.chunk.tile(idx).direction)
                .ok()
                .flatten()
                .unwrap_or(Vec2::ZERO)
        };
        let dir =
            w11 * f_dir(x1, y1) + w12 * f_dir(x1, y2) + w21 * f_dir(x2, y1) + w22 * f_dir(x2, y2);
        Some(dir.normalize_or_zero())
    }
}

/// Moves a given transfrom (with velocity) along the flow field
/// Used in different systems both server and client side
pub fn movement_impl(
    tilemap: &MapChunk<Tile>,
    flow_field: &FlowField,
    transform: &mut Transform,
    velocity: &mut Velocity,
    time: &Time,
) {
    // Movement along the flow field
    let position = transform.matrix.translation.floor();
    if let Ok(chunk_pos) = ChunkIndex::new(position.x as i32, position.z as i32) {
        if chunk_pos != flow_field.target {
            if let Some(direction) = flow_field.direction_at_pos(position.x, position.z) {
                *velocity.velocity = *-Vec3A::new(direction.x, 0.0, direction.y);
            }
        } else {
            *velocity.velocity = *Vec3::ZERO;
        }
    }
    let (scale, _, translation) = transform.matrix.to_scale_rotation_translation();
    if velocity.velocity != Vec3::ZERO {
        // Set rotation
        *transform.matrix = *Affine3A::from_scale_rotation_translation(
            scale,
            look_at(velocity.velocity.into()),
            translation,
        );
    }
    // Set new position (if valid)
    let offset: Vec3A = Vec3A::splat(4.0) * Vec3A::from(velocity.velocity);
    let new_pos: Vec3A = Vec3A::from(translation) + (offset * time.delta_time());
    let floored_new_pos = new_pos.floor();
    if let Ok(new_chunk_pos) = ChunkIndex::new(floored_new_pos.x as i32, floored_new_pos.z as i32) {
        let translation = &mut transform.matrix.translation;
        *translation = new_pos;
        let tile = tilemap.tile(new_chunk_pos);
        let tile_position = Vec2::new(translation.x % TILE_WIDTH, translation.z % TILE_HEIGHT);
        translation.y = tile.height_at(tile_position);
    }
}

pub fn look_at(direction: Vec3A) -> Quat {
    let mut rotation_axis = Vec3A::Z.cross(direction).normalize_or_zero();
    if rotation_axis.length_squared() < 0.001 {
        rotation_axis = Vec3A::Y;
    }
    let dot = Vec3A::Z.dot(direction);
    let angle = dot.acos();
    Quat::from_axis_angle(rotation_axis.into(), angle)
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
                FlowTile {
                    direction: Some(direction),
                }
            } else {
                // The tile doesn't have a path to the target
                FlowTile { direction: None }
            }
        })
        .collect::<Vec<FlowTile>>();
    MapChunk::from_parts(tiles, *distance_field.transform())
}
