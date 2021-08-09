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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Transform {
    pub matrix: Affine3A,
}
// TODO: implement buildar macro or by hand (new builder struct)
impl Transform {
    pub fn from_position(position: Vec3) -> Self {
        Transform {
            matrix: Affine3A::from_translation(position),
        }
    }

    pub fn new(translation: Vec3, scale: Vec3, rotation: Quat) -> Self {
        Transform {
            matrix: Affine3A::from_scale_rotation_translation(scale, rotation, translation),
        }
    }
}
