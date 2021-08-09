use glam::Vec3;
use legion::{world::SubWorld, *};
use systems::CommandBuffer;
use unnamed_rts::components::*;
use unnamed_rts::resources::*;

#[system]
pub fn movement(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] time: &Time,
    query: &mut Query<(Entity, &MoveTarget, &mut Velocity, &mut Transform)>,
) {
    query
        .iter_mut(world)
        .for_each(|(entity, move_target, velocity, transform)| {
            let target = move_target.target.into();
            if !transform.matrix.translation.abs_diff_eq(target, 0.01) {
                // very temporary fix here
                let tmp_vel = (target - transform.matrix.translation).normalize() * 3.0;
                velocity.velocity = tmp_vel.into();
                transform.matrix.translation += tmp_vel * time.delta_time;
            } else {
                velocity.velocity = Vec3::splat(0.0);
                command_buffer.remove_component::<MoveTarget>(*entity)
            }
        });
}
