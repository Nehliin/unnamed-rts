use std::net::SocketAddr;

use crate::{
    assets::{Assets, Handle},
    graphics::{
        camera::Camera,
        gltf::GltfModel,
        heightmap_pass::HeightMap,
        ui::ui_context::{UiContext, WindowSize},
    },
    input::{CursorPosition, MouseButtonState},
};
use glam::*;
use legion::{world::SubWorld, *};
use rayon::prelude::*;
use unnamed_rts::components::*;
use unnamed_rts::resources::*;
use unnamed_rts::{components::Selectable, resources::Time};
use winit::event::MouseButton;
pub struct DebugMenueSettings {
    pub show_grid: bool,
    pub show_bounding_boxes: bool,
}

#[system]
pub fn draw_debug_ui(
    world: &SubWorld,
    #[resource] ui_context: &UiContext,
    #[resource] debug_settings: &mut DebugMenueSettings,
    #[resource] time: &Time,
    query: &mut Query<&Selectable>,
) {
    egui::SidePanel::left("Debug menue", 80.0).show(&ui_context.context, |ui| {
        let label = egui::Label::new(format!("FPS: {:.0}", 1.0 / time.delta_time))
            .text_color(egui::Color32::WHITE);
        ui.add(label);
        ui.checkbox(
            &mut debug_settings.show_bounding_boxes,
            "Show bounding boxes",
        );
        ui.checkbox(&mut debug_settings.show_grid, "Show debug grid");
        for selectable in query.iter(world) {
            ui.label(format!("Selected: {}", selectable.is_selected));
        }
    });
}

#[system]
#[allow(clippy::too_many_arguments)]
pub fn move_action(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] network: &NetworkSocket,
    #[resource] net_serilization: &NetworkSerialization,
    #[resource] window_size: &WindowSize,
    query: &mut Query<(Entity, &Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Right) {
        query.par_for_each(world, |(entity, selectable)| {
            if selectable.is_selected {
                let ray = camera.raycast(mouse_pos, window_size);
                // check intersection with the regular ground plan
                let normal = Vec3A::new(0.0, 1.0, 0.0);
                let denominator = normal.dot(ray.direction);
                if denominator.abs() > 0.0001 {
                    // it isn't parallel to the plane
                    // (camera can still theoretically be within the plane but don't care about that)
                    let t = -(normal.dot(ray.origin)) / denominator;
                    if t >= 0.0 {
                        // there was an intersection
                        let target = (t * ray.direction) + ray.origin;
                        let payload =
                            net_serilization.serialize_client_update(&ClientUpdate::Move {
                                entity: *entity,
                                target,
                            });

                        let packet = laminar::Packet::reliable_unordered(
                            SocketAddr::new(SERVER_ADDR.into(), SERVER_PORT),
                            payload,
                        );
                        network.sender.send(packet).unwrap();
                    }
                }
            }
        });
    }
}

#[system]
pub fn height_map_modification(
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] time: &Time,
    #[resource] height_map: &mut HeightMap,
) {
    if mouse_button_state.is_pressed(&MouseButton::Left) {
        let ray = camera.raycast(mouse_pos, window_size);
        // check intersection with the heightmap
        let normal = Vec3A::new(0.0, 1.0, 0.0);
        let denominator = normal.dot(ray.direction);
        if denominator.abs() > 0.0001 {
            // it isn't parallel to the plane
            // (camera can still theoretically be within the height_map but don't care about that)
            let height_map_pos: Vec3A = height_map.get_transform().translation.into();
            let t = (height_map_pos - ray.origin).dot(normal) / denominator;
            if t >= 0.0 {
                // there was an intersection
                let target = (t * ray.direction) + ray.origin;
                let local_coords = height_map.get_transform().get_model_matrix().inverse()
                    * Vec4::new(target.x, target.y, target.z, 1.0);
                let radius = 20.0;
                let strenght = 350.0_f32;
                let center = local_coords.xy();
                // assuming row order
                // TODO: Not very performance frendly
                height_map
                    .get_buffer_mut()
                    .par_chunks_exact_mut(256)
                    .enumerate()
                    .for_each(|(y, chunk)| {
                        chunk.iter_mut().enumerate().for_each(|(x, byte)| {
                            let distance = Vec2::new(x as f32, y as f32).distance(center);
                            if distance < radius {
                                let raise =
                                    (strenght * (radius - distance) / radius) * time.delta_time;
                                *byte = std::cmp::min(
                                    255,
                                    (*byte as f32 + raise as f32).round() as u32,
                                ) as u8;
                            }
                        })
                    });
            }
        }
    }
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
            let model = asset_storage.get(&handle).unwrap();
            let (min, max) = (model.min_vertex, model.max_vertex);
            let world_min = transform.get_model_matrix() * Vec4::new(min.x, min.y, min.z, 1.0);
            let world_max = transform.get_model_matrix() * Vec4::new(max.x, max.y, max.z, 1.0);
            selectable.is_selected = intesercts(
                camera.get_position(),
                dirfrac,
                world_min.xyz().into(),
                world_max.xyz().into(),
            );
        })
    }
}

fn intesercts(origin: Vec3A, dirfrac: Vec3A, aabb_min: Vec3A, aabb_max: Vec3A) -> bool {
    let t1 = (aabb_min - origin) * dirfrac;
    let t2 = (aabb_max - origin) * dirfrac;

    let tmin = t1.min(t2);
    let tmin = tmin.max_element();

    let tmax = t1.max(t2);
    let tmax = tmax.min_element();

    !(tmax < 0.0 || tmax < tmin)
}
