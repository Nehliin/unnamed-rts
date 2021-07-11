use std::{cmp::Reverse, collections::BinaryHeap};

use glam::{IVec2, Vec3A};

use crate::tilemap::TileMap;

#[derive(Debug, Default, Clone, Copy)]
pub struct FlowTile {
    pub distance: u8,
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
    pub tiles: Vec<FlowTile>,
    pub target: IVec2,
}

impl FlowField {
    pub fn new(x: i32, y: i32, tilemap: &TileMap) -> Self {
        // Flood fill alogrithm
        let mut flow_tiles = vec![None; (tilemap.size() * tilemap.size()) as usize];
        let mut to_visit = BinaryHeap::new();
        to_visit.push(Reverse(FlowTile {
            distance: 0,
            direction: Vec3A::ZERO,
            pos: IVec2::new(x, y),
        }));
        while let Some(flow_tile) = to_visit.pop() {
            let neighbours = [
                IVec2::new(flow_tile.0.pos.x - 1, flow_tile.0.pos.y),
                IVec2::new(flow_tile.0.pos.x, flow_tile.0.pos.y - 1),
                IVec2::new(flow_tile.0.pos.x, flow_tile.0.pos.y + 1),
                IVec2::new(flow_tile.0.pos.x + 1, flow_tile.0.pos.y),
            ];

            neighbours.iter().for_each(|pos| {
                if let Some(idx) = tilemap.tile_index(pos.x, pos.y) {
                    // SAFETY: The index is always within bounds if the flow tiles matches the
                    // tilemap size
                    let neighbour = unsafe { flow_tiles.get_unchecked_mut(idx) };
                    if neighbour.is_none() {
                        let new_tile = FlowTile {
                            distance: flow_tile.0.distance + 1,
                            direction: Vec3A::ZERO,
                            pos: *pos,
                        };
                        *neighbour = Some(new_tile);
                        to_visit.push(Reverse(new_tile));
                    }
                }
            });
        }
        let tiles: Vec<FlowTile> = flow_tiles
            .iter()
            .filter_map(|flow_tile| flow_tile.as_ref())
            .map(|flow_tile| {
                let neighbours = [
                    IVec2::new(flow_tile.pos.x - 1, flow_tile.pos.y - 1),
                    IVec2::new(flow_tile.pos.x - 1, flow_tile.pos.y),
                    IVec2::new(flow_tile.pos.x - 1, flow_tile.pos.y + 1),
                    IVec2::new(flow_tile.pos.x, flow_tile.pos.y - 1),
                    IVec2::new(flow_tile.pos.x, flow_tile.pos.y + 1),
                    IVec2::new(flow_tile.pos.x + 1, flow_tile.pos.y - 1),
                    IVec2::new(flow_tile.pos.x + 1, flow_tile.pos.y),
                    IVec2::new(flow_tile.pos.x + 1, flow_tile.pos.y + 1),
                ];
                let (_, min_pos) = neighbours
                    .iter()
                    .filter_map(|n_pos| {
                        if let Some(idx) = tilemap.tile_index(n_pos.x, n_pos.y) {
                            let n_tile = unsafe { flow_tiles.get_unchecked(idx).unwrap() };
                            Some((n_tile.distance, n_tile.pos))
                        } else {
                            None
                        }
                    })
                    .min_by_key(|(n_dist, _)| std::cmp::max(*n_dist as i16 - flow_tile.distance as i16, 0) as u8)
                    .unwrap();
                let tmp = min_pos - flow_tile.pos;
                let direction = Vec3A::new(tmp.x as f32, 0.0, tmp.y as f32);
                FlowTile {
                    direction,
                    ..*flow_tile
                }
            })
            .collect();
        assert_eq!(
            tiles.len(),
            (tilemap.size() * tilemap.size()) as usize,
            "Failed to construct flow field"
        );
        info!("Target: {}", IVec2::new(x, y));
        FlowField {
            tiles,
            target: IVec2::new(x, y),
        }
    }
}
