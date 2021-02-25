use glam::{Quat, Vec3};
use laminar::{Packet, Socket, SocketEvent};
use legion::*;
use log::{error, info, warn};
use std::time::Instant;
use systems::CommandBuffer;
use unnamed_rts::resources::{
    NetworkSerialization, NetworkSocket, ServerUpdate, Time, SERVER_UPDATE_STREAM,
};
use unnamed_rts::server_systems::*;
use unnamed_rts::{components::*, resources::ClientUpdate};

fn setup_world(world: &mut World, net_serilization: &NetworkSerialization) -> Vec<u8> {
    world.extend(vec![
        (
            EntityType::BasicUnit,
            Transform::new(
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.2, 0.2, 0.2),
                Quat::identity(),
            ),
            Velocity {
                velocity: Vec3::splat(0.0),
            },
        ),
        (
            EntityType::BasicUnit,
            Transform::new(
                Vec3::new(-2.0, 0.0, 0.0),
                Vec3::new(0.2, 0.2, 0.2),
                Quat::identity(),
            ),
            Velocity {
                velocity: Vec3::splat(0.0),
            },
        ),
    ]);
    net_serilization.serialize_world(world, any())
}

fn start_game(
    socket: &mut Socket,
    initial_state: Vec<u8>,
    net_serilization: &NetworkSerialization,
) {
    info!("Waiting for client to connect");
    'outer: loop {
        for event in socket.get_event_receiver().try_iter() {
            match event {
                SocketEvent::Packet(packet) => {
                    match net_serilization.deserialize_client_update(&packet.payload()) {
                        ClientUpdate::StartGame => {
                            info!("Starting game!");
                            // keep track of clients here later on, count number of players etc etc
                            let packet = Packet::reliable_ordered(
                                ([127, 0, 0, 1], 1337).into(),
                                initial_state,
                                None,
                            );
                            socket
                                .send(packet)
                                .expect("failed to send start game packet");
                            // not super pretty
                            socket.manual_poll(Instant::now());
                            break 'outer;
                        }
                        _ => {
                            warn!("Unexpected packet, match hasn't started");
                        }
                    }
                }
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
        socket.manual_poll(Instant::now());
    }
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let mut world = World::default();
    let mut resources = Resources::default();
    info!("Starting server..");
    let mut socket = Socket::bind("127.0.0.1:1338").expect("failed to open socket");
    let net_serilization = NetworkSerialization::default();
    let initial_state = setup_world(&mut world, &net_serilization);
    start_game(&mut socket, initial_state, &net_serilization);
    resources.insert(Time {
        current_time: Instant::now(),
        delta_time: 0.0,
    });
    resources.insert(net_serilization);
    resources.insert(NetworkSocket {
        sender: socket.get_packet_sender(),
        receiver: socket.get_event_receiver(),
    });
    let mut schedule = Schedule::builder()
        .add_system(client_input_system())
        .add_system(movement_system())
        .build();
    info!("Game started!");
    let mut last_update = Instant::now();
    loop {
        let mut time = resources.get_mut::<Time>().unwrap();
        let now = Instant::now();
        time.delta_time = (now - time.current_time).as_secs_f32();
        time.current_time = now;
        drop(time);

        schedule.execute(&mut world, &mut resources);
        if (now - last_update).as_secs_f32() >= 0.033 {
            send_state(&world, &resources);
            //better to do in other thread probably?
            socket.manual_poll(now);
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
                match net_serilization.deserialize_client_update(&packet.payload()) {
                    ClientUpdate::Move { entity, target } => {
                        info!("Successfully deserialized packet!");
                        command_buffer.add_component(
                            entity,
                            MoveTarget {
                                target: target.into(),
                            },
                        );
                    }
                    ClientUpdate::StartGame => {
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
    let mut query = <(Entity, Read<Transform>)>::query();
    let transforms: Vec<(Entity, Transform)> = query.iter(world).map(|(e, t)| (*e, *t)).collect();
    let server_update = ServerUpdate::State { transforms };
    let payload = net_serilization.serialize_server_update(&server_update);
    let packet = Packet::unreliable_sequenced(
        ([127, 0, 0, 1], 1337).into(),
        payload,
        Some(SERVER_UPDATE_STREAM),
    );
    network.sender.send(packet).unwrap();
}
