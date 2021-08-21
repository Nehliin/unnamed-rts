use std::time::Instant;

use crate::{
    input::{self, InputHandler},
    rendering::pass::ui_pass,
    rendering::ui::{ui_resources::UiContext, ui_systems},
    resources::{Time, WindowSize},
    states::State,
    states::{StateStack, StateTransition},
};
use crossbeam_channel::Receiver;
use legion::{systems::Step, *};
use wgpu::{
    Backends, CommandBuffer, Device, DeviceDescriptor, Features, Instance, Limits, PowerPreference,
    PresentMode, Queue, Surface, SurfaceConfiguration, SurfaceTexture, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::{
    dpi::PhysicalSize,
    event::{self, Event},
    window::Window,
};

pub struct Renderer {
    surface: Surface,
    surface_config: SurfaceConfiguration,
    state_command_receivers: Vec<Receiver<CommandBuffer>>,
    post_state_command_receivers: Vec<Receiver<CommandBuffer>>,
}

pub struct FrameTexture {
    pub view: TextureView,
    pub texture: SurfaceTexture,
}

impl Renderer {
    pub async fn init(window: &Window, resourcs: &mut Resources) -> Renderer {
        let size = window.inner_size();
        #[cfg(not(target_os = "macos"))]
        let instance = Instance::new(Backends::VULKAN);
        #[cfg(target_os = "macos")]
        let instance = Instance::new(Backends::METAL);
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
                    features: Features::NON_FILL_POLYGON_MODE
                        | Features::ADDRESS_MODE_CLAMP_TO_BORDER, // TODO: Set this properly
                    limits: Limits::default(),
                    label: Some("Device"),
                },
                None,
            )
            .await
            .expect("Failed to find device");

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Immediate,
        };

        surface.configure(&device, &surface_config);
        resourcs.insert(device);
        resourcs.insert(queue);
        Renderer {
            surface,
            surface_config,
            state_command_receivers: Vec::default(),
            post_state_command_receivers: Vec::default(),
        }
    }

    pub fn push_post_state_command_receiver(&mut self, receiver: Receiver<CommandBuffer>) {
        self.post_state_command_receivers.push(receiver);
    }

    pub fn resize(&mut self, new_size: &WindowSize, resources: &mut Resources) {
        self.surface_config.width = new_size.physical_width;
        self.surface_config.height = new_size.physical_height;
        let _ = resources.remove::<FrameTexture>();
        let device = resources.get::<Device>().expect("Device to be registerd");
        self.surface.configure(&device, &self.surface_config);
    }

    pub fn begin_frame(&self, resources: &mut Resources) -> Result<(), wgpu::SurfaceError> {
        resources.remove::<FrameTexture>();
        let frame = self.surface.get_current_frame()?.output;
        let frame_view = frame.texture.create_view(&TextureViewDescriptor::default());
        resources.insert(FrameTexture {
            texture: frame,
            view: frame_view,
        });
        Ok(())
    }

    pub fn submit_frame(&self, resources: &mut Resources) {
        let queue = resources.get_mut::<Queue>().unwrap();
        queue.submit(
            self.state_command_receivers
                .iter()
                .chain(&self.post_state_command_receivers)
                .filter_map(|rc| rc.try_recv().ok()),
        );
    }
}

fn construct_schedule(state_steps: &mut Vec<Step>) -> Schedule {
    let mut initial_systems = Schedule::builder()
        .add_system(ui_systems::update_ui_system())
        .add_system(ui_systems::begin_ui_frame_system(Instant::now()))
        .build()
        .into_vec();
    let mut closing_systems = Schedule::builder()
        .add_system(ui_systems::end_ui_frame_system())
        .add_system(input::event_system())
        .build()
        .into_vec();
    let mut all_steps =
        Vec::with_capacity(initial_systems.len() + closing_systems.len() + state_steps.len());
    all_steps.append(&mut initial_systems);
    all_steps.append(state_steps);
    all_steps.append(&mut closing_systems);
    Schedule::from(all_steps)
}

pub struct Engine {
    world: World,
    resources: Resources,
    state_stack: StateStack,
    schedule: Schedule,
    renderer: Renderer,
    input_handler: InputHandler,
}

