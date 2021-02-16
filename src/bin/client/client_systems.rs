use crate::{
    assets::{Assets, Handle},
    graphics::{
        camera::Camera,
        model::Model,
        ui::ui_context::{UiContext, WindowSize},
    },
    input::{CursorPosition, MouseButtonState},
};
use legion::{world::SubWorld, *};
use nalgebra::{Point3, Vector3, Vector4};
use rayon::iter::ParallelIterator;
use std::cmp::Ordering;
use unnamed_rts::components::*;
use unnamed_rts::{components::Selectable, resources::Time};
use winit::event::MouseButton;
pub struct DebugMenueSettings {
    pub show_grid: bool,
    pub show_bounding_boxes: bool,
}

#[system]
#[read_component(Selectable)]
pub fn draw_debug_ui(
    world: &SubWorld,
    #[resource] ui_context: &UiContext,
    #[resource] debug_settings: &mut DebugMenueSettings,
    #[resource] time: &Time,
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
        let mut query = <Read<Selectable>>::query();
        for selectable in query.iter(world) {
            ui.label(format!("Selected: {}", selectable.is_selected));
        }
    });
}

#[system]
#[write_component(Selectable)]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn selection(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] asset_storage: &Assets<Model>,
    #[resource] window_size: &WindowSize,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Left) {
        let ray = camera.raycast(mouse_pos, window_size);
        let dirfrac = Vector3::new(
            1.0 / ray.direction.x,
            1.0 / ray.direction.y,
            1.0 / ray.direction.z,
        );
        let mut query = <(Read<Transform>, Read<Handle<Model>>, Write<Selectable>)>::query();
        query
            .par_iter_mut(world)
            .for_each(|(transform, handle, mut selectable)| {
                let model = asset_storage.get(&handle).unwrap();
                let (min, max) = (model.min_position, model.max_position);
                let world_min =
                    transform.get_model_matrix() * Vector4::new(min.x, min.y, min.z, 1.0);
                let world_max =
                    transform.get_model_matrix() * Vector4::new(max.x, max.y, max.z, 1.0);
                selectable.is_selected = intesercts(
                    camera.get_position(),
                    dirfrac,
                    world_min.xyz(),
                    world_max.xyz(),
                );
            })
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct NonNan(f32);

impl NonNan {
    fn new(val: f32) -> Option<NonNan> {
        if val.is_nan() {
            None
        } else {
            Some(NonNan(val))
        }
    }
}

impl Eq for NonNan {}

impl PartialOrd for NonNan {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for NonNan {
    fn cmp(&self, other: &NonNan) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn intesercts(
    origin: Point3<f32>,
    dirfrac: Vector3<f32>,
    aabb_min: Vector3<f32>,
    aabb_max: Vector3<f32>,
) -> bool {
    use std::cmp::{max, min};
    let t1 = NonNan::new((aabb_min.x - origin.x) * dirfrac.x).unwrap();
    let t2 = NonNan::new((aabb_max.x - origin.x) * dirfrac.x).unwrap();
    let t3 = NonNan::new((aabb_min.y - origin.y) * dirfrac.y).unwrap();
    let t4 = NonNan::new((aabb_max.y - origin.y) * dirfrac.y).unwrap();
    let t5 = NonNan::new((aabb_min.z - origin.z) * dirfrac.z).unwrap();
    let t6 = NonNan::new((aabb_max.z - origin.z) * dirfrac.z).unwrap();

    let tmin = max(max(min(t1, t2), min(t3, t4)), min(t5, t6));
    let tmax = min(min(max(t1, t2), max(t3, t4)), max(t5, t6));

    if tmax < NonNan::new(0.0).unwrap() || tmax < tmin {
        return false;
    }
    true
}
