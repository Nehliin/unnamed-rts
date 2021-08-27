use std::time::Instant;

use crossbeam_channel::Receiver;
use glam::{Quat, Vec3};
use unnamed_rts::{
    assets::{self, Assets},
    common_systems,
    components::{Selectable, Transform, Velocity},
    input::KeyboardState,
    rendering::{camera, gltf::GltfModel, pass::selection_pass},
    states::{State, StateTransition},
    tilemap::{TILE_HEIGHT, TILE_WIDTH},
};

use legion::*;
use wgpu::Device;
use winit::event::VirtualKeyCode;

use crate::playground_systems::{self, DebugFlow};

#[system]
fn exit(
    #[resource] state_transition: &mut StateTransition,
    #[resource] keyboard_input: &KeyboardState,
) {
    if keyboard_input.pressed_current_frame(VirtualKeyCode::Q) {
        info!("Exiting playground state!");
        *state_transition = StateTransition::Pop;
    }
}

fn setup_render_resources(
    resources: &mut Resources,
    command_receivers: &mut Vec<Receiver<wgpu::CommandBuffer>>,
) {
    let (selectable_sender, selectable_rc) = crossbeam_channel::bounded(1);

    command_receivers.push(selectable_rc);

    let device = resources.get::<Device>().expect("Device to be present");

    let start = Instant::now();
    let selection_pass = selection_pass::SelectionPass::new(&device, selectable_sender);
    info!(
        "Playground Pipeline setup time: {}ms",
        start.elapsed().as_millis()
    );
    drop(device);
    resources.insert(selection_pass);
}

#[derive(Debug, Default)]
pub struct PlaygroundState;

impl State for PlaygroundState {
    fn on_init(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        command_receivers: &mut Vec<Receiver<wgpu::CommandBuffer>>,
    ) {
        setup_render_resources(resources, command_receivers);
        let mut model_assets = resources
            .get_mut::<Assets<GltfModel>>()
            .expect("Model assets to be loaded");
        // Set entities
        let unit = model_assets.load("toon.glb").unwrap();
        let debug_arrow = model_assets.load("arrow.glb").unwrap();
        world.extend(vec![(
            Transform::new(
                Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0),
                Vec3::ONE,
                Quat::IDENTITY,
            ),
            Velocity {
                velocity: Vec3::splat(0.0),
            },
            unit,
            Selectable { is_selected: false },
        )]);

        drop(model_assets);

        // set up resources
        resources.insert(DebugFlow {
            current_target: None,
            arrow_handle: debug_arrow,
            spawned_arrows: None,
        });
    }

    fn on_destroy(&mut self, world: &mut legion::World, _resources: &mut legion::Resources) {
        // TODO: Clean up command command_receivers
        world.clear();
    }

    fn background_schedule(&self) -> legion::Schedule {
        todo!()
    }

    fn foreground_schedule(&self) -> legion::Schedule {
        Schedule::builder()
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(camera::free_flying_camera_system())
            .add_system(selection_pass::draw_system())
            .add_system(common_systems::selection_system())
            .add_system(playground_systems::move_action_system())
            .add_system(playground_systems::movement_system())
            .add_system(common_systems::fps_ui_system())
            .add_system(exit_system())
            .build()
    }
}
