use std::time::Instant;

use crossbeam_channel::Receiver;
use egui::{FontDefinitions, SidePanel};
use egui_demo_lib::DemoWindows;
use image::{GenericImageView, ImageFormat};
use legion::{Resources, Schedule, World};
use nalgebra::{Isometry3, Point3, Vector3};
use ui_pass::{ScreenDescriptor, UiPass};
use wgpu::{
    BackendBit, CommandBuffer, Device, DeviceDescriptor, Features, Instance, Limits,
    PowerPreference, Queue, Surface, SwapChain, SwapChainDescriptor, SwapChainTexture,
    TextureFormat, TextureUsage,
};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode},
    window::Window,
};

use crate::{
    assets::Assets,
    components::Transform,
    graphics::{
        camera::Camera,
        model::Model,
        model_pass::{self, ModelPass},
        simple_texture::SimpleTexture,
        texture::{LoadableTexture, Texture},
        ui_pass,
    },
};

//use egui_wgpu_backend::ScreenDescriptor;
use egui_winit_platform::{Platform, PlatformDescriptor};

const CAMERA_SPEED: f32 = 6.5;

pub struct Time {
    current_time: std::time::Instant,
    pub delta_time: f32,
}

pub struct App {
    world: World,
    resources: Resources,
    schedule: Schedule,
    swap_chain: SwapChain,
    surface: Surface,
    sc_desc: SwapChainDescriptor,
    // TODO: use small vec instead
    command_receivers: Vec<Receiver<CommandBuffer>>,
    pub size: PhysicalSize<u32>,
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
                    shader_validation: true,
                },
                None,
            )
            .await
            .expect("Failed to find device");

        let sc_desc = SwapChainDescriptor {
            usage: TextureUsage::OUTPUT_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        //window.set_cursor_grab(true).unwrap();
        //window.set_cursor_visible(false);

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let screen_descriptor = ScreenDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };
        let platform = Platform::new(PlatformDescriptor {
            physical_width: size.width as u32,
            physical_height: size.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });

        let mut assets: Assets<Model> = Assets::new();
        let mut world = World::default();
        let mut resources = Resources::default();
        let (ui_sender, ui_rc) = crossbeam_channel::bounded(1);
        let (model_sender, model_rc) = crossbeam_channel::bounded(1);
        let schedule = Schedule::builder()
            .add_system(model_pass::update_system())
            .add_system(model_pass::draw_system())
            .add_system(ui_pass::begin_ui_frame_system(Instant::now()))
            .add_system(ui_pass::draw_demo_system())
            .add_system(ui_pass::end_ui_frame_system(UiPass::new(
                &device, ui_sender,
            )))
            .build();
        resources.insert(ModelPass::new(&device, &sc_desc, model_sender));
        resources.insert(device);
        resources.insert(platform);
        resources.insert(screen_descriptor);
        resources.insert(queue);

        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0,
        });

        // This should be in a game state
        let suit = assets.load("nanosuit/nanosuit.obj").unwrap();
        resources.insert(assets);
        resources.insert(Camera::new(
            Point3::new(0., 0., 3.),
            Vector3::new(0.0, 0.0, -1.0),
            size.width,
            size.height,
        ));

        world.push((
            suit.clone(),
            Transform::new(
                Isometry3::translation(2.0, -1.75, 0.0),
                Vector3::new(0.2, 0.2, 0.2),
            ),
        ));
        world.push((
            suit,
            Transform::new(
                Isometry3::translation(-2.0, -1.75, 0.0),
                Vector3::new(0.2, 0.2, 0.2),
            ),
        ));
        App {
            size,
            world,
            schedule,
            resources,
            swap_chain,
            surface,
            sc_desc,
            command_receivers: vec![model_rc, ui_rc],
        }
    }
    // maybe use a system for this instead?
    pub fn resize(&mut self, new_size: PhysicalSize<u32>, updated_scale_factor: Option<f32>) {
        let device = self
            .resources
            .get::<Device>()
            .expect("Device to be registerd");
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = device.create_swap_chain(&self.surface, &self.sc_desc);
        let mut screen_descriptor = self.resources.get_mut::<ScreenDescriptor>().expect("Screen descriptor not available");
        screen_descriptor.physical_width = new_size.width;
        screen_descriptor.physical_height = new_size.height;
        if let Some(scale_factor) = updated_scale_factor {
            screen_descriptor.scale_factor = scale_factor;
        }
        let mut camera = self.resources.get_mut::<Camera>().unwrap();
        camera.update_aspect_ratio(new_size.width, new_size.height);
        self.resources.get_mut::<ModelPass>().unwrap().handle_resize(&device, &self.sc_desc);
    }

    pub fn event_handler(&mut self, event: &Event<()>) {
        match event {
            Event::DeviceEvent { ref event, .. } => {
                let mut camera = self.resources.get_mut::<Camera>().unwrap();
                let time = self.resources.get::<Time>().unwrap();
                match event {
                    DeviceEvent::Key(KeyboardInput {
                        state,
                        virtual_keycode: Some(key),
                        ..
                    }) => match *key {
                        VirtualKeyCode::A if *state == ElementState::Pressed => {
                            camera.move_sideways(-CAMERA_SPEED * time.delta_time);
                        }
                        VirtualKeyCode::D if *state == ElementState::Pressed => {
                            camera.move_sideways(CAMERA_SPEED * time.delta_time);
                        }
                        VirtualKeyCode::W if *state == ElementState::Pressed => {
                            camera.move_in_direction(CAMERA_SPEED * time.delta_time);
                        }
                        VirtualKeyCode::S if *state == ElementState::Pressed => {
                            camera.move_in_direction(-CAMERA_SPEED * time.delta_time);
                        }
                        _ => {}
                    },
                    DeviceEvent::MouseMotion { delta } => {
                        let mut xoffset = delta.0 as f32;
                        let mut yoffset = delta.1 as f32; // reversed since y-coordinates go from bottom to top
                        let sensitivity: f32 = 0.05; // change this value to your liking
                        xoffset *= sensitivity;
                        yoffset *= sensitivity;
                        let yaw = camera.get_yaw();
                        let pitch = camera.get_pitch();
                        camera.set_yaw(xoffset + yaw);
                        camera.set_pitch(yoffset + pitch);
                    }
                    _ => {}
                }
            }
            _ => {
                let mut platform = self.resources.get_mut::<Platform>().unwrap();
                platform.handle_event(event);
            }
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
        {
            let queue = self.resources.get_mut::<Queue>().unwrap();
            let mut asset_storage = self.resources.get_mut::<Assets<Model>>().unwrap();
            let device = self.resources.get::<Device>().unwrap();
            // move to a system instead
            asset_storage.clear_load_queue(&device, &queue).unwrap();
        }

        self.schedule.execute(&mut self.world, &mut self.resources);
        // How to handle the different uniforms?

        let queue = self.resources.get_mut::<Queue>().unwrap();

        queue.submit(self.command_receivers.iter().map(|rc| rc.recv().unwrap()));

        Ok(())
    }
}
