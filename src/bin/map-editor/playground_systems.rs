use glam::{Affine3A, Vec3, Vec3A};
use itertools::Itertools;
use legion::{systems::CommandBuffer, world::SubWorld, *};
use unnamed_rts::{
    assets::{Assets, Handle},
    components::{Selectable, Transform, Velocity},
    input::{CursorPosition, MouseButtonState},
    map_chunk::{ChunkIndex, MapChunk, CHUNK_SIZE},
    navigation::{self, FlowField},
    rendering::{camera::Camera, drawable_tilemap::*, gltf::GltfModel},
    resources::{Time, WindowSize},
    tilemap::{Tile, TILE_HEIGHT, TILE_WIDTH},
};
use winit::event::MouseButton;

//TODO: Associate each selected entity with a group which in turn gets assigned a flowfield
#[system]
#[allow(clippy::too_many_arguments)]
pub fn move_action(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] map_assets: &Assets<DrawableTileMap>,
    #[resource] map_handle: &Handle<DrawableTileMap>,
    query: &mut Query<(Entity, &Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Right) {
        query.for_each(world, |(entity, selectable)| {
            if selectable.is_selected {
                let ray = camera.raycast(mouse_pos, window_size);
                // check intersection with the regular ground plan
                let normal = Vec3A::Y;
                let denominator = normal.dot(ray.direction);
                if denominator.abs() > 0.0001 {
                    // it isn't parallel to the plane
                    // (camera can still theoretically be within the plane but don't care about that)
                    let t = -(normal.dot(ray.origin)) / denominator;
                    if t >= 0.0 {
                        // there was an intersection
                        let target = (t * ray.direction) + ray.origin;
                        info!("Move target: {}", target);
                        let tilemap = map_assets.get(map_handle).expect("Map needs to be loaded");
                        if let Ok(index) = ChunkIndex::new(target.x as i32, target.z as i32) {
                            command_buffer
                                .add_component(*entity, FlowField::new(index, tilemap.tile_grid()));
                        }
                    }
                }
            }
        });
    }
}

#[derive(Debug)]
pub struct DebugFlow {
    pub current_target: Option<ChunkIndex>,
    pub arrow_handle: Handle<GltfModel>,
    pub spawned_arrows: Option<Vec<Entity>>,
}

fn debug_draw_flow_field(
    command_buffer: &mut CommandBuffer,
    flow_field: &FlowField,
    tilemap: &MapChunk<Tile>,
    redraw_flow: &mut DebugFlow,
) {
    if redraw_flow.current_target != Some(flow_field.target) {
        redraw_flow.current_target = Some(flow_field.target);
        if let Some(arrows) = redraw_flow.spawned_arrows.as_ref() {
            for entity in arrows.iter() {
                command_buffer.remove(*entity);
            }
        }
        let transform = *tilemap.transform();
        let debug_arrows = (0..CHUNK_SIZE)
            .cartesian_product(0..CHUNK_SIZE)
            .into_iter()
            .map(|(y, x)| {
                let tile_pos = ChunkIndex::new(x, y).unwrap();
                let height = tilemap.tile(tile_pos).middle_height();
                // 1. calc offset for arrow
                let translation = Vec3::new(
                    x as f32 * TILE_WIDTH + 0.5,
                    height + 0.7,
                    y as f32 * TILE_HEIGHT + 0.5,
                );
                let scale = Vec3::splat(0.1);
                let direction = flow_field
                    .direction_at_pos(translation.x, translation.z)
                    .map(|direction| Vec3A::new(direction.x, 0.0, direction.y))
                    .unwrap_or(Vec3A::Y);
                // 2. create Transform for chunky providing pos, rotation, scale
                let arrow_transform = Affine3A::from_scale_rotation_translation(
                    scale,
                    navigation::look_at(direction),
                    translation,
                );
                // 3. multiply transforms
                (
                    redraw_flow.arrow_handle,
                    Transform {
                        matrix: transform.matrix * arrow_transform,
                    },
                )
            })
            .collect::<Vec<_>>();
        let spawned_arrows = command_buffer.extend(debug_arrows);
        redraw_flow.spawned_arrows = Some(spawned_arrows.to_vec());
    }
}

#[system]
pub fn movement(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] map_assets: &Assets<DrawableTileMap>,
    #[resource] map_handle: &Handle<DrawableTileMap>,
    #[resource] redraw_flow: &mut DebugFlow,
    #[resource] time: &Time,
    query: &mut Query<(
        Entity,
        &FlowField,
        &Selectable,
        &mut Transform,
        &mut Velocity,
    )>,
) {
    query.for_each_mut(
        world,
        |(_entity, flow_field, selectable, transform, velocity)| {
            let tilemap = map_assets.get(map_handle).expect("Map needs to be loaded");
            // if selectable.is_selected {
            //    debug_draw_flow_field(command_buffer, flow_field, tilemap.tile_grid(), redraw_flow);
            // }
            // Movement along the flow field
            navigation::movement_impl(tilemap.tile_grid(), flow_field, transform, velocity, time);
        },
    );
}
