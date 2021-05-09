use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};

use anyhow::Result;
use bincode::de::Deserializer;
use bincode::{DefaultOptions, Options};
use crossbeam_channel::{Receiver, Sender};
use glam::Vec3;
use laminar::{Config, Packet, Socket, SocketEvent};
use legion::{query::LayoutFilter, serialize::Canon, *};
use serde::{de::DeserializeSeed, Deserialize, Serialize};

use crate::components::{EntityType, Transform, Velocity};
#[derive(Debug, Clone, Copy)]
pub struct WindowSize {
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f32,
}

impl WindowSize {
    pub fn logical_size(&self) -> (u32, u32) {
        let logical_width = self.physical_width as f32 / self.scale_factor;
        let logical_height = self.physical_height as f32 / self.scale_factor;
        (logical_width as u32, logical_height as u32)
    }
}

#[derive(Debug)]
pub struct DebugRenderSettings {
    pub show_grid: bool,
    pub show_bounding_boxes: bool,
}

#[derive(Debug)]
pub struct Time {
    pub current_time: std::time::Instant,
    pub delta_time: f32,
}

#[derive(Debug)]
// Make construction private
#[non_exhaustive]
pub struct NetworkSocket {
    pub sender: Sender<Packet>,
    pub receiver: Receiver<SocketEvent>,
    pub ip: [u8; 4],
    pub port: u16,
}

impl NetworkSocket {
    fn from_socket(mut socket: Socket) -> NetworkSocket {
        let local_addr = socket
            .local_addr()
            .expect("There must exist a local addr the socket is bound to");
        let ip = if let IpAddr::V4(ipv4) = local_addr.ip() {
            ipv4.octets()
        } else {
            panic!("Expect to be bound to ipV4 addr");
        };
        let network_socket = NetworkSocket {
            sender: socket.get_packet_sender(),
            receiver: socket.get_event_receiver(),
            ip,
            port: local_addr.port(),
        };
        std::thread::spawn(move || socket.start_polling());
        network_socket
    }

    pub fn bind_any_with_config(config: Config) -> NetworkSocket {
        let socket = Socket::bind_any_with_config(config).expect("Failed to open socket");
        NetworkSocket::from_socket(socket)
    }

    pub fn bind_with_config<A: ToSocketAddrs>(addresses: A, config: Config) -> NetworkSocket {
        let socket = Socket::bind_with_config(addresses, config).expect("Failed to open socket");
        NetworkSocket::from_socket(socket)
    }
}

//Move this
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ClientUpdate {
    Move { entity: Entity, path: Vec<Vec3> },
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
