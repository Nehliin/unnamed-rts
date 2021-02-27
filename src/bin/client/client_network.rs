use laminar::{Config, Packet, Socket, SocketEvent};
use legion::{systems::CommandBuffer, EntityStore, *};
use log::{error, info, warn};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    net::{IpAddr, SocketAddr},
    thread::JoinHandle,
    time::Duration,
};
use unnamed_rts::{
    components::{EntityType, Selectable, Transform},
    resources::{
        ClientUpdate, NetworkSerialization, NetworkSocket, ServerUpdate, SERVER_ADDR, SERVER_PORT,
    },
};

use crate::{assets::Handle, graphics::model::Model};

pub fn init_client_network(resources: &mut Resources) -> JoinHandle<()> {
    let mut socket = Socket::bind_any_with_config(Config {
        heartbeat_interval: Some(Duration::from_millis(1000)),
        ..Default::default()
    })
    .expect("Can't open socket");
    let local_addr = socket
        .local_addr()
        .expect("There must exist a local addr the socket is bound to");
    let ip = if let IpAddr::V4(ipv4) = local_addr.ip() {
        ipv4.octets()
    } else {
        panic!("Expect to be bound to ipV4 addr");
    };
    resources.insert(NetworkSocket {
        sender: socket.get_packet_sender(),
        receiver: socket.get_event_receiver(),
        ip,
        port: local_addr.port(),
    });
    std::thread::spawn(move || {
        // change this later on
        socket.start_polling();
    })
}

pub fn connect_to_server(world: &mut World, resources: &mut Resources) {
    let socket = resources.get::<NetworkSocket>().unwrap();
    // Tell server to start the game
    let serialized = bincode::serialize(&ClientUpdate::StartGame {
        ip: socket.ip,
        port: socket.port,
    })
    .expect("Serilization to work");

    let packet =
        Packet::reliable_unordered(SocketAddr::new(SERVER_ADDR.into(), SERVER_PORT), serialized);
    let network = resources.get::<NetworkSocket>().unwrap();
    let net_serialization = resources.get::<NetworkSerialization>().unwrap();
    network.sender.send(packet).unwrap();
    // wait for initial game state
    for event in network.receiver.iter() {
        match event {
            SocketEvent::Packet(packet) => {
                if let Ok(mut initial_state) =
                    net_serialization.deserialize_new_world(packet.payload())
                {
                    world.move_from(&mut initial_state, &any());
                    break;
                } else {
                    warn!("Unexpected server packet: Expected initial state")
                }
            }
            SocketEvent::Connect(addr) => {
                info!("Connected to server: {}", addr);
            }
            _ => error!("Unexpected socket event, client should not yet have connected"),
        }
    }
}

pub fn add_client_components(world: &mut World, resources: &mut Resources, suit: &Handle<Model>) {
    let mut query = <(Entity, Read<EntityType>)>::query();
    let mut command_buffer = CommandBuffer::new(&world);
    for (entity, _entity_type) in query.iter(world) {
        command_buffer.add_component(*entity, suit.clone());
        command_buffer.add_component(*entity, Selectable { is_selected: false });
    }
    command_buffer.flush(world, resources);
}

pub fn handle_server_update(world: &mut World, resources: &mut Resources) {
    let network = resources.get::<NetworkSocket>().unwrap();
    let net_serialization = resources.get::<NetworkSerialization>().unwrap();
    for event in network.receiver.try_iter() {
        match event {
            SocketEvent::Packet(packet) => {
                let ServerUpdate::State { transforms } =
                    net_serialization.deserialize_server_update(&packet.payload());
                // Safety: there must be a unique entity id per element in the update which is currently
                // guarenteed by the server query that creates the transform vec
                transforms
                    .into_par_iter()
                    .for_each(|(entity, new_transform)| {
                        let entry = world.entry_ref(entity).unwrap();
                        unsafe {
                            let transform = entry.get_component_unchecked::<Transform>().unwrap();
                            *transform = new_transform;
                        }
                    });
            }
            SocketEvent::Connect(addr) => {
                info!("Connected to server at: {}", addr);
            }
            SocketEvent::Timeout(_addr) => {
                error!("Server Timed out!");
            }
            SocketEvent::Disconnect(_addr) => {
                error!("Server disconnected!")
            }
        }
    }
}
