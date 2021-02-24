use bincode::{de::Deserializer, deserialize};
use bincode::{DefaultOptions, Options};
use crossbeam_channel::{Receiver, Sender};
use glam::Vec3A;
use laminar::{Packet, SocketEvent};
use legion::{query::LayoutFilter, serialize::Canon, *};
use legion_typeuuid::{collect_registry, register_serialize, SerializableTypeUuid};
use serde::{de::DeserializeSeed, Deserialize, Serialize};

use crate::components::{EntityType, MoveTarget, Transform, Velocity};

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
    registry: Registry<SerializableTypeUuid>,
    canon: Canon,
}

impl Default for NetworkSerialization {
    fn default() -> Self {
        // This is very annoying, but needed because of custom legion version
        let mut registry = Registry::default();
        let uuid = SerializableTypeUuid::parse_str("1d97d71a-76bf-41d1-94c3-fcaac8231f12").unwrap();
        registry.register::<Velocity>(uuid);
        let uuid = SerializableTypeUuid::parse_str("51b229c8-4b3e-4462-b4bf-5ebeb80880e6").unwrap();
        registry.register::<MoveTarget>(uuid);
        let uuid = SerializableTypeUuid::parse_str("71d225bb-a312-45b8-85af-d98649804ac8").unwrap();
        registry.register::<Transform>(uuid);
        let uuid = SerializableTypeUuid::parse_str("a22e4176-748f-4882-8376-b1047b130caf").unwrap();
        registry.register::<EntityType>(uuid);
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
        match update_type {
            ServerUpdateType::InitialState => {
                let world_bytes = self.serialize_world(&world, filter);
                bincode::serialize(&ServerUpdate {
                    update_type,
                    world_bytes,
                })
                .unwrap()
            }
            ServerUpdateType::Update => {
                let mut query = <(Entity, Read<Transform>)>::query();
                let test: Vec<(Entity, Transform)> =
                    query.iter(world).map(|(e, t)| (*e, *t)).collect();
                use legion::serialize::set_entity_serializer;
                set_entity_serializer(&self.canon, || {
                    bincode::serialize(&ServerUpdate {
                        update_type,
                        world_bytes: bincode::serialize(&test).unwrap(),
                    })
                    .unwrap()
                })
            }
        }
    }

    pub fn deserialize_update(&self, bytes: &[u8]) -> Vec<(Entity, Transform)> {
        use legion::serialize::set_entity_serializer;
        set_entity_serializer(&self.canon, || bincode::deserialize(bytes).unwrap())
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
