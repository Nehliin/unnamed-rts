use std::time::Instant;

use glam::{Vec2, Vec3A, Vec4, Vec4Swizzles};
use legion::*;
use rayon::prelude::*;
use unnamed_rts::{
    graphics::{camera::Camera, heightmap_pass::HeightMap},
    input::{CursorPosition, MouseButtonState},
    resources::{Time, WindowSize},
    ui::ui_context::UiContext,
};
use winit::event::MouseButton;
#[derive(Debug)]
pub struct EditorSettings {
    pub edit_heightmap: bool,
    pub hm_tool_radius: f32,
    pub hm_tool_strenght: f32,
    pub hm_tool_invert: bool,
    pub map_size: u32,
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings {
            edit_heightmap: true,
            hm_tool_radius: 10.0,
            hm_tool_strenght: 10.0,
            hm_tool_invert: false,
            map_size: 256,
        }
    }
}

#[system]
pub fn editor_ui(
    #[resource] ui_context: &UiContext,
    #[resource] editor_settings: &mut EditorSettings,
) {
    egui::SidePanel::left("editor_side_panel", 120.0).show(&ui_context.context, |ui| {
        ui.vertical_centered(|ui| {
            ui.checkbox(&mut editor_settings.edit_heightmap, "Edit heightmap");
            if editor_settings.edit_heightmap {
                ui.collapsing("Heightmap settings", |ui| {
                    ui.add(
                        egui::Slider::f32(&mut editor_settings.hm_tool_radius, 1.0..=300.0)
                            .text("Radius"),
                    );
                    ui.add(
                        egui::Slider::f32(&mut editor_settings.hm_tool_strenght, 1.0..=10.0)
                            .text("Strenght"),
                    );
                    ui.checkbox(&mut editor_settings.hm_tool_invert, "Invert");
                });
            }
        });
    });
    egui::TopPanel::top("editor_top_panel").show(&ui_context.context, |ui| {
        ui.horizontal(|ui| {
            ui.columns(3, |columns| {
                columns[1].label(format!(
                    "Map editor: <name>, size: {}",
                    editor_settings.map_size
                ));
            })
        });
    });
}

// TODO: This should be done in a more general way instead
pub struct HeightMapModificationState {
    pub last_update: Instant,
}
const MAX_UPDATE_FREQ: f32 = 1.0 / 60.0;

#[allow(clippy::too_many_arguments)]
#[system]
pub fn height_map_modification(
    #[state] modification_state: &mut HeightMapModificationState,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] time: &Time,
    #[resource] height_map: &mut HeightMap,
    #[resource] editor_settings: &EditorSettings,
) {
    if !editor_settings.edit_heightmap {
        return;
    }
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
            let radius = editor_settings.hm_tool_radius;
            let strenght = editor_settings.hm_tool_strenght;
            let center = local_coords.xy();
            if (time.current_time - modification_state.last_update).as_secs_f32() <= MAX_UPDATE_FREQ
            {
                return;
            }
            modification_state.last_update = time.current_time;
            // assuming row order
            // TODO: Not very performance frendly
            if mouse_button_state.is_pressed(&MouseButton::Left) {
                height_map
                    .get_buffer_mut()
                    .par_chunks_exact_mut(editor_settings.map_size as usize)
                    .enumerate()
                    .for_each(|(y, chunk)| {
                        chunk.iter_mut().enumerate().for_each(|(x, byte)| {
                            let distance = Vec2::new(x as f32, y as f32).distance(center);
                            if distance < radius {
                                let raise = strenght * (radius - distance) / radius;
                                if editor_settings.hm_tool_invert {
                                    *byte = std::cmp::max(
                                    0,
                                    (*byte as f32 - raise as f32).round() as u32,
                                ) as u8;
                                } else {
                                    *byte = std::cmp::min(
                                        255,
                                        (*byte as f32 + raise as f32).round() as u32,
                                    ) as u8;
                                };
                            }
                        })
                    });
            }
        }
    }
}
