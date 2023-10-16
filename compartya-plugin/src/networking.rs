use compartya_shared::{PacketMessage, PacketResponse, PartyaError, PlayerUid, SentPacket};
use laminar::{ErrorKind, Packet, Socket, SocketEvent};
use rrplug::prelude::*;
use std::{
    net::SocketAddr,
    sync::mpsc::{Receiver, Sender},
};

use crate::{ConnectionState, Host, LocalMessage, User, MATCHMAKING_SERVER_ADDR};

pub fn run_connections(
    recv_tf2: Receiver<LocalMessage>,
    send_tf2: Sender<LocalMessage>,
    addr: String,
) -> Result<(), ErrorKind> {
    let mut state = ConnectionState::User(User::default());
    // let last_ping = Instant::now();

    let mut socket = Socket::bind(addr.clone())?;
    let (send_socket, recv_socket) = (socket.get_packet_sender(), socket.get_event_receiver());
    std::thread::spawn(move || socket.start_polling());

    log::info!("got a socket connection {addr}");

    let (send_ping, recv_ping) = std::sync::mpsc::channel();
    let socket_send = send_socket.clone();
    std::thread::spawn(move || run_ping_thread(socket_send, recv_ping));

    loop {
        if let Ok(lmsg) = recv_tf2.try_recv() {
            match (lmsg, &mut state) {
                (LocalMessage::ConnectToLobby(lobby_id, password), ConnectionState::User(user)) => {
                    log::info!(
                        "trying connecting to {} with password {}",
                        lobby_id.iter().collect::<String>(),
                        password.iter().collect::<String>()
                    );

                    user.password = password;

                    _ = send_socket.send(Packet::reliable_unordered(
                        MATCHMAKING_SERVER_ADDR,
                        PacketMessage::FindLobby(lobby_id)
                            .send()
                            .try_into()
                            .expect("this shouldn't crash connect to lobby find lobby; report to catornot pls"),
                    ));
                }
                (LocalMessage::BecomeHost(password), ConnectionState::User(_)) => {
                    log::info!("became host");
                    state = ConnectionState::Host(Host {
                        password,
                        ..Default::default()
                    });

                    _ = send_socket.send(Packet::reliable_unordered(
                        MATCHMAKING_SERVER_ADDR,
                        PacketMessage::CreateLobby
                            .send()
                            .try_into()
                            .expect("this shouldn't crash connect to lobby find lobby; report to catornot pls"),
                    ));
                }
                (LocalMessage::BecomeUser, ConnectionState::Host(_)) => {
                    log::info!("became user");
                    state = ConnectionState::User(User::default());
                }
                (LocalMessage::Leave, _) => {
                    log::info!("left current state");
                    state = ConnectionState::User(User::default());
                }
                (LocalMessage::NewOrder(order), ConnectionState::Host(host)) => {
                    host.clients
                        .iter()
                        .map(|(addr, id)| {
                            (
                                PacketMessage::NewOrder(*id, order.clone())
                                    .send()
                                    .try_into(),
                                addr,
                            )
                        })
                        .filter_map(|(maybe_err, addr)| {
                            Some((
                                maybe_err
                                    .map_err(|err| {
                                        _ = log::info!("failed to build order packet {err}")
                                    })
                                    .ok()?,
                                addr,
                            ))
                        })
                        .for_each(|(packet, addr)| {
                            _ = send_socket.send(Packet::reliable_unordered(*addr, packet))
                        });
                }
                (LocalMessage::ExecuteFunction(func), _) => {
                    _ = send_tf2.send(LocalMessage::ExecuteFunction(func));
                }
                (
                    LocalMessage::BecomeUser
                    | LocalMessage::BecomeHost(_)
                    | LocalMessage::ConnectToLobby(_, _)
                    | LocalMessage::ExecuteOrder(_)
                    | LocalMessage::NewOrder(_),
                    _,
                ) => {}
            }
        }

        // const SECONDS_2: Duration = Duration::from_secs(2);
        // if last_ping.elapsed() > SECONDS_2 {
        //     match &state {
        //         ConnectionState::User(user) if user.server.is_some() => {
        //             _ = send_socket.send(Packet::unreliable(
        //                 user.server.unwrap(),
        //                 PacketMessage::Ping(Some(user.uid))
        //                     .send()
        //                     .try_into()
        //                     .expect("ping build failed"),
        //             ));
        //         }
        //         ConnectionState::Host(host)
        //             if host.lobby_id.is_some() && host.lobby_id.unwrap() != LobbyUid::default() =>
        //         {
        //             _ = send_socket.send(Packet::unreliable(
        //                 MATCHMAKING_SERVER_ADDR,
        //                 PacketMessage::Ping(None)
        //                     .send()
        //                     .try_into()
        //                     .expect("ping build failed"),
        //             ));
        //         }
        //         _ => {}
        //     }
        // }

        let Ok(event) = recv_socket.try_recv() else {
            continue;
        };

        match (event, &mut state) {
            (SocketEvent::Packet(packet), _) => {
                let recv_packet: SentPacket = match packet.payload().try_into() {
                    Ok(p) => p,
                    Err(err) => {
                        log::info!("packet desiriazation failed {err}");
                        continue;
                    }
                };
                let addr = packet.addr();

                let maybe_err = match recv_packet {
                    SentPacket::PacketMessage(msg) => match &mut state {
                        ConnectionState::Host(host) => {
                            process_message_host(addr, msg, &send_socket, host)
                        }
                        ConnectionState::User(user) => {
                            process_message_user(addr, msg, &send_socket, user, &send_tf2)
                        }
                    },
                    SentPacket::PacketResponse(response) => {
                        process_response(addr, response, &send_socket, &mut state, &send_ping)
                    }
                };

                if let Err(err) = maybe_err {
                    if let (PartyaError::IllegalUid(_, addr), ConnectionState::Host(host)) =
                        (&err, &mut state)
                    {
                        remove_from_host(host, addr);
                    }

                    log::error!("{err}");
                }
            }
            (SocketEvent::Connect(_), ConnectionState::User(_)) => {}
            (SocketEvent::Connect(addr), _) => log::info!("{} connected", addr),
            (SocketEvent::Timeout(_), _) => {}
            (SocketEvent::Disconnect(addr), ConnectionState::User(user)) => {
                // log::warn!("{} disconnected", addr);

                if user.server == Some(addr) {
                    user.server = None
                }
            }
            (SocketEvent::Disconnect(addr), ConnectionState::Host(host)) => {
                log::warn!("{} disconnected", addr);

                remove_from_host(host, &addr);

                if addr == MATCHMAKING_SERVER_ADDR {
                    host.lobby_id = None
                }
            }
        }
    }
}

