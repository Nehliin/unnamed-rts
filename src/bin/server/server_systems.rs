use glam::{IVec3, Vec3, Vec3A};
use legion::{world::SubWorld, *};
use pathfinding::prelude::*;
use systems::CommandBuffer;
use unnamed_rts::components::*;
use unnamed_rts::resources::*;

use crate::DisplacementBuffer;

struct Path {
    inner: Vec<IVec3>,
    index: usize,
}

#[system]
pub fn path_finding(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] buffer: &DisplacementBuffer,
    query: &mut Query<(Entity, &MoveTarget, &mut Velocity, &mut Transform)>,
) {
    query
        .iter_mut(world)
        .for_each(|(entity, move_target, velocity, transform)| {
            // use floats instead
            let start = buffer.get(
                transform.translation.x as i32,
                transform.translation.y as i32,
            );
            let end = IVec3::new(
                move_target.target.x as i32,
                move_target.target.y as i32,
                move_target.target.z as i32,
            );
            let map_size = 512;
            if let Some((path,_)) = astar(
                &start,
                |pos| buffer.adjacent(pos.x, pos.z),
                |pos| end - *pos,
                |pos| *pos == end,
            ) {
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
            let current = IVec3::new(transform.translation.x as i32, transform.translation.y as i32, transform.translation.z as i32);
            let target = path.inner[path.index];
            if current != target {
                // very temporary fix here
                let target = Vec3::new(target.x as f32, target.y as f32, target.z as f32);
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
