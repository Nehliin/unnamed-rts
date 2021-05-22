use std::{
    hash::Hash,
    ops::{Add, Sub},
};

use glam::{IVec3, Vec3, Vec3A};
use legion::{world::SubWorld, *};
use ordered_float::OrderedFloat;
use pathfinding::{num_traits::ToPrimitive, prelude::*};
use systems::CommandBuffer;
use unnamed_rts::components::*;
use unnamed_rts::resources::*;

use crate::DisplacementBuffer;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathNode {
    pub x: OrderedFloat<f32>,
    pub y: OrderedFloat<f32>,
    pub z: OrderedFloat<f32>,
}

impl PathNode {
    fn abs_diff_eq(&self, node: &PathNode, num: f32) -> bool {
        (self.x - node.x).abs() <= num
            && (self.y - node.y).abs() <= num
            && (self.z - node.z).abs() <= num
    }
}

impl From<IVec3> for PathNode {
    fn from(vec: IVec3) -> Self {
        PathNode {
            x: OrderedFloat::from(vec.x as f32),
            y: OrderedFloat::from(vec.y as f32),
            z: OrderedFloat::from(vec.z as f32),
        }
    }
}

impl pathfinding::num_traits::Zero for PathNode {
    fn zero() -> Self {
        PathNode {
            x: OrderedFloat::from(0.0),
            y: OrderedFloat::from(0.0),
            z: OrderedFloat::from(0.0),
        }
    }

    fn is_zero(&self) -> bool {
        self.eq(&Self::zero())
    }
}

impl Sub for PathNode {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        PathNode {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Add for PathNode {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        PathNode {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

/* impl std::hash::Hash for PathNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
       self.0.as_i32().to_array().hash(state);
    }
}

impl Eq for

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {

    }
} */

pub struct Path {
    inner: Vec<PathNode>,
    index: usize,
}

#[system]
pub fn path_finding(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] buffer: &DisplacementBuffer,
    query: &mut Query<(Entity, &MoveTarget, &mut Transform)>,
) {
    query
        .iter_mut(world)
        .for_each(|(entity, move_target, transform)| {
            // use floats instead
            let start = buffer
                .get(
                    transform.translation.x,
                    transform.translation.z,
                )
                .unwrap();
            /* let start = PathNode {
                x: OrderedFloat::from(start.x as f32),
                y: OrderedFloat::from(start.y as f32),
                z: OrderedFloat::from(start.z as f32),
            }; */
            let end = buffer.get(move_target.target.x, move_target.target.z).unwrap();
            /* let end = PathNode {
                x: OrderedFloat::from(end.x as f32),
                y: OrderedFloat::from(end.y as f32),
                z: OrderedFloat::from(end.z as f32),
            }; */
            if let Some((path, _)) = astar(
                &start,
                |pos| buffer.adjacent(pos.x.0, pos.z.0),
                |pos| {
                    let diff = end - *pos;
                    let tmp = diff.x.0.powi(2) + diff.y.powi(2) + diff.z.powi(2);
                    let dist = tmp.sqrt();
                    dist as u32
                },
                |pos| *pos == end,
            ) {
                println!("Start {:?}, end {:?}", start, end);
                command_buffer.add_component(
                    *entity,
                    Path {
                        inner: path,
                        index: 0,
                    },
                )
            }
            command_buffer.remove_component::<MoveTarget>(*entity)
        });
}

#[system]
pub fn movement(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] time: &Time,
    query: &mut Query<(Entity, &mut Path, &mut Velocity, &mut Transform)>,
) {
    query
        .iter_mut(world)
        .for_each(|(entity, path, velocity, transform)| {
            let current = PathNode {
                x: OrderedFloat::from(transform.translation.x),
                y: OrderedFloat::from(transform.translation.y),
                z: OrderedFloat::from(transform.translation.z),
            };
            let target = path.inner[path.index];
            if !current.abs_diff_eq(&target, 0.1) {
                // very temporary fix here
                let target = Vec3::new(target.x.0, target.y.0, target.z.0);
                velocity.velocity = (target - transform.translation).normalize() * 3.0;
                transform.translation += velocity.velocity * time.delta_time;
            } else {
                path.index += 1;
                if path.index >= path.inner.len() {
                    velocity.velocity = Vec3::splat(0.0);
                    command_buffer.remove_component::<Path>(*entity);
                }
            }
        });
}