fn process_message_host(
    addr: SocketAddr,
    msg: PacketMessage,
    send_socket: &crossbeam_channel::Sender<Packet>,
    state: &mut Host,
) -> Result<(), PartyaError> {
    let conn = state.clients.iter().find(|(a, _)| addr == *a);

    match (msg, conn) {
        (PacketMessage::Auth(password), None) => {
            if password != state.password {
                _ = send_socket.send(Packet::reliable_unordered(
                    addr,
                    PacketResponse::FailedAuth.send().try_into()?,
                ));

                return Ok(());
            }

            log::info!("{addr} authenticated with lobby");

            let id = nanoid::nanoid!(5)
                .chars()
                .collect::<Vec<char>>()
                .try_into()
                .expect("can't fail to collect a 5 len vec into a 5 len array");

            state.clients.push((addr, id));

            _ = send_socket.send(Packet::reliable_unordered(
                addr,
                PacketResponse::AuthAccepted(id, state.password)
                    .send()
                    .try_into()?,
            ));
        }
        (PacketMessage::GetLastOrder(uid), Some(conn)) => {
            if uid != conn.1 {
                return Err(PartyaError::IllegalUid(conn.1, conn.0));
            }

            _ = send_socket.send(Packet::reliable_unordered(
                addr,
                PacketMessage::NewOrder(uid, state.last_order.clone())
                    .send()
                    .try_into()?,
            ));
        }
        (PacketMessage::Ping(Some(uid)), Some(conn)) => {
            if uid != conn.1 {
                return Err(PartyaError::IllegalUid(conn.1, conn.0));
            }

            _ = send_socket.send(Packet::unreliable(
                addr,
                PacketResponse::Pong.send().try_into()?,
            ))
        } // should limit this
        _ => log::warn!("received a unexpected host message packet"),
    }

    Ok(())
}

