use glam::{Affine3A, Quat, Vec2, Vec3, Vec3A, Vec3Swizzles};
use itertools::Itertools;
use legion::{systems::CommandBuffer, world::SubWorld, *};
use unnamed_rts::{
    assets::{Assets, Handle},
    components::{Selectable, Transform, Velocity},
    input::{CursorPosition, MouseButtonState},
    map_chunk::{ChunkIndex, MapChunk, CHUNK_SIZE},
    navigation::{FlowField, FlowTile},
    rendering::{camera::Camera, drawable_tilemap::*, gltf::GltfModel},
    resources::{Time, WindowSize},
    tilemap::{Tile, TILE_HEIGHT, TILE_WIDTH},
};
use winit::event::MouseButton;

// TODO Move everything below to common systems?? --------------------------------
fn intesercts(origin: Vec3A, dirfrac: Vec3A, aabb_min: Vec3A, aabb_max: Vec3A) -> bool {
    let t1 = (aabb_min - origin) * dirfrac;
    let t2 = (aabb_max - origin) * dirfrac;

    let tmin = t1.min(t2);
    let tmin = tmin.max_element();

    let tmax = t1.max(t2);
    let tmax = tmax.min_element();

    !(tmax < 0.0 || tmax < tmin)
}

#[system]
pub fn selection(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] asset_storage: &Assets<GltfModel>,
    #[resource] window_size: &WindowSize,
    query: &mut Query<(&Transform, &Handle<GltfModel>, &mut Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Left) {
        let ray = camera.raycast(mouse_pos, window_size);
        let dirfrac = ray.direction.recip();
        query.par_for_each_mut(world, |(transform, handle, mut selectable)| {
            let model = asset_storage.get(handle).unwrap();
            let (min, max) = (model.min_vertex, model.max_vertex);
            let world_min = transform.matrix.transform_point3a(min.into());
            let world_max = transform.matrix.transform_point3a(max.into());
            selectable.is_selected = intesercts(
                camera.get_position(),
                dirfrac,
                world_min.xyz(),
                world_max.xyz(),
            );
        })
    }
}

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
    #[resource] tilemap: &DrawableTileMap,
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

fn look_at(direction: Vec3A) -> Quat {
    let mut rotation_axis = Vec3A::Z.cross(direction).normalize_or_zero();
    if rotation_axis.length_squared() < 0.001 {
        rotation_axis = Vec3A::Y;
    }
    let dot = Vec3A::Z.dot(direction);
    let angle = dot.acos();
    Quat::from_axis_angle(rotation_axis.into(), angle)
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
                let direction =
                    bilinear_interpolation(translation.x, translation.z, &flow_field.chunk);
                // 2. create Transform for chunky providing pos, rotation, scale
                let arrow_transform = Affine3A::from_scale_rotation_translation(
                    scale,
                    look_at(Vec3A::new(direction.x, 0.0, direction.y)),
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

fn bilinear_interpolation(x: f32, y: f32, chunk: &MapChunk<FlowTile>) -> Vec2 {
    let (fx, fy) = (x.floor(), y.floor());
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
            .map(|idx| chunk.tile(idx).direction)
            .unwrap_or(Vec2::ZERO)
    };
    let dir = w11 * f_dir(x1, y1) + w12 * f_dir(x1, y2) + w21 * f_dir(x2, y1) + w22 * f_dir(x2, y2);
    dir.normalize_or_zero()
}

#[system]
pub fn movement(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] tilemap: &mut DrawableTileMap,
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
            if selectable.is_selected {
                debug_draw_flow_field(command_buffer, flow_field, tilemap.tile_grid(), redraw_flow);
            }
            // Movement along the flow field
            let position = transform.matrix.translation.floor();
            if let Ok(chunk_pos) = ChunkIndex::new(position.x as i32, position.z as i32) {
                if chunk_pos != flow_field.target {
                    let flow_direction = flow_field.chunk.tile(chunk_pos);
                    if flow_direction.direction != Vec2::ZERO {
                        let direction =
                            bilinear_interpolation(position.x, position.z, &flow_field.chunk);
                        // Height is determined by interpolation on the tile height
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
            let new_pos: Vec3A = Vec3A::from(translation) + (offset * time.delta_time);
            let floored_new_pos = new_pos.floor();
            if let Ok(new_chunk_pos) =
                ChunkIndex::new(floored_new_pos.x as i32, floored_new_pos.z as i32)
            {
                let translation = &mut transform.matrix.translation;
                *translation = new_pos;
                let tile = tilemap.tile_grid().tile(new_chunk_pos);
                let tile_position =
                    Vec2::new(translation.x % TILE_WIDTH, translation.z % TILE_HEIGHT);
                translation.y = tile.height_at(tile_position);
            }
        },
    );
}
