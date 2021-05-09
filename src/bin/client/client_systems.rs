use std::net::SocketAddr;

use glam::*;
use legion::{world::SubWorld, *};
use unnamed_rts::resources::*;
use unnamed_rts::{
    assets::{Assets, Handle},
    graphics::{camera::Camera, gltf::GltfModel},
    input::{CursorPosition, MouseButtonState},
    ui::ui_resources::UiContext,
};
use unnamed_rts::{components::Selectable, resources::Time};
use unnamed_rts::{components::*, graphics::navmesh_pass::DrawableNavMesh};
use winit::event::MouseButton;

#[system]
pub fn draw_debug_ui(
    world: &SubWorld,
    #[resource] ui_context: &UiContext,
    #[resource] debug_settings: &mut DebugRenderSettings,
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
    query: &mut Query<(Entity, &Selectable, &Transform)>,
    mesh_query: &mut Query<(&Transform, &DrawableNavMesh)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Right) {
        let (_, nav_mesh) = mesh_query.iter(world).next().unwrap();
        query.par_for_each(world, |(entity, selectable, transfrom)| {
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
                        let from: [f32; 3] = transfrom.translation.into();
                        // let to = (target.x, 0.0, target.z);
                        let to: [f32; 3] = target.into();
                        println!("From {:#?}, to: {:#?}", from, to);
                        if let Some(path) = nav_mesh.mesh.find_path(
                            from.into(),
                            to.into(),
                            navmesh::NavQuery::Accuracy,
                            navmesh::NavPathMode::Accuracy,
                        ) {
                            dbg!(&path);
                            let converted_path: Vec<Vec3> = path
                                .iter()
                                .map(|nvec| Vec3::new(nvec.x, nvec.y, nvec.z))
                                .collect();
                            let payload =
                                net_serilization.serialize_client_update(&ClientUpdate::Move {
                                    entity: *entity,
                                    path: converted_path,
                                });

                            let packet = laminar::Packet::reliable_unordered(
                                SocketAddr::new(SERVER_ADDR.into(), SERVER_PORT),
                                payload,
                            );
                            network.sender.send(packet).unwrap();
                        }
                    }
                }
            }
        });
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
