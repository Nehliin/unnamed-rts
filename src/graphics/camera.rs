use crevice::std140::AsStd140;
use crevice::std140::Std140;
use legion::*;
use nalgebra::geometry::Perspective3;
use nalgebra::{Matrix4, Point3, Vector3};
use once_cell::sync::Lazy;
use winit::event::{MouseButton, VirtualKeyCode};

use crate::{
    application::Time,
    input::{EventReader, KeyboardState, MouseButtonState, MouseMotion},
};
#[derive(Debug)]
pub struct Camera {
    direction: Vector3<f32>,
    position: Point3<f32>,
    view_matrix: Matrix4<f32>,
    projection_matrix: Perspective3<f32>,
    pitch: f32,
    yaw: f32,
    gpu_buffer: wgpu::Buffer,
}

#[derive(Debug, Copy, Clone, AsStd140)]
struct CameraUniform {
    pub view_matrix: mint::ColumnMatrix4<f32>,
    pub projection: mint::ColumnMatrix4<f32>,
    pub view_pos: mint::Vector3<f32>,
}

#[rustfmt::skip]
static OPENGL_TO_WGPU_COORDS: Lazy<Matrix4<f32>> = Lazy::new(|| {
    Matrix4::new(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.5,
        0.0, 0.0, 0.0, 1.0)
});

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        CameraUniform {
            view_matrix: camera.view_matrix.into(),
            projection: (*OPENGL_TO_WGPU_COORDS * (camera.get_projection_matrix())).into(),
            view_pos: camera.get_vec_position().into(),
        }
    }
}

#[inline]
fn to_vec(point: &Point3<f32>) -> Vector3<f32> {
    Vector3::new(point.x, point.y, point.z)
}

pub const CAMERA_SPEED: f32 = 4.5;

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
        camera.position += camera
            .direction
            .cross(&Vector3::new(0.0, 1.0, 0.0))
            .normalize()
            * -CAMERA_SPEED
            * time.delta_time;
    }
    if keyboard_state.is_pressed(VirtualKeyCode::D) {
        camera.position += camera
            .direction
            .cross(&Vector3::new(0.0, 1.0, 0.0))
            .normalize()
            * CAMERA_SPEED
            * time.delta_time;
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
    queue.write_buffer(&camera.gpu_buffer, 0, uniform_data.as_std140().as_bytes());
}

#[allow(dead_code)]
impl Camera {
    pub fn new(
        device: &wgpu::Device,
        position: Point3<f32>,
        direction: Vector3<f32>,
        window_width: u32,
        window_height: u32,
    ) -> Self {
        // what POINT should the camera look at?
        let view_target = position + direction;
        let gpu_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera buffer"),
            size: std::mem::size_of::<<CameraUniform as AsStd140>::Std140Type>() as u64,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        Camera {
            direction,
            position,
            view_matrix: Matrix4::look_at_rh(&position, &view_target, &Vector3::new(0.0, 1.0, 0.0)),
            projection_matrix: Perspective3::new(
                window_width as f32 / window_height as f32,
                45.0,
                0.1,
                100.0,
            ),
            yaw: -90.0,
            pitch: 0.0,
            gpu_buffer,
        }
    }

    pub fn get_binding_type() -> wgpu::BindingType {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        }
    }

    pub fn get_binding_resource(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer {
            buffer: &self.gpu_buffer,
            offset: 0,
            size: None,
        }
    }

    pub fn update_aspect_ratio(&mut self, width: u32, height: u32) {
        self.projection_matrix = Perspective3::new(width as f32 / height as f32, 45.0, 0.1, 100.0);
    }

    #[inline]
    pub fn get_vec_position(&self) -> Vector3<f32> {
        to_vec(&self.position)
    }

    #[inline]
    pub fn move_in_direction(&mut self, amount: f32) {
        self.position += self.direction * amount;
        self.view_matrix = Matrix4::look_at_rh(
            &self.position,
            &(self.position + self.direction),
            &Vector3::new(0.0, 1.0, 0.0),
        );
    }

    #[inline]
    pub fn move_sideways(&mut self, amount: f32) {
        self.position += self
            .direction
            .cross(&Vector3::new(0.0, 1.0, 0.0))
            .normalize()
            * amount;
        self.view_matrix = Matrix4::look_at_rh(
            &self.position,
            &(self.position + self.direction),
            &Vector3::new(0.0, 1.0, 0.0),
        );
    }

    #[inline]
    pub fn get_projection_matrix(&self) -> &Matrix4<f32> {
        &self.projection_matrix.as_matrix()
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
        self.direction.normalize_mut();

        self.view_matrix = Matrix4::look_at_rh(
            &self.position,
            &(self.position + self.direction),
            &Vector3::new(0.0, 1.0, 0.0),
        );
    }
}
