use crate::components::*;
use crate::resources::*;
use glam::Vec3;
use legion::{world::SubWorld, *};
use systems::CommandBuffer;

#[system]
#[read_component(MoveTarget)]
#[write_component(Velocity)]
#[write_component(Transform)]
pub fn movement(world: &mut SubWorld, command_buffer: &mut CommandBuffer, #[resource] time: &Time) {
    let mut query = <(Entity, Read<MoveTarget>, Write<Velocity>, Write<Transform>)>::query();
    query
        .iter_mut(world)
        .for_each(|(entity, move_target, velocity, transform)| {
            if !transform.translation.abs_diff_eq(move_target.target, 0.01) {
                // very temporary fix here
                velocity.velocity = (move_target.target - transform.translation).normalize() * 3.0;
                transform.translation += velocity.velocity * time.delta_time;
            } else {
                velocity.velocity = Vec3::splat(0.0);
                command_buffer.remove_component::<MoveTarget>(*entity)
            }
        });
}
