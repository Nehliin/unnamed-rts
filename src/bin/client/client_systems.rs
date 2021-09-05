use std::net::SocketAddr;

use glam::*;
use legion::{world::SubWorld, *};
use unnamed_rts::resources::*;
use unnamed_rts::{components::Selectable, resources::FpsStats};
use unnamed_rts::{
    input::{CursorPosition, MouseButtonState},
    rendering::{camera::Camera, ui::ui_resources::UiContext},
};
use winit::event::MouseButton;

#[system]
pub fn draw_debug_ui(
    world: &SubWorld,
    #[resource] ui_context: &mut UiContext,
    #[resource] debug_settings: &mut DebugRenderSettings,
    #[resource] fps: &FpsStats,
    query: &mut Query<&Selectable>,
) {
    egui::SidePanel::left("Debug menue")
        .resizable(false)
        .max_width(80.0)
        .show(ui_context.context(), |ui| {
            let label = egui::Label::new(format!(
                "FPS: Avg: {}, Min: {}, Max: {}",
                fps.avg_fps, fps.min_fps, fps.max_fps
            ))
            .text_color(egui::Color32::WHITE);
            ui.add(label);
            let label = egui::Label::new(format!(
                "Frame time: Avg: {}, Min: {}, Max: {}",
                fps.avg_frame_time, fps.min_frame_time, fps.max_frame_time
            ))
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
                let normal = Vec3A::Y;
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
