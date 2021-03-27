use crate::{
    assets::{self, Assets},
    graphics::{
        camera::Camera,
        common::DepthTexture,
        gltf::GltfModel,
        ui::{
            ui_context::{UiContext, WindowSize},
            ui_pass::UiPass,
            ui_systems,
        },
    },
    input::{self, KeyboardState, MouseButtonState, MouseMotion, Text},
};
use crate::{client_network::handle_server_update, state::State};
use crossbeam_channel::{Receiver, Sender};
use input::CursorPosition;
use legion::*;
use log::warn;
use unnamed_rts::resources::{NetworkSerialization, Time};
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

pub struct Engine {
    world: World,
    resources: Resources,
    states: Vec<Box<dyn State>>,
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

impl Engine {
    pub async fn new(window: &Window) -> Engine {
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
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let mut world = World::default();
        let mut resources = Resources::default();

        let window_size = WindowSize {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };
        let ui_context = UiContext::new(&window_size);

        resources.insert(ui_context);
        resources.insert(window_size);
        resources.insert(device);
        resources.insert(queue);
        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0,
        });
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

        let mut state = crate::state::GameState {};
        let mut command_receivers = vec![];
        state.on_init(&mut world, &mut resources);
        let schedule = state.on_forgrounded(&mut world, &mut resources, &mut command_receivers);

        Engine {
            world,
            resources,
            swap_chain,
            surface,
            sc_desc,
            schedule,
            command_receivers,
            states: vec![Box::new(state)],
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
        // TODO MOVE TO on_resize
        let mut camera = self.resources.get_mut::<Camera>().unwrap();
        camera.update_aspect_ratio(window_size.physical_width, window_size.physical_height);
        self.resources.get_mut::<DepthTexture>().unwrap().resize(
            &device,
            self.sc_desc.width,
            self.sc_desc.height,
        );
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
        let states = &mut self.states;
        let mut world = &mut self.world;
        let mut resources = &mut self.resources;
        self.schedule.execute(world, resources);
        {
            states.iter_mut().for_each(|state| {
                state.on_update();
            });
        }
        handle_server_update(&mut self.world, &mut self.resources);

        // How to handle the different uniforms?
        let queue = self.resources.get_mut::<Queue>().unwrap();
        queue.submit(self.command_receivers.iter().map(|rc| rc.recv().unwrap()));

        Ok(())
    }
}
