use std::{cmp::Ordering, time::Instant};

use crate::{
    assets::{self, Assets, Handle},
    components::{Selectable, Transform},
    graphics::{
        camera::{self, Camera},
        common::DepthTexture,
        debug_lines_pass::{self, DebugLinesPass},
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
use debug_lines_pass::BoundingBoxMap;
use input::CursorPosition;
use legion::*;
use legion::{Resources, Schedule, World};
use log::warn;
use nalgebra::{Isometry3, Point3, Vector3, Vector4};
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
use world::SubWorld;

pub struct Time {
    current_time: std::time::Instant,
    pub delta_time: f32,
}

// TODO move this and the system somewhere else
pub struct DebugMenueSettings {
    pub show_grid: bool,
    pub show_bounding_boxes: bool,
}

#[system]
#[read_component(Selectable)]
pub fn draw_debug_ui(
    world: &SubWorld,
    #[resource] ui_context: &UiContext,
    #[resource] debug_settings: &mut DebugMenueSettings,
    #[resource] time: &Time,
) {
    egui::SidePanel::left("Debug menue", 80.0).show(&ui_context.context, |ui| {
        let label = egui::Label::new(format!("FPS: {:.0}", 1.0 / time.delta_time))
            .text_color(egui::Color32::WHITE);
        ui.add(label);
        ui.checkbox(
            &mut debug_settings.show_bounding_boxes,
            "Show bounding boxes",
        );
        ui.checkbox(&mut debug_settings.show_grid, "Show debug grid");
        let mut query = <Read<Selectable>>::query();
        for selectable in query.iter(world) {
            ui.label(format!("Selected: {}", selectable.is_selected));
        }
    });
}

#[system]
#[write_component(Selectable)]
#[read_component(Transform)]
#[read_component(Handle<Model>)]
pub fn selection(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] asset_storage: &Assets<Model>,
    #[resource] window_size: &WindowSize,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Left) {
        let screen_pos = mouse_pos;
        let view_inverse = camera.get_view_matrix().try_inverse().unwrap();
        let proj_inverse = camera.get_projection_matrix().try_inverse().unwrap();
        // normalised device space position TODO: take scaling into account
        let normalised = Vector3::new(
            (2.0 * screen_pos.x as f32) / window_size.physical_width as f32 - 1.0,
            1.0 - (2.0 * screen_pos.y as f32) / window_size.physical_height as f32,
            1.0,
        );
        let clip_space = Vector4::new(normalised.x, normalised.y, -1.0, 1.0);
        let view_space = proj_inverse * clip_space;
        let view_space = Vector4::new(view_space.x, view_space.y, -1.0, 0.0);

        let ray_dir_world_space = (view_inverse * view_space).xyz().normalize();
        let mut query = <(Read<Transform>, Read<Handle<Model>>, Write<Selectable>)>::query();
        for (transform, handle, mut selectable) in query.iter_mut(world) {
            let model = asset_storage.get(&handle).unwrap();
            let (min, max) = (model.min_position, model.max_position);
            let world_min = transform.get_model_matrix() * Vector4::new(min.x, min.y, min.z, 1.0);
            let world_max = transform.get_model_matrix() * Vector4::new(max.x, max.y, max.z, 1.0);
            selectable.is_selected = intesercts(
                camera.get_position(),
                ray_dir_world_space,
                world_min.xyz(),
                world_max.xyz()
            );
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
struct NonNan(f32);

impl NonNan {
    fn new(val: f32) -> Option<NonNan> {
        if val.is_nan() {
            None
        } else {
            Some(NonNan(val))
        }
    }
}

impl Eq for NonNan {}

impl Ord for NonNan {
    fn cmp(&self, other: &NonNan) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn intesercts(
    origin: Point3<f32>,
    ray: Vector3<f32>,
    aabb_min: Vector3<f32>,
    aabb_max: Vector3<f32>,
) -> bool {
    use std::cmp::{max, min};
    // can be precomputed
    let dirfrac = Vector3::new(1.0 / ray.x, 1.0 / ray.y, 1.0 / ray.z);

    let t1 = NonNan::new((aabb_min.x - origin.x) * dirfrac.x).unwrap();
    let t2 = NonNan::new((aabb_max.x - origin.x) * dirfrac.x).unwrap();
    let t3 = NonNan::new((aabb_min.y - origin.y) * dirfrac.y).unwrap();
    let t4 = NonNan::new((aabb_max.y - origin.y) * dirfrac.y).unwrap();
    let t5 = NonNan::new((aabb_min.z - origin.z) * dirfrac.z).unwrap();
    let t6 = NonNan::new((aabb_max.z - origin.z) * dirfrac.z).unwrap();

    let tmin = max(max(min(t1, t2), min(t3, t4)), min(t5, t6));
    let tmax = min(min(max(t1, t2), max(t3, t4)), max(t5, t6));

    if tmax < NonNan::new(0.0).unwrap() {
        return false;
    }

    if tmin > tmax {
        return false;
    }
    true
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
        let (lines_sender, lines_rc) = crossbeam_channel::bounded(1);
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
            .add_system(selection_system())
            .add_system(grid_pass::draw_system(GridPass::new(
                &device,
                &camera,
                debug_sender,
            )))
            .add_system(debug_lines_pass::update_bounding_boxes_system())
            .add_system(debug_lines_pass::draw_system(DebugLinesPass::new(
                &device,
                &camera,
                lines_sender,
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

        init_ui_resources(&mut resources, &size, window.scale_factor() as f32);
        // This should be in a game state
        let suit = assets.load("nanosuit/nanosuit.obj").unwrap();
        resources.insert(assets);
        resources.insert(DebugMenueSettings {
            show_grid: true,
            show_bounding_boxes: true,
        });

        world.push((
            suit.clone(),
            Selectable { is_selected: false },
            Transform::new(
                Isometry3::translation(2.0, 0.0, 0.0),
                Vector3::new(0.2, 0.2, 0.2),
            ),
        ));
        world.push((
            suit,
            Selectable { is_selected: false },
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
            command_receivers: vec![model_rc, debug_rc, lines_rc, ui_rc],
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
        // How to handle the different uniforms?
        let queue = self.resources.get_mut::<Queue>().unwrap();
        queue.submit(self.command_receivers.iter().map(|rc| rc.recv().unwrap()));

        Ok(())
    }
}
