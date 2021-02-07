use std::time::Instant;

use crate::{
    assets::{self, Assets},
    components::Transform,
    graphics::{
        camera::{self, Camera},
        common::DepthTexture,
        grid_pass::{self, GridPass},
        model::Model,
        model_pass::{self, ModelPass},
        ui::{
            ui_context::{UiContext, WindowSize},
            ui_pass::UiPass,
            ui_systems,
        },
    },
    input::{self, KeyboardState, MouseButtonState, MouseMotion, Text},
};
use crossbeam_channel::{Receiver, Sender};
use input::CursorPosition;
use legion::*;
use legion::{Resources, Schedule, World};
use log::warn;
use nalgebra::{Isometry3, Point3, Vector3};
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

pub struct Time {
    current_time: std::time::Instant,
    pub delta_time: f32,
}

// TODO move this and the system somewhere else
pub struct DebugMenueSettings {
    pub show_grid: bool,
    pub show_bounding_cylinder: bool,
}

#[system]
pub fn draw_debug_ui(
    #[resource] ui_context: &UiContext,
    #[resource] debug_settings: &mut DebugMenueSettings,
    #[resource] time: &Time,
) {
    /*egui::Area::new("FPS area")
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(&ui_context.context, |ui| {
            let label = egui::Label::new(format!("FPS: {:.0}", 1.0 / time.delta_time))
                .text_color(egui::Color32::WHITE);
            ui.add(label);
        });*/

    egui::SidePanel::left("Debug menue", 80.0).show(&ui_context.context, |ui| {
        let label = egui::Label::new(format!("FPS: {:.0}", 1.0 / time.delta_time))
            .text_color(egui::Color32::WHITE);
        ui.add(label);
        ui.checkbox(
            &mut debug_settings.show_bounding_cylinder,
            "Show bounding cylinders",
        );
        ui.checkbox(&mut debug_settings.show_grid, "Show debug grid")
    });
}
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
    cursor_position_sender: Sender<CursorPosition>,
    modifiers_state_sender: Sender<ModifiersState>,
    // TODO: use small vec instead
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
        let instance = Instance::new(BackendBit::PRIMARY);
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
                    features: Features::empty(), // TODO: Set this properly
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
            Point3::new(0., 2., 3.5),
            Vector3::new(0.0, 0.0, -1.0),
            size.width,
            size.height,
        );
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let mut assets: Assets<Model> = Assets::new();
        let mut world = World::default();
        let mut resources = Resources::default();
        let (ui_sender, ui_rc) = crossbeam_channel::bounded(1);
        let (debug_sender, debug_rc) = crossbeam_channel::bounded(1);
        let (model_sender, model_rc) = crossbeam_channel::bounded(1);
        let schedule = Schedule::builder()
            .add_system(assets::asset_load_system::<Model>())
            .add_system(camera::free_flying_camera_system())
            .add_system(model_pass::update_system())
            .add_system(model_pass::draw_system(ModelPass::new(
                &device,
                &camera,
                model_sender,
            )))
            .add_system(ui_systems::update_ui_system())
            .add_system(grid_pass::draw_system(GridPass::new(
                &device,
                &camera,
                debug_sender,
            )))
            .add_system(ui_systems::begin_ui_frame_system(Instant::now()))
            .add_system(draw_debug_ui_system())
            .add_system(ui_systems::end_ui_frame_system(UiPass::new(
                &device, ui_sender,
            )))
            .add_system(input::event_system())
            .build();

        resources.insert(DepthTexture::new(&device, &sc_desc));
        resources.insert(device);
        resources.insert(queue);
        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0,
        });
        resources.insert(camera);
        // Event readers and input
        let (text_input_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<Text>::new(rc));
        let (cursor_position_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<CursorPosition>::new(rc));
        let (mouse_scroll_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<MouseScrollDelta>::new(rc));
        let (mouse_motion_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<MouseMotion>::new(rc));
        let (modifiers_state_sender, rc) = crossbeam_channel::unbounded();
        resources.insert(input::EventReader::<ModifiersState>::new(rc));

        resources.insert(KeyboardState::default());
        resources.insert(MouseButtonState::default());

        init_ui_resources(&mut resources, &size, window.scale_factor() as f32);
        // This should be in a game state
        let suit = assets.load("nanosuit/nanosuit.obj").unwrap();
        resources.insert(assets);
        resources.insert(DebugMenueSettings {
            show_grid: true,
            show_bounding_cylinder: false,
        });

        world.push((
            suit.clone(),
            Transform::new(
                Isometry3::translation(2.0, 0.0, 0.0),
                Vector3::new(0.2, 0.2, 0.2),
            ),
        ));
        world.push((
            suit,
            Transform::new(
                Isometry3::translation(-2.0, 0.0, 0.0),
                Vector3::new(0.2, 0.2, 0.2),
            ),
        ));
        App {
            world,
            schedule,
            resources,
            swap_chain,
            surface,
            sc_desc,
            command_receivers: vec![model_rc, debug_rc, ui_rc],
            text_input_sender,
            mouse_scroll_sender,
            mouse_motion_sender,
            cursor_position_sender,
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
                    let _ = self.cursor_position_sender.send(CursorPosition {
                        x: position.x,
                        y: position.y,
                    });
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
        // How to handle the different uniforms?
        let queue = self.resources.get_mut::<Queue>().unwrap();
        queue.submit(self.command_receivers.iter().map(|rc| rc.recv().unwrap()));

        Ok(())
    }
}