fn process_message_user(
    addr: SocketAddr,
    msg: PacketMessage,
    send_socket: &crossbeam_channel::Sender<Packet>,
    state: &mut User,
    send_tf2: &Sender<LocalMessage>,
) -> Result<(), PartyaError> {
    match msg {
        PacketMessage::NewOrder(uid, order) if uid == state.uid => send_tf2
            .send(LocalMessage::ExecuteOrder(order))
            .expect("somehow a channel broke"),
        PacketMessage::Ping(Some(uid)) if uid == state.uid => {
            _ = send_socket.send(Packet::unreliable(
                addr,
                PacketResponse::Pong.send().try_into()?,
            ))
        }
        _ => log::warn!("received a unexpected user message packet"),
    }

    Ok(())
}

fn process_response(
    addr: SocketAddr,
    response: PacketResponse,
    send_socket: &crossbeam_channel::Sender<Packet>,
    state: &mut ConnectionState,
    send_ping: &Sender<(SocketAddr, Option<PlayerUid>)>,
) -> Result<(), PartyaError> {
    match (response, state) {
        (PacketResponse::AuthAccepted(uid, password), ConnectionState::User(user))
            if user.password == password && user.server.is_none() =>
        {
            log::info!("authenticated with lobby");

            user.server = Some(addr);
            user.uid = uid;

            _ = send_socket.send(Packet::reliable_unordered(
                addr,
                PacketMessage::GetLastOrder(user.uid).send().try_into()?,
            ));

            _ = send_ping.send((addr, Some(uid)));
        }
        (PacketResponse::FailedAuth, ConnectionState::User(user)) => {
            log::error!("failed to authenticate with lobby {:?}", user.server)
        }
        (PacketResponse::FoundLobby(lobby_addr), ConnectionState::User(user)) => {
            log::info!("found lobby connecting");

            _ = send_socket.send(Packet::reliable_unordered(
                lobby_addr,
                PacketMessage::Auth(user.password).send().try_into()?,
            ));
        }
        (PacketResponse::NoLobby(lobby_id), ConnectionState::User(_)) => {
            log::info!(
                "failed to find lobby {}",
                lobby_id.into_iter().collect::<String>()
            );
        }
        (PacketResponse::CreatedLobby(lobby_id), ConnectionState::Host(host)) => {
            host.lobby_id = Some(lobby_id);

            log::info!(
                "created a lobby {}",
                lobby_id.into_iter().collect::<String>()
            );

            _ = send_ping.send((addr, None));
        }
        (PacketResponse::Pong, ConnectionState::Host(host)) => {
            if let Some((_, uid)) = host.clients.iter().find(|(a, _)| a == &addr) {
                _ = send_ping.send((addr, Some(*uid)));
            } else if addr == MATCHMAKING_SERVER_ADDR {
                _ = send_ping.send((addr, None));
            }
        } // pong comfirmed
        (PacketResponse::Pong, ConnectionState::User(user)) => {
            if user.server == Some(addr) {
                _ = send_ping.send((addr, Some(user.uid)));
            }
        } // pong comfirmed
        (_, ConnectionState::User(_)) => log::warn!("received a unexpected user message packet"),
        (_, ConnectionState::Host(_)) => log::warn!("received a unexpected host message packet"),
    }

    Ok(())
}

fn remove_from_host(host: &mut Host, addr: &SocketAddr) {
    host.clients
        .iter()
        .position(|(a, _)| a == addr)
        .map(|i| _ = host.clients.swap_remove(i));
}

pub fn run_ping_thread(
    send_socket: crossbeam_channel::Sender<Packet>,
    recv_ping: Receiver<(SocketAddr, Option<PlayerUid>)>,
) {
    while let Ok(ping) = recv_ping.recv() {
        wait(1000);

        _ = send_socket.send(Packet::unreliable(
            ping.0,
            PacketMessage::Ping(ping.1).send().try_into().unwrap(),
        ))
    }
}
