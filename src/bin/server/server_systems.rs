use legion::{world::SubWorld, *};
use unnamed_rts::components::*;
use unnamed_rts::navigation::{FlowField, movement_impl};
use unnamed_rts::resources::*;
use unnamed_rts::tilemap::TileMap;

#[system]
pub fn movement(
    world: &mut SubWorld,
    #[resource] tilemap: &TileMap,
    #[resource] time: &Time,
    query: &mut Query<(Entity, &FlowField, &mut Transform, &mut Velocity)>,
) {
    query.for_each_mut(world, |(_entity, flow_field, transform, velocity)| {
        // Movement along the flow field
        movement_impl(&tilemap.chunk, flow_field, transform, velocity, time);
    });
}
