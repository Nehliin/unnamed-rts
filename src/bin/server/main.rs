use laminar::{Config, Packet, Socket, SocketEvent};
use legion::*;
use log::{error, info, warn};
use serialize::Canon;
use std::time::{Duration, Instant};
use systems::CommandBuffer;
use unnamed_rts::resources::{NetResource, Time};
use unnamed_rts::server_systems::*;
use unnamed_rts::{components::*, resources::ClientActions};

// maybe 0: handle connection init
// 1. run system fetching client inputs and add componnents etc
// 2. run game system
// 3. serialize world and send it out at 30hz

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let mut world = World::default();
    let mut resources = Resources::default();
    info!("Starting server..");
    let mut socket = Socket::bind_with_config(
        "127.0.0.1:1338",
        Config {
            heartbeat_interval: Some(Duration::from_millis(50)),
            ..Default::default()
        },
    )
    .expect("failed to open socket");

    resources.insert(Time {
        current_time: Instant::now(),
        delta_time: 0.0,
    });

    resources.insert(NetResource {
        sender: socket.get_packet_sender(),
        receiver: socket.get_event_receiver(),
    });
    // todo switch to uuid? // also break out to the common lib
    let mut registry = Registry::<String>::default();
    let canon = Canon::default();
    registry.register::<Transform>("transform".to_string());
    let mut schedule = Schedule::builder()
        .add_system(client_input_system())
        .add_system(movement_system())
        .build();
    info!("Server started!");
    let mut last_update = Instant::now();
    loop {
        let mut time = resources.get_mut::<Time>().unwrap();
        let now = Instant::now();
        time.delta_time = (now - time.current_time).as_secs_f32();
        time.current_time = now;
        drop(time);

        schedule.execute(&mut world, &mut resources);

        if (last_update - now).as_secs_f32() >= 0.033 {
            send_state(&registry, &world, &resources, &canon);
            //better to do in other thread probably?
            socket.manual_poll(now);
            last_update = now;
        }
    }
}

#[system]
fn client_input(command_buffer: &mut CommandBuffer, #[resource] network: &NetResource) {
    for event in network.receiver.iter() {
        match event {
            SocketEvent::Packet(packet) => {
                if let Ok(client_action) = bincode::deserialize::<ClientActions>(packet.payload()) {
                    match client_action {
                        ClientActions::Move { entity, target } => {
                            command_buffer.add_component(
                                entity,
                                MoveTarget {
                                    target: target.into(),
                                },
                            );
                        }
                    }
                } else {
                    error!("Failed to deserialize packet!");
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

fn send_state(registry: &Registry<String>, world: &World, resources: &Resources, canon: &Canon) {
    let network = resources.get::<NetResource>().unwrap();
    let serilizable_world =
        world.as_serializable(component::<Transform>(), registry, canon);
    let packet = Packet::reliable_sequenced(
        ([127, 0, 0, 1], 1337).into(),
        bincode::serialize(&serilizable_world).expect("failed to serialize"),
        None,
    );
    network.sender.send(packet).unwrap();
}
