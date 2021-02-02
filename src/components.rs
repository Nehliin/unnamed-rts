use nalgebra::{Isometry3, Matrix4, Vector3};

pub struct model;

#[repr(C)]
pub struct Transform {
    pub isometry: Isometry3<f32>,
    pub scale: Vector3<f32>,
}
// TODO: implement buildar macro or by hand (new builder struct)
impl Transform {
    pub fn from_position(position: Vector3<f32>) -> Self {
        let isometry = Isometry3::translation(position.x, position.y, position.z);
        let scale = Vector3::new(1.0, 1.0, 1.0);
        Transform { isometry, scale }
    }

    pub fn new(isometry: Isometry3<f32>, scale: Vector3<f32>) -> Self {
        Transform { isometry, scale }
    }

    pub fn get_model_matrix(&self) -> Matrix4<f32> {
        self.isometry.to_homogeneous() * Matrix4::new_nonuniform_scaling(&self.scale)
    }

    pub fn translation(&self) -> Vector3<f32> {
        self.isometry.translation.vector
    }
}
