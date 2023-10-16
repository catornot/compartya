use compartya_shared::{LobbyUid, PacketMessage, PacketResponse, PartyaError, SentPacket};
use laminar::{Config, Packet, Socket, SocketEvent};
use std::{net::SocketAddr, time::Duration};

const SERVER_ADDR: &str = "192.168.0.243:2000";
// const SERVER_ADDR: &str = "127.0.0.1:2000";

#[derive(Default, Debug)]
pub struct Server {
    lobby_connections: Vec<(LobbyUid, SocketAddr)>,
}

pub fn main() -> Result<(), ()> {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let mut server = Server::default();

    let mut socket = Socket::bind_with_config(
        SERVER_ADDR,
        Config {
            idle_connection_timeout: Duration::from_secs(5),
            ..Default::default()
        },
    )
    .map_err(|err| _ = log::error!("failed to setup socket {err}"))?;
    let (send_socket, recv_socket) = (socket.get_packet_sender(), socket.get_event_receiver());
    std::thread::spawn(move || socket.start_polling());

    log::info!("got a socket connection {SERVER_ADDR}");

    loop {
        let Ok(event) = recv_socket.recv() else {
            continue;
        };

        match event {
            SocketEvent::Packet(packet) => {
                let recv_packet: SentPacket = match packet.payload().try_into() {
                    Ok(p) => p,
                    Err(err) => {
                        log::info!("packet desiriazation failed {err}");
                        continue;
                    }
                };
                let addr = packet.addr();

                let maybe_err = match recv_packet {
                    SentPacket::PacketMessage(msg) => {
                        process_message(addr, msg, &send_socket, &mut server)
                    }
                    SentPacket::PacketResponse(response) => {
                        process_response(addr, response, &send_socket, &mut server)
                    }
                };

                if let Err(err) = maybe_err {
                    if let PartyaError::IllegalPacket(_) = err {
                        remove_from_server(&mut server, &addr);
                    }
                    log::error!("{err}");
                }
            }
            SocketEvent::Connect(addr) => log::info!("{} connected", addr),
            SocketEvent::Timeout(_) => {}
            SocketEvent::Disconnect(addr) => {
                remove_from_server(&mut server, &addr);
                log::info!("{} disconnected", addr)
            }
        }
    }
}

fn process_message(
    addr: SocketAddr,
    msg: PacketMessage,
    send_socket: &crossbeam_channel::Sender<Packet>,
    server: &mut Server,
) -> Result<(), PartyaError> {
    let lobby = server.lobby_connections.iter().find(|(_, a)| *a == addr);

    match (msg, lobby) {
        (PacketMessage::FindLobby(lobby_id), None) => {
            let Some(lobby) = server
                .lobby_connections
                .iter()
                .find(|(id, _)| *id == lobby_id)
            else {
                log::error!("didn't find lobby for {addr}");

                _ = send_socket.send(Packet::reliable_unordered(
                    addr,
                    PacketResponse::NoLobby(lobby_id).send().try_into()?,
                ));
                return Ok(());
            };

            log::info!("found lobby for {addr}");

            _ = send_socket.send(Packet::reliable_unordered(
                addr,
                PacketResponse::FoundLobby(lobby.1).send().try_into()?,
            ));
        }
        (PacketMessage::CreateLobby, None) => {
            let id = nanoid::nanoid!(8)
                .chars()
                .collect::<Vec<char>>()
                .try_into()
                .expect("can't fail to collect a 5 len vec into a 5 len array");

            log::info!("creating lobby for {addr}");

            server.lobby_connections.push((id, addr));

            _ = send_socket.send(Packet::reliable_unordered(
                addr,
                PacketResponse::CreatedLobby(id).send().try_into()?,
            ))
        }
        (PacketMessage::Ping(_), Some(_)) => {
            _ = send_socket.send(Packet::unreliable(
                addr,
                PacketResponse::Pong.send().try_into()?,
            ))
        }
        (PacketMessage::Ping(_), _) => {}
        (m, _) => Err(PartyaError::IllegalPacket(Box::new(m.send())))?,
    }

    Ok(())
}

fn process_response(
    _addr: SocketAddr,
    response: PacketResponse,
    _send_socket: &crossbeam_channel::Sender<Packet>,
    _server: &mut Server,
) -> Result<(), PartyaError> {
    match response {
        PacketResponse::Pong => {
            // _ = send_socket.send(Packet::unreliable(
            //     addr,
            //     PacketMessage::Ping(None).send().try_into()?,
            // ))
        }
        r => Err(PartyaError::IllegalPacket(Box::new(r.send())))?,
    }

    Ok(())
}

fn remove_from_server(server: &mut Server, addr: &SocketAddr) {
    server
        .lobby_connections
        .iter()
        .position(|(_, a)| a == addr)
        .map(|i| _ = server.lobby_connections.swap_remove(i));
}
