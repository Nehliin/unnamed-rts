#![allow(unused)]
use glam::*;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Selectable {
    pub is_selected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Velocity {
    pub velocity: Vec3,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct MoveTarget {
    pub target: Vec3,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum EntityType {
    BasicUnit,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Transform {
    pub rotation: Quat,
    pub scale: Vec3,
    pub translation: Vec3,
}
// TODO: implement buildar macro or by hand (new builder struct)
impl Transform {
    pub fn from_position(position: Vec3) -> Self {
        let rotation = Quat::IDENTITY;
        let scale = Vec3::splat(1.0);
        Transform {
            translation: position,
            rotation,
            scale,
        }
    }

    pub fn new(translation: Vec3, scale: Vec3, rotation: Quat) -> Self {
        Transform {
            rotation,
            translation,
            scale,
        }
    }

    pub fn get_model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
