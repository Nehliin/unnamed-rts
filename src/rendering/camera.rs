use crate::input::{CursorPosition, EventReader, KeyboardState, MouseButtonState, MouseMotion};
use crate::resources::{Time, WindowSize};
use crevice::std430::AsStd430;
use crevice::std430::Std430;
use glam::*;
use legion::*;
use once_cell::sync::OnceCell;
use winit::event::{MouseButton, VirtualKeyCode};

#[derive(Debug)]
pub struct Camera {
    direction: Vec3A,
    position: Vec3A,
    view_matrix: Mat4,
    proj_matrix: Mat4,
    pitch: f32,
    yaw: f32,
    gpu_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

#[derive(Debug, Copy, Clone, AsStd430)]
struct CameraUniform {
    pub view_matrix: mint::ColumnMatrix4<f32>,
    pub projection: mint::ColumnMatrix4<f32>,
    pub view_pos: mint::Vector3<f32>,
    pub view_inverse: mint::ColumnMatrix4<f32>,
    pub projection_inverse: mint::ColumnMatrix4<f32>,
}

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        CameraUniform {
            view_matrix: camera.view_matrix.into(),
            projection: camera.proj_matrix.into(),
            view_pos: camera.get_position().into(),
            view_inverse: camera.view_matrix.inverse().into(),
            projection_inverse: camera.proj_matrix.inverse().into(),
        }
    }
}

pub const CAMERA_SPEED: f32 = 10.5;

pub struct Ray {
    pub origin: Vec3A,
    pub direction: Vec3A,
}

#[system]
pub fn free_flying_camera(
    #[resource] camera: &mut Camera,
    #[resource] time: &Time,
    #[resource] keyboard_state: &KeyboardState,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_motion: &EventReader<MouseMotion>,
    #[resource] queue: &wgpu::Queue,
) {
    if keyboard_state.is_pressed(VirtualKeyCode::A) {
        camera.position +=
            camera.direction.cross(Vec3A::Y).normalize() * -CAMERA_SPEED * time.delta_time;
    }
    if keyboard_state.is_pressed(VirtualKeyCode::D) {
        camera.position +=
            camera.direction.cross(Vec3A::Y).normalize() * CAMERA_SPEED * time.delta_time;
    }
    if keyboard_state.is_pressed(VirtualKeyCode::W) {
        camera.position += camera.direction * CAMERA_SPEED * time.delta_time;
    }
    if keyboard_state.is_pressed(VirtualKeyCode::S) {
        camera.position += camera.direction * -CAMERA_SPEED * time.delta_time;
    }
    if mouse_button_state.is_pressed(&MouseButton::Right) {
        for delta in mouse_motion.events() {
            let mut xoffset = delta.delta_x as f32;
            let mut yoffset = delta.delta_y as f32;
            let sensitivity: f32 = 0.1; // change this value to your liking
            xoffset *= sensitivity;
            yoffset *= sensitivity;
            camera.yaw += xoffset;
            camera.pitch += yoffset;
            if camera.pitch < -89.0 {
                camera.pitch = -89.0;
            } else if 89.0 < camera.pitch {
                camera.pitch = 89.0;
            }
        }
    }

    camera.update_view_matrix();
    // update uniform buffer
    let uniform_data: CameraUniform = (&*camera).into();
    queue.write_buffer(&camera.gpu_buffer, 0, uniform_data.as_std430().as_bytes());
}

#[allow(dead_code)]
impl Camera {
    pub fn new(
        device: &wgpu::Device,
        position: Vec3,
        direction: Vec3,
        window_width: u32,
        window_height: u32,
    ) -> Self {
        // what POINT should the camera look at?
        let view_target = position + direction;
        let gpu_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera buffer"),
            size: CameraUniform::std430_size_static() as u64,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera bindgroup"),
            layout: Self::get_or_create_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &gpu_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        Camera {
            direction: direction.into(),
            position: position.into(),
            view_matrix: Mat4::look_at_rh(position, view_target, Vec3::Y),
            proj_matrix: Mat4::perspective_rh_gl(
                45.0,
                window_width as f32 / window_height as f32,
                0.1,
                100.0,
            ),
            yaw: -90.0,
            pitch: 0.0,
            gpu_buffer,
            bind_group,
        }
    }

    pub fn raycast(&self, mouse_pos: &CursorPosition, window_size: &WindowSize) -> Ray {
        let view_inverse = self.view_matrix.inverse();
        let proj_inverse = self.proj_matrix.inverse();

        let width = window_size.physical_width as f32 * window_size.scale_factor;
        let height = window_size.physical_height as f32 * window_size.scale_factor;
        let normalised = Vec3A::new(
            (2.0 * mouse_pos.x as f32) / width - 1.0,
            1.0 - (2.0 * mouse_pos.y as f32) / height as f32,
            1.0,
        );
        let clip_space = Vec4::new(normalised.x, normalised.y, -1.0, 1.0);
        let view_space = proj_inverse * clip_space;
        let view_space = Vec4::new(view_space.x, view_space.y, -1.0, 0.0);
        // ray direction in world space coordinates
        let direction = (view_inverse * view_space)
            .xyz()
            .try_normalize()
            .expect("Normalization to work")
            .into();

        Ray {
            origin: self.position,
            direction,
        }
    }

    pub fn get_or_create_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(|| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
        })
    }

    pub fn update_aspect_ratio(&mut self, width: u32, height: u32) {
        self.proj_matrix = Mat4::perspective_rh_gl(45.0, width as f32 / height as f32, 0.1, 100.0);
    }

    #[inline]
    pub fn get_position(&self) -> Vec3A {
        self.position
    }

    #[inline]
    pub fn move_in_direction(&mut self, amount: f32) {
        self.position += self.direction * amount;
        self.view_matrix = Mat4::look_at_rh(
            self.position.into(),
            (self.position + self.direction).into(),
            Vec3::Y,
        );
    }

    #[inline]
    pub fn move_sideways(&mut self, amount: f32) {
        self.position += self.direction.cross(Vec3A::Y).normalize() * amount;
        self.view_matrix = Mat4::look_at_rh(
            self.position.into(),
            (self.position + self.direction).into(),
            Vec3::Y,
        );
    }

    #[inline]
    pub fn get_view_matrix(&self) -> &Mat4 {
        &self.view_matrix
    }

    #[inline]
    pub fn get_projection_matrix(&self) -> &Mat4 {
        &self.proj_matrix
    }

    #[inline]
    pub fn set_pitch(&mut self, pitch: f32) {
        if pitch < -89.0 {
            self.pitch = -89.0;
        } else if 89.0 < pitch {
            self.pitch = 89.0;
        } else {
            self.pitch = pitch;
        }
        self.update_view_matrix();
    }

    #[inline]
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
        self.update_view_matrix();
    }

    #[inline]
    pub fn get_pitch(&self) -> f32 {
        self.pitch
    }

    #[inline]
    pub fn get_yaw(&self) -> f32 {
        self.yaw
    }

    #[inline]
    fn update_view_matrix(&mut self) {
        self.direction.x = self.yaw.to_radians().cos() * self.pitch.to_radians().cos();
        self.direction.y = self.pitch.to_radians().sin();
        self.direction.z = self.yaw.to_radians().sin() * self.pitch.to_radians().cos();
        self.direction = self.direction.normalize();

        self.view_matrix = Mat4::look_at_rh(
            self.position.into(),
            (self.position + self.direction).into(),
            Vec3::Y,
        );
    }

    /// Get a reference to the camera's bind group.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
