#![allow(dead_code)]
use crate::{
    client_network::{self, add_client_components, connect_to_server},
    client_systems,
};
use core::fmt::Debug;
use crossbeam_channel::Receiver;
use glam::Vec3;
use legion::*;
use std::{path::Path, time::Instant};
use unnamed_rts::{
    assets::{self, Assets},
    common_systems,
    rendering::{
        camera::{self, Camera},
        common::DepthTexture,
        gltf::GltfModel,
        lights::{self, LightUniformBuffer},
        pass::debug_lines_pass::{self, BoundingBoxMap},
        pass::grid_pass,
        pass::model_pass,
        pass::selection_pass,
        pass::tilemap_pass,
        ui::ui_resources::UiTexture,
    },
    resources::{DebugRenderSettings, FpsStats},
    states::State,
    tilemap::TileMap,
};
use unnamed_rts::{
    rendering::drawable_tilemap::DrawableTileMap,
    resources::{NetworkSerialization, WindowSize},
};
use wgpu::{CommandBuffer, Device, Queue};

fn setup_render_resources(
    resources: &mut Resources,
    command_receivers: &mut Vec<Receiver<wgpu::CommandBuffer>>,
    size: WindowSize,
) {
    let (debug_sender, debug_rc) = crossbeam_channel::bounded(1);
    let (model_sender, model_rc) = crossbeam_channel::bounded(1);
    let (tilemap_sender, tilemap_rc) = crossbeam_channel::bounded(1);
    let (lines_sender, lines_rc) = crossbeam_channel::bounded(1);
    let (selectable_sender, selectable_rc) = crossbeam_channel::bounded(1);
    command_receivers.push(model_rc);
    command_receivers.push(tilemap_rc);
    command_receivers.push(selectable_rc);
    command_receivers.push(debug_rc);
    command_receivers.push(lines_rc);

    let device = resources.get::<Device>().expect("Device to be present");

    let start = Instant::now();
    let grid_pass = grid_pass::GridPass::new(&device, debug_sender);
    let model_pass = model_pass::ModelPass::new(&device, model_sender);
    let selection_pass = selection_pass::SelectionPass::new(&device, selectable_sender);
    let tilemap_pass = tilemap_pass::TileMapPass::new(&device, tilemap_sender);
    let debug_lines_pass = debug_lines_pass::DebugLinesPass::new(&device, lines_sender);
    info!(
        "Game state Pipeline setup time: {}ms",
        start.elapsed().as_millis()
    );

    let light_uniform = LightUniformBuffer::new(&device);
    let depth_texture = DepthTexture::new(&device, size.physical_width, size.physical_height);

    drop(device);
    resources.insert(model_pass);
    resources.insert(depth_texture);
    resources.insert(light_uniform);
    resources.insert(grid_pass);
    resources.insert(selection_pass);
    resources.insert(tilemap_pass);
    resources.insert(debug_lines_pass);
    resources.insert(BoundingBoxMap::default());
}

#[derive(Debug)]
pub struct GameState;

impl State for GameState {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) {
        let size = *resources
            .get::<WindowSize>()
            .expect("Window size to be present");

        setup_render_resources(resources, command_receivers, size);

        let queue = resources.get::<Queue>().expect("Queue to be present");
        let device = resources.get::<Device>().expect("Device to be present");

        let camera = Camera::new(
            &device,
            Vec3::new(0., 2., 3.5),
            Vec3::new(0.0, 0.0, -1.0),
            size.physical_width,
            size.physical_height,
        );
        // TODO: This must be synced with the server
        let mut map_assets = Assets::<DrawableTileMap>::default();
        let map_handle = map_assets.load(Path::new("Tilemap.map")).unwrap();
        let mut model_assets = Assets::<GltfModel>::default();
        let suit = model_assets.load("FlightHelmet/FlightHelmet.gltf").unwrap();

        drop(device);
        drop(queue);
        resources.insert(Assets::<UiTexture>::default());
        resources.insert(model_assets);
        resources.insert(map_handle);
        resources.insert(map_assets);
        resources.insert(FpsStats::default());
        resources.insert(BoundingBoxMap::default());
        resources.insert(NetworkSerialization::default());

        // Set up network and connect to server
        connect_to_server(world, resources);
        add_client_components(world, resources, &suit);

        resources.insert(DebugRenderSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });
        resources.insert(camera);
    }

    fn on_destroy(&mut self, _world: &mut World, _resources: &mut Resources) {
        todo!()
    }

    fn background_schedule(&self) -> Schedule {
        todo!()
    }

    fn foreground_schedule(&self) -> Schedule {
        Schedule::builder()
            .add_system(common_systems::fps_system())
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(assets::asset_load_system::<DrawableTileMap>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system())
            .add_system(selection_pass::draw_system())
            .add_system(tilemap_pass::draw_system())
            .add_system(common_systems::selection_system())
            .add_system(grid_pass::draw_system())
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system())
            .add_system(client_systems::draw_debug_ui_system())
            .add_system(client_systems::move_action_system())
            .add_system(client_network::server_update_system())
            .build()
    }
}