impl Engine {
    pub async fn new(window: &Window) -> Engine {
        let world = World::default();
        let mut resources = Resources::default();
        let mut renderer = Renderer::init(window, &mut resources).await;

        let size = window.inner_size();
        let window_size = WindowSize {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };

        // Setup ui
        let ui_context = UiContext::new(&window_size);
        let (ui_sender, ui_rc) = crossbeam_channel::bounded(1);
        let device = resources.get::<Device>().unwrap();
        let ui_pass = ui_pass::UiPass::new(&device, ui_sender);
        renderer.push_post_state_command_receiver(ui_rc);
        drop(device);
        resources.insert(ui_pass);
        resources.insert(ui_context);
        resources.insert(window_size);
        resources.insert(StateTransition::Noop);
        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0,
        });
        // Event readers and input
        let input_handler = InputHandler::init(&mut resources);

        let state_stack = StateStack::default();

        Engine {
            world,
            resources,
            schedule: construct_schedule(&mut Vec::new()),
            state_stack,
            renderer,
            input_handler,
        }
    }

    pub fn push_state(&mut self, state: Box<dyn State>) {
        let mut state_steps = self.state_stack.push(
            state,
            &mut self.world,
            &mut self.resources,
            &mut self.renderer.state_command_receivers,
        );
        self.schedule = construct_schedule(&mut state_steps);
    }

    pub fn push_all_states(&mut self, states: Vec<Box<dyn State>>) {
        let mut state_steps = self.state_stack.push_all(
            states,
            &mut self.world,
            &mut self.resources,
            &mut self.renderer.state_command_receivers,
        );
        self.schedule = construct_schedule(&mut state_steps);
    }

    pub fn resize(&mut self, new_size: &WindowSize) {
        self.renderer.resize(new_size, &mut self.resources);
        self.state_stack.resize_states(new_size, &self.resources);
    }

    pub fn recreate_swap_chain(&mut self) {
        let window_size = self.resources.get::<WindowSize>().unwrap();
        let old_size = *window_size;
        drop(window_size);
        self.resize(&old_size);
    }

    pub fn event_handler(&mut self, event: &Event<()>) -> bool {
        match event {
            Event::WindowEvent {
                ref event,
                window_id: _,
            } => match event {
                // Window was minimized do nothing
                event::WindowEvent::Resized(PhysicalSize {
                    width: 0,
                    height: 0,
                }) => true,
                event::WindowEvent::Resized(physical_size) => {
                    let mut window_size = self.resources.get_mut::<WindowSize>().unwrap();
                    window_size.physical_height = physical_size.height;
                    window_size.physical_width = physical_size.width;
                    let new_size = *window_size;
                    drop(window_size);
                    self.resize(&new_size);
                    true
                }
                event::WindowEvent::ScaleFactorChanged {
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
                event::WindowEvent::ModifiersChanged(modifier_state) => {
                    self.input_handler.handle_modifiers_changed(*modifier_state)
                }

                event::WindowEvent::CursorMoved { position, .. } => self
                    .input_handler
                    .handle_cursor_moved(position, &self.resources),
                event::WindowEvent::ReceivedCharacter(char) => {
                    self.input_handler.handle_recived_char(*char)
                }
                //todo?
                //winit::event::WindowEvent::CursorLeft { device_id } => {}
                _ => false,
            },
            Event::DeviceEvent { event, .. } => self
                .input_handler
                .handle_device_event(event, &self.resources),
            _ => false,
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // move this somewhere else:
        let mut time = self.resources.get_mut::<Time>().unwrap();
        let now = Instant::now();
        time.delta_time = (now - time.current_time).as_secs_f32();
        time.current_time = now;
        drop(time);

        self.renderer.begin_frame(&mut self.resources)?;
        self.schedule.execute(&mut self.world, &mut self.resources);
        // Check the current state transition
        let state_transition = self
            .resources
            .remove::<StateTransition>()
            .expect("No state transition found between frames");
        match state_transition {
            StateTransition::Pop => {
                let mut new_steps = self.state_stack.pop(&mut self.world, &mut self.resources);
                self.schedule = construct_schedule(&mut new_steps);
            }
            StateTransition::Push(new_state) => {
                let mut new_steps = self.state_stack.push(
                    new_state,
                    &mut self.world,
                    &mut self.resources,
                    &mut self.renderer.state_command_receivers,
                );
                self.schedule = construct_schedule(&mut new_steps);
            }
            StateTransition::Noop => {}
        }
        // Reset the state transition between frames
        self.resources.insert(StateTransition::Noop);
        self.renderer.submit_frame(&mut self.resources);
        Ok(())
    }
}
