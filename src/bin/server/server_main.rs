use glam::{Quat, Vec3};
use laminar::{Config, Packet, SocketEvent};
use legion::*;
use log::{error, info, warn};
use mimalloc::MiMalloc;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use server_systems::*;
use std::{
    net::{SocketAddr, SocketAddrV4},
    time::Instant,
};
use systems::CommandBuffer;
use unnamed_rts::resources::{
    NetworkSerialization, NetworkSocket, ServerUpdate, Time, SERVER_ADDR, SERVER_PORT,
    SERVER_UPDATE_STREAM,
};
use unnamed_rts::{components::*, resources::ClientUpdate};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod server_systems;
#[derive(Debug, Default)]
struct ConnectedClients {
    // hash set?
    addrs: Vec<SocketAddrV4>,
}

fn setup_world(
    world: &mut World,
    _resources: &mut Resources,
    net_serilization: &NetworkSerialization,
) -> Vec<u8> {
    world.extend(vec![
        (
            EntityType::BasicUnit,
            Transform::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY),
            Velocity {
                velocity: Vec3::splat(0.0),
            },
        ),
        /*(
            EntityType::BasicUnit,
            Transform::new(
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
                Quat::IDENTITY,
            ),
            Velocity {
                velocity: Vec3::splat(0.0),
            },
        ),*/
    ]);
    // This must be synced with the clients
    //    let map = TileMap::load(std::path::Path::new("assets/Tilemap.map")).unwrap();
    //    resources.insert(map);
    net_serilization.serialize_world(world, any())
}

fn start_game(
    socket: &NetworkSocket,
    initial_state: Vec<u8>,
    net_serilization: &NetworkSerialization,
    connected_clients: &mut ConnectedClients,
    num_players: u8,
) {
    info!("Waiting for {} clients to connect", num_players);
    for event in socket.receiver.iter() {
        match event {
            SocketEvent::Packet(packet) => {
                match net_serilization.deserialize_client_update(packet.payload()) {
                    ClientUpdate::StartGame { ip, port } => {
                        let addr = SocketAddrV4::new(ip.into(), port);
                        if !connected_clients.addrs.contains(&addr) {
                            connected_clients.addrs.push(addr);
                            info!("Connected client: {}", addr);
                            if num_players as usize <= connected_clients.addrs.len() {
                                break;
                            }
                        }
                    }
                    _ => {
                        warn!("Unexpected packet, match hasn't started");
                    }
                }
            }
            // maybe use this instead to record connected clients?
            SocketEvent::Connect(_) => {
                info!("Connection!");
            }
            SocketEvent::Timeout(_) => {
                warn!("timeout")
            }
            SocketEvent::Disconnect(_) => {
                error!("Disconnect!");
            }
        }
    }
    info!("All players connected, starting game!");
    connected_clients
        .addrs
        .par_iter()
        .for_each(move |client_addr| {
            let packet =
                Packet::reliable_ordered(SocketAddr::V4(*client_addr), initial_state.clone(), None);
            socket
                .sender
                .send(packet)
                .expect("failed to send start game packet");
        });
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
    info!("Starting server..");
    let net_serilization = NetworkSerialization::default();
    let network_socket = NetworkSocket::bind_with_config(
        SocketAddrV4::new(SERVER_ADDR, SERVER_PORT),
        Config::default(),
    );

    let mut world = World::default();
    let mut resources = Resources::default();
    let initial_state = setup_world(&mut world, &mut resources, &net_serilization);
    let mut connected_clients = ConnectedClients::default();
    start_game(
        &network_socket,
        initial_state,
        &net_serilization,
        &mut connected_clients,
        1,
    );
    resources.insert(Time::default());
    resources.insert(net_serilization);
    resources.insert(network_socket);
    resources.insert(connected_clients);

    let mut schedule = Schedule::builder()
        .add_system(client_input_system())
        .add_system(movement_system())
        .build();

    info!("Game started!");
    let mut last_update = Instant::now();
    loop {
        let mut time = resources.get_mut::<Time>().unwrap();
        time.update();
        let now = *time.current_time();
        drop(time);
        schedule.execute(&mut world, &mut resources);
        // TODO: this isn't fixed timestep
        // see: https://gafferongames.com/post/fix_your_timestep/
        if (now - last_update).as_secs_f32() >= 0.033 {
            send_state(&world, &resources);
            last_update = now;
        }
    }
}

#[system]
fn client_input(
    command_buffer: &mut CommandBuffer,
    #[resource] network: &NetworkSocket,
    #[resource] net_serilization: &NetworkSerialization,
) {
    for event in network.receiver.try_iter() {
        match event {
            SocketEvent::Packet(packet) => {
                match net_serilization.deserialize_client_update(packet.payload()) {
                    ClientUpdate::Move { entity, target } => {
                        info!("Successfully deserialized packet!");
                        command_buffer.add_component(
                            entity,
                            MoveTarget {
                                target: target.into(),
                            },
                        );
                    }
                    ClientUpdate::StartGame { .. } => {
                        warn!("unexpected packet");
                    }
                }
            }
            SocketEvent::Connect(addr) => {
                info!("Connected to: {}", addr);
            }
            SocketEvent::Timeout(addr) => {
                error!("Timeout to: {}", addr);
            }
            SocketEvent::Disconnect(addr) => {
                warn!("Disconnected from: {}", addr);
            }
        }
    }
}

fn send_state(world: &World, resources: &Resources) {
    let network = resources.get::<NetworkSocket>().unwrap();
    let net_serilization = resources.get::<NetworkSerialization>().unwrap();
    let connected_clients = resources.get::<ConnectedClients>().unwrap();
    let mut query = <(Entity, Read<Transform>)>::query();
    let transforms: Vec<(Entity, Transform)> = query.iter(world).map(|(e, t)| (*e, *t)).collect();
    let server_update = ServerUpdate::State { transforms };
    let payload = net_serilization.serialize_server_update(&server_update);
    connected_clients.addrs.par_iter().for_each(|client_addr| {
        let packet = Packet::unreliable_sequenced(
            SocketAddr::V4(*client_addr),
            payload.clone(),
            Some(SERVER_UPDATE_STREAM),
        );
        network.sender.send(packet).unwrap();
    });
}
