use bincode::{de::Deserializer, deserialize};
use bincode::{DefaultOptions, Options};
use crossbeam_channel::{Receiver, Sender};
use glam::Vec3A;
use laminar::{Packet, SocketEvent};
use legion::{any, query::LayoutFilter, serialize::Canon, Entity, Registry, World};
use serde::{de::DeserializeSeed, Deserialize, Serialize};

use crate::components::{EntityType, Transform};

#[derive(Debug)]
pub struct Time {
    pub current_time: std::time::Instant,
    pub delta_time: f32,
}

#[derive(Debug)]
pub struct NetworkSocket {
    pub sender: Sender<Packet>,
    pub receiver: Receiver<SocketEvent>,
}

//Move this
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientUpdate {
    Move { entity: Entity, target: Vec3A },
    StartGame,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum ServerUpdateType {
    InitialState,
    Update,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ServerUpdate {
    pub update_type: ServerUpdateType,
    pub world_bytes: Vec<u8>,
}

pub struct NetworkSerialization {
    registry: Registry<String>,
    canon: Canon,
}

impl Default for NetworkSerialization {
    fn default() -> Self {
        let mut registry = Registry::default();
        registry.register::<Transform>("transform".to_string());
        registry.register::<EntityType>("entity_type".to_string());
        NetworkSerialization {
            registry,
            canon: Canon::default(),
        }
    }
}

impl NetworkSerialization {
    pub fn serialize_client_update(&self, update: &ClientUpdate) -> Vec<u8> {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::serialize(&update).expect("Client action to be seializable")
        })
    }

    pub fn deserialize_client_update(&self, bytes: &[u8]) -> ClientUpdate {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || {
            bincode::deserialize(bytes).expect("Client action to be deserializable")
        })
    }

    pub fn serialize_server_update<F: LayoutFilter>(
        &self,
        update_type: ServerUpdateType,
        world: &World,
        filter: F,
    ) -> Vec<u8> {
        let world_bytes = self.serialize_world(&world, filter);
        bincode::serialize(&ServerUpdate {
            update_type,
            world_bytes,
        })
        .unwrap()
    }

    pub fn deserialize_server_update(&self, bytes: &[u8]) -> ServerUpdate {
        //TODO: avoid double deserilization? also is this even necessary??
        bincode::deserialize(bytes).expect("Can't deserialize server update!")
    }

    pub fn deserialize_new_world(&self, world_bytes: &[u8]) -> World {
        self.registry
            .as_deserialize(&self.canon)
            .deserialize(&mut Deserializer::from_slice(
                &world_bytes,
                DefaultOptions::new()
                    .with_fixint_encoding()
                    .allow_trailing_bytes(),
            ))
            .expect("World to be deserializable")
    }
    pub fn deserialize_into_world(&self, world: &mut World, world_bytes: &[u8]) {
        self.registry
            .as_deserialize_into_world(world, &self.canon)
            .deserialize(&mut Deserializer::from_slice(
                &world_bytes,
                DefaultOptions::new()
                    .with_fixint_encoding()
                    .allow_trailing_bytes(),
            ))
            .expect("World to be deserializable");
    }

    pub fn serialize_world<F: LayoutFilter>(&self, world: &World, filter: F) -> Vec<u8> {
        let serilizable_world = world.as_serializable(filter, &self.registry, &self.canon);
        bincode::serialize(&serilizable_world).expect("World to be serializable")
    }
}
