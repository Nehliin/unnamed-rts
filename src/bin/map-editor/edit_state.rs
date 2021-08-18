use glam::Vec3;
use legion::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
use unnamed_rts::{
    assets::{self, Assets, Handle},
    components::Transform,
    input::KeyboardState,
    rendering::{
        camera::{self, Camera},
        common::DepthTexture,
        drawable_tilemap::*,
        gltf::GltfModel,
        lights::{self, LightUniformBuffer},
        pass::*,
        ui::ui_resources::UiTexture,
    },
    resources::{DebugRenderSettings, WindowSize},
    states::{State, StateTransition},
    tilemap::TileMap,
};
use wgpu::{Device, Queue};
use winit::event::VirtualKeyCode;

use crate::{
    editor_systems::{self, EditorSettings, LastTileMapUpdate, UiState},
    playground_state::PlaygroundState,
};

#[derive(Debug, Default, Clone)]
struct ShouldEnterPlayground(Arc<AtomicBool>);

#[system]
fn enter_playground(
    #[resource] should_enter_playground: &ShouldEnterPlayground,
    #[resource] keyboard_input: &KeyboardState,
) {
    if keyboard_input.pressed_current_frame(VirtualKeyCode::P) {
        info!("Entering playground state!");
        should_enter_playground.0.store(true, Ordering::Release);
    }
}

#[derive(Debug, Default)]
pub struct EditState {
    test_img: Option<Handle<UiTexture<'static>>>,
    should_enter_playground: ShouldEnterPlayground,
}

impl State for EditState {
    fn on_init(
        &mut self,
        _world: &mut legion::World,
        resources: &mut legion::Resources,
        command_receivers: &mut Vec<crossbeam_channel::Receiver<wgpu::CommandBuffer>>,
    ) {
        resources.insert(self.should_enter_playground.clone());

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
        let mut tex_assets = Assets::<UiTexture>::default();
        let handle = tex_assets.load("moon.png").unwrap();
        self.test_img = Some(handle);
        resources.insert(tex_assets);

        let device = resources.get::<Device>().expect("Device to be present");
        let start = Instant::now();
        let grid_pass = grid_pass::GridPass::new(&device, debug_sender);
        let selection_pass = selection_pass::SelectionPass::new(&device, selectable_sender);
        let model_pass = model_pass::ModelPass::new(&device, model_sender);
        let heightmap_pass = tilemap_pass::TileMapPass::new(&device, tilemap_sender);
        let debug_lines_pass = debug_lines_pass::DebugLinesPass::new(&device, lines_sender);
        info!(
            "Edit state Pipeline setup time: {}ms",
            start.elapsed().as_millis()
        );

        let size = resources
            .get::<WindowSize>()
            .expect("Window size to be present");
        let queue = resources.get::<Queue>().expect("Queue to be present");
        let camera = Camera::new(
            &device,
            Vec3::new(1.0, 0.5, 3.5),
            -Vec3::Z,
            size.physical_width,
            size.physical_height,
        );
        let transform = Transform::from_position(Vec3::ZERO);
        let tilemap = TileMap::new("Tilemap".into(), 100, transform);
        let tilemap = DrawableTileMap::new(&device, &queue, tilemap);

        // render resources
        let depth_texture = DepthTexture::new(&device, size.physical_width, size.physical_height);
        let light_uniform = LightUniformBuffer::new(&device);
        drop(device);
        drop(size);
        drop(queue);
        resources.insert(Assets::<GltfModel>::default());
        resources.insert(grid_pass);
        resources.insert(model_pass);
        resources.insert(selection_pass);
        resources.insert(heightmap_pass);
        resources.insert(debug_lines_pass);
        resources.insert(debug_lines_pass::BoundingBoxMap::default());
        resources.insert(DebugRenderSettings {
            show_grid: false,
            show_bounding_boxes: true,
        });
        let editor_settings = EditorSettings::default();
        resources.insert(editor_settings);
        resources.insert(depth_texture);
        resources.insert(tilemap);
        resources.insert(light_uniform);
        resources.insert(camera);
    }

    fn on_foreground_tick(&mut self) -> unnamed_rts::states::StateTransition {
        match self.should_enter_playground.0.compare_exchange(
            true,
            false,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(true) => {
                StateTransition::Push(Box::new(PlaygroundState::default()))
            }
            _ => StateTransition::Noop,
        }
    }

    fn on_destroy(&mut self, _world: &mut legion::World, _resources: &mut legion::Resources) {
        todo!()
    }

    fn background_schedule(&self) -> legion::Schedule {
        Schedule::builder()
            .add_system(assets::asset_load_system::<UiTexture>())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system())
            .add_system(tilemap_pass::update_system())
            .add_system(tilemap_pass::draw_system())
            .add_system(grid_pass::draw_system())
            .build()
    }

    fn foreground_schedule(&self) -> legion::Schedule {
        Schedule::builder()
            .add_system(assets::asset_load_system::<UiTexture>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system())
            .add_system(editor_systems::tilemap_modification_system(
                LastTileMapUpdate {
                    last_update: std::time::Instant::now(),
                },
            ))
            .add_system(tilemap_pass::update_system())
            .add_system(tilemap_pass::draw_system())
            .add_system(grid_pass::draw_system())
            .add_system(editor_systems::editor_ui_system(UiState {
                img: self.test_img.unwrap(),
                show_load_popup: false,
                debug_tile_draw_on: false,
                load_error_label: None,
            }))
            .add_system(enter_playground_system())
            .build()
    }
}
