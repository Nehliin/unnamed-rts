use crate::{
    assets::{Assets, Handle},
    graphics::{
        camera::Camera,
        model::Model,
        ui::ui_context::{UiContext, WindowSize},
    },
    input::{CursorPosition, MouseButtonState},
};
use glam::*;
use legion::{world::SubWorld, *};
use rayon::iter::ParallelIterator;
use serialize::Canon;
use unnamed_rts::components::*;
use unnamed_rts::resources::*;
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
#[read_component(Selectable)]
pub fn move_action(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] network: &NetResource,
    #[resource] canon: &Canon,
    #[resource] window_size: &WindowSize,
) {
    let mut query = <(Entity, Read<Selectable>)>::query();
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
                        let move_target = (t * ray.direction) + ray.origin;
                        use legion::serialize::{set_entity_serializer};
                        let test = set_entity_serializer(canon, || {
                            bincode::serialize(&ClientActions::Move {
                                entity: *entity,
                                target: move_target,
                            }).unwrap()
                        });
                       
                        let packet = laminar::Packet::reliable_unordered(
                            ([127, 0, 0, 1], 1338).into(),
                            test,
                        );
                        network.sender.send(packet).unwrap();
                    }
                }
            }
        });
    }
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
        let dirfrac = ray.direction.recip();
        let mut query = <(Read<Transform>, Read<Handle<Model>>, Write<Selectable>)>::query();
        query
            .par_iter_mut(world)
            .for_each(|(transform, handle, mut selectable)| {
                let model = asset_storage.get(&handle).unwrap();
                let (min, max) = (model.min_position, model.max_position);
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
