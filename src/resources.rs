use std::net::Ipv4Addr;

use anyhow::Result;
use bincode::de::Deserializer;
use bincode::{DefaultOptions, Options};
use crossbeam_channel::{Receiver, Sender};
use glam::Vec3A;
use laminar::{Packet, SocketEvent};
use legion::{query::LayoutFilter, serialize::Canon, *};
use serde::{de::DeserializeSeed, Deserialize, Serialize};

use crate::components::{EntityType, Transform, Velocity};

#[derive(Debug)]
pub struct Time {
    pub current_time: std::time::Instant,
    pub delta_time: f32,
}

#[derive(Debug)]
pub struct NetworkSocket {
    pub sender: Sender<Packet>,
    pub receiver: Receiver<SocketEvent>,
    pub ip: [u8; 4],
    pub port: u16,
}

//Move this
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientUpdate {
    Move { entity: Entity, target: Vec3A },
    StartGame { ip: [u8; 4], port: u16 },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerUpdate {
    State {
        transforms: Vec<(Entity, Transform)>,
    },
}

pub const SERVER_UPDATE_STREAM: u8 = 1;
pub const CLIENT_UPDATE_STREAM: u8 = 2;

pub const SERVER_ADDR: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
pub const SERVER_PORT: u16 = 1338;
pub struct NetworkSerialization {
    registry: Registry<i32>,
    canon: Canon,
}

impl Default for NetworkSerialization {
    fn default() -> Self {
        let mut registry = Registry::default();
        registry.register::<Velocity>(1);
        registry.register::<Transform>(2);
        registry.register::<EntityType>(3);
        NetworkSerialization {
            registry,
            canon: Canon::default(),
        }
    }
}
// TODO: refactor this
impl NetworkSerialization {
    pub fn serialize_client_update(&self, update: &ClientUpdate) -> Vec<u8> {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::serialize(&update).expect("Client update to be seializable")
        })
    }

    pub fn deserialize_client_update(&self, bytes: &[u8]) -> ClientUpdate {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::deserialize(bytes).expect("Client update to be deserializable")
        })
    }

    pub fn serialize_server_update(&self, server_update: &ServerUpdate) -> Vec<u8> {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::serialize(server_update).expect("Server update to be serializable")
        })
    }

    pub fn deserialize_server_update(&self, bytes: &[u8]) -> ServerUpdate {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::deserialize(bytes).expect("Server update to be serializable")
        })
    }

    pub fn deserialize_new_world(&self, world_bytes: &[u8]) -> Result<World> {
        let new_world = self.registry.as_deserialize(&self.canon).deserialize(
            &mut Deserializer::from_slice(
                &world_bytes,
                DefaultOptions::new()
                    .with_fixint_encoding()
                    .allow_trailing_bytes(),
            ),
        )?;
        Ok(new_world)
    }

    pub fn serialize_world<F: LayoutFilter>(&self, world: &World, filter: F) -> Vec<u8> {
        let serilizable_world = world.as_serializable(filter, &self.registry, &self.canon);
        bincode::serialize(&serilizable_world).expect("World to be serializable")
    }
}
