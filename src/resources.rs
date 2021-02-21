use crossbeam_channel::{Receiver, Sender};
use glam::Vec3A;
use laminar::{Packet, SocketEvent};
use legion::{Entity };
use serde::{Serialize,Deserialize};

#[derive(Debug)]
pub struct Time {
    pub current_time: std::time::Instant,
    pub delta_time: f32,
}

#[derive(Debug)]
pub struct NetResource {
    pub sender: Sender<Packet>,
    pub receiver: Receiver<SocketEvent>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientActions {
    Move {
        entity: Entity,
        target: Vec3A
    }
}
