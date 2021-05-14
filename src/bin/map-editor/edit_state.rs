use std::f32::consts::PI;

use glam::{Quat, Vec3};
use legion::*;
use unnamed_rts::{
    assets::{self, Assets, Handle},
    components::Transform,
    graphics::{
        camera::{self, Camera},
        common::DepthTexture,
        debug_lines_pass::{self, BoundingBoxMap},
        gltf::GltfModel,
        grid_pass,
        heightmap_pass::{self, HeightMap},
        lights::{self, LightUniformBuffer},
        model_pass, selection_pass,
    },
    resources::{DebugRenderSettings, WindowSize},
    states::{State, StateTransition},
    ui::ui_resources::UiTexture,
};
use wgpu::{Device, Queue};

use crate::editor_systems::{self, EditorSettings, HeightMapModificationState, UiState};

#[derive(Debug, Default)]
pub struct EditState {
    test_img: Option<Handle<UiTexture<'static>>>,
}

// Very similar to game state atm
impl State for EditState {
    fn on_init(
        &mut self,
        _world: &mut legion::World,
        resources: &mut legion::Resources,
        command_receivers: &mut Vec<crossbeam_channel::Receiver<wgpu::CommandBuffer>>,
    ) {
        let (debug_sender, debug_rc) = crossbeam_channel::bounded(1);
        let (model_sender, model_rc) = crossbeam_channel::bounded(1);
        let (heightmap_sender, heightmap_rc) = crossbeam_channel::bounded(1);
        let (lines_sender, lines_rc) = crossbeam_channel::bounded(1);
        let (selectable_sender, selectable_rc) = crossbeam_channel::bounded(1);
        command_receivers.push(model_rc);
        command_receivers.push(heightmap_rc);
        command_receivers.push(selectable_rc);
        command_receivers.push(debug_rc);
        command_receivers.push(lines_rc);
        let mut tex_assets = Assets::<UiTexture>::default();
        let handle = tex_assets.load("moon.png").unwrap();
        self.test_img = Some(handle);
        resources.insert(Assets::<GltfModel>::default());
        resources.insert(tex_assets);

        let device = resources.get::<Device>().expect("Device to be present");
        let grid_pass = grid_pass::GridPass::new(&device, debug_sender);
        let model_pass = model_pass::ModelPass::new(&device, model_sender);
        let selection_pass = selection_pass::SelectionPass::new(&device, selectable_sender);
        let heightmap_pass = heightmap_pass::HeightMapPass::new(&device, heightmap_sender);
        let debug_lines_pass = debug_lines_pass::DebugLinesPass::new(&device, lines_sender);

        let size = resources
            .get::<WindowSize>()
            .expect("Window size to be present");
        let queue = resources.get::<Queue>().expect("Queue to be present");
        let camera = Camera::new(
            &device,
            Vec3::new(0., 2., 3.5),
            Vec3::new(0.0, 0.0, -1.0),
            size.physical_width,
            size.physical_height,
        );
        let mut transform = Transform::from_position(Vec3::new(0.0, 0.0, 0.0));
        transform.scale = Vec3::splat(0.1);
        transform.rotation = Quat::from_rotation_x(PI / 2.0);
        let height_map = HeightMap::new(&device, &queue, "MyMap".to_string(), 512, transform);

        // render resources
        let depth_texture = DepthTexture::new(&device, size.physical_width, size.physical_height);
        let light_uniform = LightUniformBuffer::new(&device);
        drop(device);
        drop(size);
        drop(queue);

        resources.insert(model_pass);
        resources.insert(grid_pass);
        resources.insert(selection_pass);
        resources.insert(heightmap_pass);
        resources.insert(Assets::<HeightMap>::default());
        resources.insert(debug_lines_pass);
        resources.insert(BoundingBoxMap::default());
        resources.insert(DebugRenderSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });
        let editor_settings = EditorSettings::default();
        resources.insert(editor_settings);
        resources.insert(depth_texture);
        resources.insert(height_map);
        resources.insert(light_uniform);
        resources.insert(camera);
    }

    fn on_resize(&mut self, resources: &Resources, new_size: &WindowSize) {
        let mut camera = resources.get_mut::<Camera>().unwrap();
        let device = resources.get::<Device>().unwrap();
        camera.update_aspect_ratio(new_size.physical_width, new_size.physical_height);
        resources.get_mut::<DepthTexture>().unwrap().resize(
            &device,
            new_size.physical_width,
            new_size.physical_height,
        );
    }

    fn on_foreground_tick(&mut self) -> unnamed_rts::states::StateTransition {
        StateTransition::Noop
    }

    fn on_destroy(&mut self, _world: &mut legion::World, _resources: &mut legion::Resources) {
        todo!()
    }

    fn background_schedule(&self) -> legion::Schedule {
        todo!()
    }

    fn foreground_schedule(&self) -> legion::Schedule {
        Schedule::builder()
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(assets::asset_load_system::<UiTexture>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system())
            .add_system(selection_pass::draw_system())
            .add_system(editor_systems::height_map_modification_system(
                HeightMapModificationState {
                    last_update: std::time::Instant::now(),
                },
            ))
            .add_system(heightmap_pass::update_system())
            .add_system(heightmap_pass::draw_system())
            .add_system(grid_pass::draw_system())
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system())
            .add_system(editor_systems::editor_ui_system(UiState {
                img: self.test_img.unwrap(),
                show_load_popup: false,
                load_error_label: None,
            }))
            .build()
    }
}
