use crate::client_network::handle_server_update;
use crate::client_network::{add_client_components, connect_to_server};
use crate::client_systems;
use crate::client_systems::DebugMenueSettings;
use crate::{
    assets::{self, Assets},
    graphics::{
        camera::{self, Camera},
        common::DepthTexture,
        debug_lines_pass::{self, DebugLinesPass},
        gltf::GltfModel,
        grid_pass::{self, GridPass},
        heightmap_pass::{self, HeightMapPass},
        lights::{self, LightUniformBuffer},
        model_pass::{self, ModelPass},
        selection_pass::{self, SelectionPass},
        texture::TextureContent,
        ui::{
            ui_context::{UiContext, WindowSize},
            ui_pass::UiPass,
            ui_systems,
        },
    },
    input::{self, KeyboardState, MouseButtonState, MouseMotion, Text},
};
use crossbeam_channel::{Receiver, Sender};
use debug_lines_pass::BoundingBoxMap;
use glam::{Quat, Vec3};
use heightmap_pass::HeightMap;
use image::GenericImageView;
use input::CursorPosition;
use legion::*;
use log::warn;
use std::{borrow::Cow, f32::consts::PI, time::Instant};
use unnamed_rts::{
    components::Transform,
    resources::{NetworkSerialization, Time},
};
use wgpu::{
    BackendBit, CommandBuffer, Device, DeviceDescriptor, Features, Instance, Limits,
    PowerPreference, Queue, Surface, SwapChain, SwapChainDescriptor, SwapChainTexture,
    TextureFormat, TextureUsage,
};
use winit::{
    dpi::PhysicalSize,
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta,
    },
    window::{Window, WindowId},
};

pub struct App {
    world: World,
    resources: Resources,
    schedule: Schedule,
    swap_chain: SwapChain,
    surface: Surface,
    sc_desc: SwapChainDescriptor,
    text_input_sender: Sender<Text>,
    mouse_scroll_sender: Sender<MouseScrollDelta>,
    mouse_motion_sender: Sender<MouseMotion>,
    modifiers_state_sender: Sender<ModifiersState>,
    command_receivers: Vec<Receiver<CommandBuffer>>,
}

fn init_ui_resources(resources: &mut Resources, size: &PhysicalSize<u32>, scale_factor: f32) {
    let window_size = WindowSize {
        physical_width: size.width,
        physical_height: size.height,
        scale_factor,
    };

    let ui_context = UiContext::new(&window_size);
    resources.insert(ui_context);
    resources.insert(window_size);
}

