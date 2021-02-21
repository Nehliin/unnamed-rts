use std::thread::JoinHandle;

use bincode::DefaultOptions;
use laminar::{Socket, SocketEvent};
use legion::{serialize::Canon, *};
use log::{error, info};
use serde::de::DeserializeSeed;
use unnamed_rts::resources::NetResource;

pub fn init_client_network(resources: &mut Resources) -> JoinHandle<()> {
    let mut socket = Socket::bind("127.0.0.1:1337").expect("Can't open socket");
    resources.insert(NetResource {
        sender: socket.get_packet_sender(),
        receiver: socket.get_event_receiver(),
    });
    std::thread::spawn(move || {
        // change this later on
        socket.start_polling();
    })
}

pub fn handle_server_update(world: &mut World, registry: &Registry<String>, network: &NetResource, canon: &Canon) {
    for event in network.receiver.try_iter() {
        match event {
            SocketEvent::Packet(packet) => {
                // it's the world content
                use bincode::de::Deserializer;
                let server_update = registry
                    .as_deserialize_into_world(world, canon)
                    .deserialize(&mut Deserializer::from_slice(
                        packet.payload(),
                        DefaultOptions::default(),
                    ))
                    .unwrap();
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
}
