use glam::Vec3;
use laminar::{Config, Packet, SocketEvent};
use legion::{systems::CommandBuffer, world::SubWorld, EntityStore, *};
use log::{error, info, warn};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{net::SocketAddr, time::Duration};
use unnamed_rts::{
    components::{EntityType, Selectable, Transform},
    resources::{
        ClientUpdate, NetworkSerialization, NetworkSocket, ServerUpdate, SERVER_ADDR, SERVER_PORT,
    },
};

use unnamed_rts::{
    assets::Handle,
    graphics::{gltf::GltfModel, lights::PointLight},
};

pub fn connect_to_server(world: &mut World, resources: &mut Resources) {
    let socket = NetworkSocket::bind_any_with_config(Config {
        heartbeat_interval: Some(Duration::from_millis(1000)),
        ..Default::default()
    });
    // Tell server to start the game
    let serialized = bincode::serialize(&ClientUpdate::StartGame {
        ip: socket.ip,
        port: socket.port,
    })
    .expect("Serilization to work");
    let packet =
        Packet::reliable_unordered(SocketAddr::new(SERVER_ADDR.into(), SERVER_PORT), serialized);
    let net_serialization = resources.get::<NetworkSerialization>().unwrap();
    socket.sender.send(packet).unwrap();
    // wait for initial game state
    for event in socket.receiver.iter() {
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
    drop(net_serialization);
    resources.insert(socket);
}

pub fn add_client_components(
    world: &mut World,
    resources: &mut Resources,
    model: &Handle<GltfModel>,
) {
    let mut query = <(Entity, Read<EntityType>)>::query();
    let mut command_buffer = CommandBuffer::new(&world);
    for (entity, _entity_type) in query.iter(world) {
        command_buffer.add_component(*entity, model.clone());
        command_buffer.add_component(*entity, Selectable { is_selected: false });
    }
    command_buffer.extend(vec![
        (
            PointLight {
                color: Vec3::new(1.0, 0.0, 0.0).into(),
                position: Vec3::new(1.0, 1.0, 0.0).into(),
            },
            (),
        ),
        (
            PointLight {
                color: Vec3::new(0.0, 0.0, 1.0).into(),
                position: Vec3::new(-1.0, 1.0, 0.0).into(),
            },
            (),
        ),
    ]);
    command_buffer.flush(world, resources);
}

// If this ever leads to problems (SubWorld for example not including all the necessary entities)
// Then revert to taking entire world and resources as args instead of having this as a system. Then put
// it on_foreground tick instead
#[system]
pub fn server_update(
    world: &mut SubWorld,
    #[resource] network: &NetworkSocket,
    #[resource] net_serialization: &NetworkSerialization,
    _query: &mut Query<&mut Transform>,
) {
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
