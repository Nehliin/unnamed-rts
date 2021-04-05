use crate::{
    input::{self, InputHandler},
    resources::{Time, WindowSize},
    states::State,
    states::{StateStack, StateTransition},
    ui::{ui_context::UiContext, ui_pass, ui_systems},
};
use crossbeam_channel::Receiver;
use legion::{systems::Step, *};
use std::time::Instant;
use wgpu::{
    BackendBit, CommandBuffer, Device, DeviceDescriptor, Features, Instance, Limits,
    PowerPreference, Queue, Surface, SwapChain, SwapChainDescriptor, SwapChainTexture,
    TextureFormat, TextureUsage,
};
use winit::{
    event::Event,
    window::{Window, WindowId},
};

pub struct Renderer {
    swap_chain: SwapChain,
    surface: Surface,
    sc_desc: SwapChainDescriptor,
    state_command_receivers: Vec<Receiver<CommandBuffer>>,
    post_state_command_receivers: Vec<Receiver<CommandBuffer>>,
}

impl Renderer {
    pub async fn init(window: &Window, resourcs: &mut Resources) -> Renderer {
        let size = window.inner_size();
         #[cfg(target_os = "macos")]
        let instance =  Instance::new(BackendBit::METAL);
        // DX12 have poor performance and crashes for whatever reason
        #[cfg(not(target_os = "macos"))]
        let instance = Instance::new(BackendBit::VULKAN);
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
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        resourcs.insert(device);
        resourcs.insert(queue);
        Renderer {
            swap_chain,
            surface,
            sc_desc,
            state_command_receivers: Vec::default(),
            post_state_command_receivers: Vec::default(),
        }
    }

    pub fn push_post_state_command_receiver(&mut self, receiver: Receiver<CommandBuffer>) {
        self.post_state_command_receivers.push(receiver);
    }

    pub fn resize(&mut self, new_size: &WindowSize, resources: &mut Resources) {
        self.sc_desc.width = new_size.physical_width;
        self.sc_desc.height = new_size.physical_height;
        // Swapchain output needs to be dropped before the swapchain
        let _ = resources.remove::<SwapChainTexture>();
        let device = resources.get::<Device>().expect("Device to be registerd");
        self.swap_chain = device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn begin_frame(&self, resources: &mut Resources) {
        resources.remove::<SwapChainTexture>();
        resources.insert(
            self.swap_chain
                .get_current_frame()
                .expect("Expected frame to be available")
                .output,
        );
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
                    self.input_handler.handle_modifiers_changed(*modifier_state)
                }

                winit::event::WindowEvent::CursorMoved { position, .. } => self
                    .input_handler
                    .handle_cursor_moved(position, &self.resources),
                winit::event::WindowEvent::ReceivedCharacter(char) => {
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

    pub fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        // move this somewhere else:
        let mut time = self.resources.get_mut::<Time>().unwrap();
        let now = Instant::now();
        time.delta_time = (now - time.current_time).as_secs_f32();
        time.current_time = now;
        drop(time);

        self.renderer.begin_frame(&mut self.resources);
        self.schedule.execute(&mut self.world, &mut self.resources);
        if let Some(foreground) = self.state_stack.peek_mut() {
            match foreground.on_foreground_tick() {
                StateTransition::Pop => {
                    let new_steps = self.state_stack.pop(&mut self.world, &mut self.resources);
                    self.schedule = Schedule::from(new_steps);
                }
                StateTransition::Push(new_state) => {
                    let new_steps = self.state_stack.push(
                        new_state,
                        &mut self.world,
                        &mut self.resources,
                        &mut self.renderer.state_command_receivers,
                    );
                    self.schedule = Schedule::from(new_steps);
                }
                StateTransition::Noop => {}
            }
        }
        self.renderer.submit_frame(&mut self.resources);
        Ok(())
    }
}
