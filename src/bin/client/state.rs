use std::{borrow::Cow, f32::consts::PI, time::Instant};

use crossbeam_channel::Receiver;
use glam::{Quat, Vec3};
use image::GenericImageView;
use legion::*;
use unnamed_rts::{components::Transform, resources::NetworkSerialization};
use wgpu::{CommandBuffer, Device, Queue};

use crate::{assets::{self, Assets}, client_network::{add_client_components, connect_to_server}, client_systems::{self, DebugMenueSettings}, graphics::{camera::{self, Camera}, common::DepthTexture, debug_lines_pass::{self, BoundingBoxMap}, gltf::GltfModel, grid_pass, heightmap_pass::{self, HeightMap}, lights::{self, LightUniformBuffer}, model_pass, selection_pass, texture::TextureContent, ui::{ui_context::WindowSize, ui_pass::UiPass, ui_systems}}, input};

pub enum StateTransition {
    Pop,
    Push(Box<dyn State>),
}
pub trait State {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
    );
    fn on_update(&mut self) -> Option<StateTransition>;
    fn on_destroy(&mut self);
    fn on_backgrouded(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Schedule;
    fn on_forgrounded(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Schedule;
}

#[derive(Debug)]
pub struct GameState {}

impl State for GameState {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
    ) {
        resources.insert(BoundingBoxMap::default());
        resources.insert(Assets::<GltfModel>::new());
        let size = resources
            .get::<WindowSize>()
            .expect("Window size to be present");
        let device = resources.get::<Device>().expect("Device to be present");
        let mut assets = resources
            .get_mut::<Assets<GltfModel>>()
            .expect("GltfAsset storage to be present");
        let queue = resources.get::<Queue>().expect("Queue to be present");
        let camera = Camera::new(
            &device,
            Vec3::new(0., 2., 3.5),
            Vec3::new(0.0, 0.0, -1.0),
            size.physical_width,
            size.physical_height,
        );
        let light_uniform = LightUniformBuffer::new(&device);
        let img = image::io::Reader::open("assets/HeightMapExample.jpg")
            .unwrap()
            .decode()
            .unwrap();
        let texture = TextureContent {
            label: Some("Displacement map"),
            format: wgpu::TextureFormat::R8Unorm,
            bytes: Cow::Owned(img.as_luma8().expect("Grayscale displacement map").to_vec()),
            stride: 1,
            size: wgpu::Extent3d {
                width: img.width(),
                height: img.height(),
                depth: 1,
            },
        };
        let mut transform = Transform::from_position(Vec3::new(0.0, 0.0, 0.0));
        transform.scale = Vec3::splat(0.1);
        transform.rotation = Quat::from_rotation_x(PI / 2.0);

        // Set up network and connect to server
        let suit = assets.load("FlightHelmet/FlightHelmet.gltf").unwrap();
        let height_map = HeightMap::from_displacement_map(&device, &queue, 256, texture, transform);
        let depth_texture= DepthTexture::new(&device, size.physical_width, size.physical_height);
        drop(device);
        drop(assets);
        drop(size);
        drop(queue);
        connect_to_server(world, resources);
        add_client_components(world, resources, &suit);

        resources.insert(depth_texture);
        resources.insert(NetworkSerialization::default());
        resources.insert(height_map);
        resources.insert(DebugMenueSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });
        resources.insert(light_uniform);
        resources.insert(camera);
    }

    fn on_update(&mut self) -> Option<StateTransition> {
        None
    }

    fn on_destroy(&mut self) {
        todo!()
    }

    fn on_backgrouded(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Schedule {
        todo!()
    }

    fn on_forgrounded(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<CommandBuffer>>,
    ) -> Schedule {
        let (ui_sender, ui_rc) = crossbeam_channel::bounded(1);
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
        command_receivers.push(ui_rc); 
        let device = resources.get::<Device>().expect("Device to be present");
        Schedule::builder()
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system(model_pass::ModelPass::new(
                &device,
                model_sender,
            )))
            .add_system(selection_pass::draw_system(
                selection_pass::SelectionPass::new(&device, selectable_sender),
            ))
            .add_system(client_systems::height_map_modification_system())
            .add_system(heightmap_pass::update_system())
            .add_system(heightmap_pass::draw_system(
                heightmap_pass::HeightMapPass::new(&device, heightmap_sender),
            ))
            .add_system(ui_systems::update_ui_system())
            .add_system(client_systems::selection_system())
            .add_system(grid_pass::draw_system(grid_pass::GridPass::new(
                &device,
                debug_sender,
            )))
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system(
                debug_lines_pass::DebugLinesPass::new(&device, lines_sender),
            ))
            // THIS SHOULDN'T BE HERE
            .add_system(ui_systems::begin_ui_frame_system(Instant::now()))
            .add_system(client_systems::draw_debug_ui_system())
            // THIS SHOULDN'T BE HERE
            .add_system(ui_systems::end_ui_frame_system(UiPass::new(
                &device, ui_sender,
            )))
            .add_system(client_systems::move_action_system())
            // THIS SHOULDN'T BE HERE
            .add_system(input::event_system())
            .build()
    }
}
