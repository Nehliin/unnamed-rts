use crossbeam_channel::Receiver;
use legion::{Resources, Schedule, World};
use nalgebra::{Isometry3, Point3, Vector3};
use wgpu::{
    BackendBit, CommandBuffer, Device, DeviceDescriptor, Features, Instance, Limits,
    PowerPreference, Queue, Surface, SwapChain, SwapChainDescriptor, SwapChainTexture,
    TextureFormat, TextureUsage,
};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    window::Window,
};

use crate::{assets::{self, Assets}, components::Transform, graphics::{camera::Camera, model::{InstanceData, Model}, model_pass::{draw_system, update_system, ModelPass}}};

const CAMERA_SPEED: f32 = 6.5;

pub struct Time {
    current_time: std::time::Instant,
    pub delta_time: f32,
}

pub struct App {
    world: World,
    resources: Resources,
    schedule: Schedule,
    sc_desc: SwapChainDescriptor,
    swap_chain: SwapChain,
    surface: Surface,
    command_receiver: Receiver<CommandBuffer>,
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
        window.set_cursor_grab(true).unwrap();
        window.set_cursor_visible(false);

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);
        let mut assets: Assets<Model> = Assets::new();
        let mut world = World::default();
        let mut resources = Resources::default();
        let (sender, rc) = crossbeam_channel::bounded(1);
        let schedule = Schedule::builder()
            .add_system(update_system())
            .add_system(draw_system(ModelPass::new(&device, &sc_desc, sender)))
            .build();
        resources.insert(device);
        resources.insert(queue);

        resources.insert(Time {
            current_time: std::time::Instant::now(),
            delta_time: 0.0
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
            suit,
            Transform::new(
                Isometry3::translation((0 + 2) as f32, -1.75, 0 as f32),
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
            command_receiver: rc,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        let device = self
            .resources
            .get::<Device>()
            .expect("Device to be registerd");
        self.sc_desc.width = new_size.width;
        self.sc_desc.height = new_size.height;
        self.swap_chain = device.create_swap_chain(&self.surface, &self.sc_desc);
    }

    pub fn input_handler(&mut self, event: &DeviceEvent) {
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
        queue.submit(std::iter::once(self.command_receiver.recv().unwrap()));

        Ok(())
    }
}
