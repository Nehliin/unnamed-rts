use std::time::Duration;

use glam::{Vec3A, Vec3Swizzles};
use legion::{world::SubWorld, *};
use winit::event::MouseButton;

use crate::{
    assets::{Assets, Handle},
    components::{Selectable, Transform},
    input::{CursorPosition, MouseButtonState},
    rendering::{camera::Camera, gltf::GltfModel},
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

#[system]
pub fn fps(#[resource] time: &Time, #[resource] fps_stats: &mut FpsStats) {
    let current_fps = 1.0 / time.delta_time();
    fps_stats.max_fps = std::cmp::max(fps_stats.max_fps, current_fps as u32);
    fps_stats.min_fps = std::cmp::min(fps_stats.min_fps, current_fps as u32);

    let time_since_last_avg = fps_stats.start_frame_time - *time.current_time();

    if time_since_last_avg >= Duration::from_secs(1) {
        fps_stats.avg_frame_time =
            time_since_last_avg.as_secs_f32() / (time.current_frame() - fps_stats.start_frame_number) as f32;
        fps_stats.avg_fps = (1.0 / fps_stats.avg_frame_time) as u32;
        fps_stats.start_frame_number = time.current_frame();
        fps_stats.start_frame_time = *time.current_time();
    }
}