impl App {
    pub async fn new(window: &Window) -> App {
        let size = window.inner_size();
        let instance = if cfg!(mac) {
            Instance::new(BackendBit::METAL)
        } else {
            // DX12 have poor performance and crashes for whatever reason
            Instance::new(BackendBit::VULKAN)
        };
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to create adaptor");
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: Features::NON_FILL_POLYGON_MODE, // TODO: Set this properly
                    limits: Limits::default(),
                    label: Some("Device"),
                },
                None,
            )
            .await
            .expect("Failed to find device");

        let sc_desc = SwapChainDescriptor {
            usage: TextureUsage::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
        };
        //window.set_cursor_grab(true).unwrap();
        //window.set_cursor_visible(false);
        let camera = Camera::new(
            &device,
            Vec3::new(0., 2., 3.5),
            Vec3::new(0.0, 0.0, -1.0),
            size.width,
            size.height,
        );
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let mut world = World::default();
        let mut resources = Resources::default();
        let (ui_sender, ui_rc) = crossbeam_channel::bounded(1);
        let (debug_sender, debug_rc) = crossbeam_channel::bounded(1);
        let (model_sender, model_rc) = crossbeam_channel::bounded(1);
        let (heightmap_sender, heightmap_rc) = crossbeam_channel::bounded(1);
        let (lines_sender, lines_rc) = crossbeam_channel::bounded(1);
        let (selectable_sender, selectable_rc) = crossbeam_channel::bounded(1);
        let light_uniform = LightUniformBuffer::new(&device);
        let schedule = Schedule::builder()
            .add_system(assets::asset_load_system::<GltfModel>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(lights::update_system())
            .add_system(model_pass::draw_system(ModelPass::new(
                &device,
                model_sender,
            )))
            .add_system(selection_pass::draw_system(SelectionPass::new(
                &device,
                selectable_sender,
            )))
            .add_system(client_systems::height_map_modification_system())
            .add_system(heightmap_pass::update_system())
            .add_system(heightmap_pass::draw_system(HeightMapPass::new(
                &device,
                heightmap_sender,
            )))
            .add_system(ui_systems::update_ui_system())
            .add_system(client_systems::selection_system())
            .add_system(grid_pass::draw_system(GridPass::new(&device, debug_sender)))
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system(DebugLinesPass::new(
                &device,
                lines_sender,
            )))
            .add_system(ui_systems::begin_ui_frame_system(Instant::now()))
            .add_system(client_systems::draw_debug_ui_system())
            .add_system(ui_systems::end_ui_frame_system(UiPass::new(
                &device, ui_sender,
            )))
            .add_system(client_systems::move_action_system())
            .add_system(input::event_system())
            .build();

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
        resources.insert(HeightMap::from_displacement_map(
            &device, &queue, 256, texture, transform,
        ));
        resources.insert(DepthTexture::new(&device, &sc_desc));
        resources.insert(device);
        resources.insert(light_uniform);
        resources.insert(queue);
        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0,
        });
        resources.insert(camera);
        resources.insert(BoundingBoxMap::default());
        // Event readers and input
        let (text_input_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<Text>::new(rc));
        resources.insert(input::CursorPosition::default());
        let (mouse_scroll_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<MouseScrollDelta>::new(rc));
        let (mouse_motion_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<MouseMotion>::new(rc));
        let (modifiers_state_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<ModifiersState>::new(rc));

        resources.insert(KeyboardState::default());
        resources.insert(MouseButtonState::default());

        resources.insert(NetworkSerialization::default());
        // prelode assets: TODO: do this in app main and fetch handle based on path instead
        let mut assets = Assets::<GltfModel>::new();
        let suit = assets.load("FlightHelmet/FlightHelmet.gltf").unwrap();
        init_ui_resources(&mut resources, &size, window.scale_factor() as f32);

        resources.insert(assets);
        resources.insert(DebugMenueSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });

        // Set up network and connect to server
        connect_to_server(&mut world, &mut resources);
        add_client_components(&mut world, &mut resources, &suit);
        App {
            world,
            schedule,
            resources,
            swap_chain,
            surface,
            sc_desc,
            command_receivers: vec![
                model_rc,
                heightmap_rc,
                selectable_rc,
                debug_rc,
                lines_rc,
                ui_rc,
            ],
            text_input_sender,
            mouse_scroll_sender,
            mouse_motion_sender,
            modifiers_state_sender,
        }
    }

    // maybe use a system for this instead?
    pub fn resize(&mut self, window_size: &WindowSize) {
        let device = self
            .resources
            .get::<Device>()
            .expect("Device to be registerd");
        self.sc_desc.width = window_size.physical_width;
        self.sc_desc.height = window_size.physical_height;
        // This will lead to crashes becase the swapchain is created before the old one is dropped
        self.swap_chain = device.create_swap_chain(&self.surface, &self.sc_desc);
        let mut camera = self.resources.get_mut::<Camera>().unwrap();
        camera.update_aspect_ratio(window_size.physical_width, window_size.physical_height);
        self.resources
            .get_mut::<DepthTexture>()
            .unwrap()
            .resize(&device, &self.sc_desc);
    }

    pub fn recreate_swap_chain(&mut self) {
        let window_size = self.resources.get::<WindowSize>().unwrap();
        let old_size = *window_size;
        drop(window_size);
        self.resize(&old_size);
    }

    pub fn event_handler(&mut self, event: &Event<()>, current_window: &WindowId) -> bool {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == current_window => match event {
                winit::event::WindowEvent::Resized(physical_size) => {
                    let mut window_size = self.resources.get_mut::<WindowSize>().unwrap();
                    window_size.physical_height = physical_size.height;
                    window_size.physical_width = physical_size.width;
                    let new_size = *window_size;
                    drop(window_size);
                    self.resize(&new_size);
                    true
                }
                winit::event::WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    let mut window_size = self.resources.get_mut::<WindowSize>().unwrap();
                    window_size.physical_height = new_inner_size.height;
                    window_size.physical_width = new_inner_size.width;
                    window_size.scale_factor = *scale_factor as f32;
                    let new_size = *window_size;
                    drop(window_size);
                    self.resize(&new_size);
                    true
                }
                winit::event::WindowEvent::ModifiersChanged(modifier_state) => {
                    let _ = self.modifiers_state_sender.send(*modifier_state);
                    true
                }
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    let mut cursor_position = self.resources.get_mut::<CursorPosition>().unwrap();
                    cursor_position.x = position.x;
                    cursor_position.y = position.y;
                    true
                }
                winit::event::WindowEvent::ReceivedCharacter(char) => {
                    let _ = self.text_input_sender.send(Text { codepoint: *char });
                    true
                }
                //todo?
                //winit::event::WindowEvent::CursorLeft { device_id } => {}
                _ => false,
            },
            Event::DeviceEvent { event, .. } => match *event {
                DeviceEvent::MouseMotion { delta } => {
                    let _ = self.mouse_motion_sender.send(MouseMotion {
                        delta_x: delta.0,
                        delta_y: delta.1,
                    });
                    true
                }
                DeviceEvent::MouseWheel { delta } => {
                    let _ = self.mouse_scroll_sender.send(delta);
                    true
                }
                DeviceEvent::Button { button, state } => {
                    let mut mouse_button_state =
                        self.resources.get_mut::<MouseButtonState>().unwrap();
                    if state == ElementState::Pressed {
                        match button {
                            1 => {
                                mouse_button_state.set_pressed(&MouseButton::Left);
                                true
                            }
                            2 => {
                                mouse_button_state.set_pressed(&MouseButton::Middle);
                                true
                            }
                            3 => {
                                mouse_button_state.set_pressed(&MouseButton::Right);
                                true
                            }
                            _ => false,
                        }
                    } else {
                        match button {
                            1 => {
                                mouse_button_state.set_released(&MouseButton::Left);
                                true
                            }
                            2 => {
                                mouse_button_state.set_released(&MouseButton::Middle);
                                true
                            }
                            3 => {
                                mouse_button_state.set_released(&MouseButton::Right);
                                true
                            }
                            _ => false,
                        }
                    }
                }
                DeviceEvent::Key(KeyboardInput {
                    state,
                    virtual_keycode,
                    ..
                }) => {
                    let mut keyboard_state = self.resources.get_mut::<KeyboardState>().unwrap();
                    if state == ElementState::Pressed {
                        if let Some(key) = virtual_keycode {
                            keyboard_state.set_pressed(key);
                        } else {
                            warn!("Couldn't read keyboard input!");
                        }
                        true
                    } else {
                        if let Some(key) = virtual_keycode {
                            keyboard_state.set_released(key);
                        } else {
                            warn!("Couldn't read keyboard input!");
                        }
                        true
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }
    // Use system instead?
    pub fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        // move this somewhere else:
        let mut time = self.resources.get_mut::<Time>().unwrap();
        let now = std::time::Instant::now();
        time.delta_time = (now - time.current_time).as_secs_f32();
        time.current_time = now;
        drop(time);

        self.resources.remove::<SwapChainTexture>();
        self.resources
            .insert(self.swap_chain.get_current_frame()?.output);
        self.schedule.execute(&mut self.world, &mut self.resources);
        handle_server_update(&mut self.world, &mut self.resources);

        // How to handle the different uniforms?
        let queue = self.resources.get_mut::<Queue>().unwrap();
        queue.submit(self.command_receivers.iter().map(|rc| rc.recv().unwrap()));

        Ok(())
    }
}
