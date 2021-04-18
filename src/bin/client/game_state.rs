#![allow(dead_code)]
use crate::{
    client_network::{self, add_client_components, connect_to_server},
    client_systems::{self},
};
use core::fmt::Debug;
use crossbeam_channel::Receiver;
use glam::Vec3;
use legion::*;
use unnamed_rts::resources::{NetworkSerialization, WindowSize};
use unnamed_rts::{
    assets::{self, Assets},
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
    resources::DebugRenderSettings,
    states::{State, StateTransition},
    ui::ui_resources::UiTexture,
};
use wgpu::{CommandBuffer, Device, Queue};

#[derive(Debug)]
pub struct GameState {}

impl State for GameState {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
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
        let light_uniform = LightUniformBuffer::new(&device);
        let mut height_map_assets = Assets::<HeightMap>::default();
        let height_map = height_map_assets
            .load_immediate("mymap.map", &device, &queue)
            .unwrap();
        let mut model_assets = Assets::<GltfModel>::default();
        let suit = model_assets.load("FlightHelmet/FlightHelmet.gltf").unwrap();
        let depth_texture = DepthTexture::new(&device, size.physical_width, size.physical_height);
        drop(device);
        drop(size);
        drop(queue);
        resources.insert(Assets::<UiTexture>::default());
        resources.insert(model_assets);
        resources.insert(height_map);
        resources.insert(model_pass);
        resources.insert(grid_pass);
        resources.insert(selection_pass);
        resources.insert(heightmap_pass);
        resources.insert(debug_lines_pass);
        resources.insert(BoundingBoxMap::default());
        resources.insert(NetworkSerialization::default());

        // Set up network and connect to server
        connect_to_server(world, resources);
        add_client_components(world, resources, &suit);

        resources.insert(depth_texture);
        resources.insert(DebugRenderSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });
        resources.insert(light_uniform);
        resources.insert(camera);
    }

    fn on_foreground_tick(&mut self) -> StateTransition {
        StateTransition::Noop
    }

    fn on_resize(&mut self, resources: &Resources, window_size: &WindowSize) {
        let mut camera = resources.get_mut::<Camera>().unwrap();
        let device = resources.get::<Device>().unwrap();
        camera.update_aspect_ratio(window_size.physical_width, window_size.physical_height);
        resources.get_mut::<DepthTexture>().unwrap().resize(
            &device,
            window_size.physical_width,
            window_size.physical_height,
        );
    }

    fn on_destroy(&mut self, _world: &mut World, _resources: &mut Resources) {
        todo!()
    }

    fn background_schedule(&self) -> Schedule {
        todo!()
    }

    fn foreground_schedule(&self) -> Schedule {
        Schedule::builder()
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system())
            .add_system(selection_pass::draw_system())
            .add_system(heightmap_pass::draw_system())
            .add_system(client_systems::selection_system())
            .add_system(grid_pass::draw_system())
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system())
            .add_system(client_systems::draw_debug_ui_system())
            .add_system(client_systems::move_action_system())
            .add_system(client_network::server_update_system())
            .build()
    }
}
