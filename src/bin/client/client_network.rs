use glam::Vec3;
use laminar::{Packet, Socket, SocketEvent};
use legion::{ systems::CommandBuffer, *};
use log::{error, info};
use std::{
    net::{ Ipv4Addr, SocketAddr},
    thread::JoinHandle,
};
use unnamed_rts::{components::{EntityType, Selectable, Transform, Velocity}, resources::{ClientUpdate, NetworkSerialization, NetworkSocket, ServerUpdate, ServerUpdateType}};

use crate::{assets::Assets, graphics::model::Model};

pub const SERVER_ADDR: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
pub const SERVER_PORT: u16 = 1338;

pub fn init_client_network(resources: &mut Resources) -> JoinHandle<()> {
    let mut socket = Socket::bind("127.0.0.1:1337").expect("Can't open socket");
    resources.insert(NetworkSocket {
        sender: socket.get_packet_sender(),
        receiver: socket.get_event_receiver(),
    });
    std::thread::spawn(move || {
        // change this later on
        socket.start_polling();
    })
}

pub fn connect_to_server(resources: &Resources) {
    let serialized = bincode::serialize(&ClientUpdate::StartGame).expect("Serilization to work");
    let packet =
        Packet::reliable_unordered(SocketAddr::new(SERVER_ADDR.into(), SERVER_PORT), serialized);
    let net_resources = resources.get::<NetworkSocket>().unwrap();
    net_resources.sender.send(packet).unwrap();
}

pub fn add_client_components(world: &mut World, resources: &mut Resources) {
    let mut assets = resources.get_mut::<Assets<Model>>().unwrap();
    let suit = assets.load("nanosuit/nanosuit.obj").unwrap();
    let mut query = <(Entity, Read<EntityType>)>::query();
    let mut command_buffer = CommandBuffer::new(&world);
    for (entity, entity_type) in query.iter(world) {
        println!("{:?}", entity);
        command_buffer.add_component(*entity, suit.clone());
        command_buffer.add_component(*entity, Selectable { is_selected: false });
        command_buffer.add_component(
            *entity,
            Velocity {
                velocity: Vec3::splat(0.0),
            },
        );
    }
    drop(assets);
    command_buffer.flush(world, resources)
}

pub fn handle_server_update(world: &mut World, resources: &mut Resources) -> bool {
    let network = resources.get::<NetworkSocket>().unwrap();
    let net_serialization = resources.get::<NetworkSerialization>().unwrap();
    let mut should_update = false;
    for event in network.receiver.try_iter() {
        match event {
            SocketEvent::Packet(packet) => {
                info!("Received server update!");
                let server_update =  net_serialization.deserialize_server_update(&packet.payload());
                match server_update.update_type {
                    ServerUpdateType::InitialState => {
                        let mut new_world = net_serialization.deserialize_new_world(&server_update.world_bytes);
                        world.move_from(&mut new_world, &any());
                        should_update = true;
                        
                    }
                    ServerUpdateType::Update => {
                        net_serialization.deserialize_into_world(world, &server_update.world_bytes);
                    }
                }
                
            }
            SocketEvent::Connect(addr) => {
                info!("Connected to server at: {}", addr);
            }
            SocketEvent::Timeout(addr) => {
                error!("Server Timedout!");
            }
            SocketEvent::Disconnect(addr) => {
                error!("Server disconnected!")
            }
        }
    }
    should_update
}
