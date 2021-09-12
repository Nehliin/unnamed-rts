use std::time::Duration;

use egui::Color32;
use glam::{Vec3A, Vec3Swizzles};
use legion::{world::SubWorld, *};
use winit::event::MouseButton;

use crate::{
    assets::{Assets, Handle},
    components::{Selectable, Transform},
    input::{CursorPosition, MouseButtonState},
    rendering::{
        camera::Camera, drawable_tilemap::*, gltf::GltfModel, ui::ui_resources::UiContext,
    },
    resources::{FpsStats, Time, WindowSize},
};

fn intesercts(origin: Vec3A, dirfrac: Vec3A, aabb_min: Vec3A, aabb_max: Vec3A) -> bool {
    let t1 = (aabb_min - origin) * dirfrac;
    let t2 = (aabb_max - origin) * dirfrac;

    let tmin = t1.min(t2);
    let tmin = tmin.max_element();

    let tmax = t1.max(t2);
    let tmax = tmax.min_element();

    !(tmax < 0.0 || tmax < tmin)
}

#[derive(Debug, Default)]
pub struct SelectionState {
    pub start_selection: Option<CursorPosition>,
}

// Draws the mouse selection area in the ui layer
fn draw_selection_area(ui_ctx: &egui::CtxRef, min: egui::Vec2, max: egui::Vec2) {
    egui::Area::new("Selectable area")
        .anchor(egui::Align2::LEFT_TOP, min)
        .show(ui_ctx, |ui| {
            let (response, painter) = ui.allocate_painter(max - min, egui::Sense::hover());
            let rect = response.rect;
            let stroke = egui::Stroke::new(2.0, Color32::GREEN);
            // dim green color
            painter.rect_filled(rect, 1.0, Color32::from_rgba_premultiplied(0, 255, 0, 25));
            painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
            painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
            painter.line_segment([rect.left_top(), rect.right_top()], stroke);
            painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
        });
}

// TODO: move this to a utility function somewhere accessable
fn intesects_map(
    screen_pos: egui::Vec2,
    camera: &Camera,
    window_size: &WindowSize,
    tilemap: &DrawableTileMap<'_>,
) -> Option<Vec3A> {
    let ray = camera.raycast(
        &CursorPosition {
            x: screen_pos.x as f64,
            y: screen_pos.y as f64,
        },
        window_size,
    );
    let normal = Vec3A::new(0.0, 1.0, 0.0);
    let denominator = normal.dot(ray.direction);
    if denominator.abs() > 0.0001 {
        // this means the ray isn't parallel to the plane
        // (camera can still theoretically be within the height_map but don't care about that)
        let height_map_pos: Vec3A = tilemap.tile_grid().transform().matrix.translation;
        let t = (height_map_pos - ray.origin).dot(normal) / denominator;
        if t >= 0.0 {
            // there was an intersection
            return Some((t * ray.direction) + ray.origin);
        }
    }
    None
}

#[system]
#[allow(clippy::too_many_arguments)]
pub fn selection(
    world: &mut SubWorld,
    #[state] state: &mut SelectionState,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] asset_storage: &Assets<GltfModel>,
    #[resource] window_size: &WindowSize,
    #[resource] ui_ctx: &mut UiContext,
    #[resource] tilemap_handle: &Handle<DrawableTileMap<'static>>,
    #[resource] map_assets: &mut Assets<DrawableTileMap<'static>>,
    query: &mut Query<(&Transform, &Handle<GltfModel>, &mut Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Left) {
        state.start_selection = Some(*mouse_pos);
    }
    let start_selection = match state.start_selection {
        Some(start_selection) => egui::vec2(start_selection.x as f32, start_selection.y as f32),
        None => return,
    };

    let current_pos = egui::vec2(mouse_pos.x as f32, mouse_pos.y as f32);
    let diff = current_pos - start_selection;
    // only handle the selection as a multi selection if diff is large enough
    // to avoid drawing tiny squares
    let multi_selection = diff.length() > 5.0;
    if mouse_button_state.is_pressed(&MouseButton::Left) && multi_selection {
        let min = start_selection.min(current_pos);
        let max = start_selection.max(current_pos);
        draw_selection_area(ui_ctx.context(), min, max);
    }
    if mouse_button_state.released_current_frame(&MouseButton::Left) {
        if multi_selection {
            let tilemap = map_assets.get(tilemap_handle).unwrap();
            let screen_min = start_selection.min(current_pos);
            let screen_max = start_selection.max(current_pos);
            let min_intersection = intesects_map(screen_min, camera, window_size, tilemap);
            let max_intersection = intesects_map(screen_max, camera, window_size, tilemap);
            if let (Some(screen_min_world), Some(screen_max_world)) =
                (min_intersection, max_intersection)
            {
                if let (Some(screen_min_tile), Some(screen_max_tile)) = (
                    tilemap.to_tile_coords(screen_min_world),
                    tilemap.to_tile_coords(screen_max_world),
                ) {
                    // Take camera rotation into account. What's min/max when basing on screen
                    // space min/max coordinates might not actually be the min/max coordinates for
                    // the map itself when not rotated.
                    let min_tile = screen_min_tile.min(screen_max_tile);
                    let max_tile = screen_max_tile.max(screen_min_tile);
                    query.par_for_each_mut(world, |(transform, _handle, mut selectable)| {
                        let tile_pos = tilemap
                            .to_tile_coords(transform.matrix.translation)
                            .unwrap();
                        selectable.is_selected =
                            tile_pos.cmpge(min_tile).all() && tile_pos.cmple(max_tile).all();
                    });
                }
            }
        } else {
            let ray = camera.raycast(
                &CursorPosition {
                    x: start_selection.x as f64,
                    y: start_selection.y as f64,
                },
                window_size,
            );
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
            });
        }
        state.start_selection = None;
    }
}

#[system]
pub fn fps_ui(#[resource] ui_context: &mut UiContext, #[resource] fps: &FpsStats) {
    egui::Area::new("Fps stats")
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::ZERO)
        .show(ui_context.context(), |ui| {
            ui.colored_label(egui::Color32::WHITE, format!("Fps avg: {}", fps.avg_fps));
        });
}

#[system]
pub fn fps(#[resource] time: &Time, #[resource] fps_stats: &mut FpsStats) {
    let current_fps = 1.0 / time.delta_time();
    fps_stats.max_fps = std::cmp::max(fps_stats.max_fps, current_fps as u32);
    fps_stats.min_fps = std::cmp::min(fps_stats.min_fps, current_fps as u32);

    let time_since_last_avg = *time.current_time() - fps_stats.start_frame_time;

    if time_since_last_avg >= Duration::from_secs(1) {
        fps_stats.avg_frame_time = time_since_last_avg.as_secs_f32()
            / (time.current_frame() - fps_stats.start_frame_number) as f32;
        fps_stats.avg_fps = (1.0 / fps_stats.avg_frame_time) as u32;
        fps_stats.start_frame_number = time.current_frame();
        fps_stats.start_frame_time = *time.current_time();
    }
}
