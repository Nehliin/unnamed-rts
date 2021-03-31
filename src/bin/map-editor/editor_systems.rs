use glam::{Vec2, Vec3A, Vec4, Vec4Swizzles};
use legion::*;
use rayon::prelude::*;
use unnamed_rts::{graphics::{camera::Camera, heightmap_pass::HeightMap}, input::{CursorPosition, MouseButtonState}, resources::{Time, WindowSize}};
use winit::event::MouseButton;

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