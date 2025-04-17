use log::*;
use mio::net::UdpSocket;
use mio::{Events, Interest, Poll, Token};
use quiche::{Connection, ConnectionId};
use ring::rand::SystemRandom;
use simpleweb::quiche::{ClientIdMap, ClientMap};
use std::{collections::HashMap, net::SocketAddr, time::Instant};

const QUIC_TOKEN: Token = Token(usize::MAX - 1);
const MAX_DATAGRAM_SIZE: usize = 1350;
const MAX_BUF_SIZE: usize = 65507;

fn main() -> std::io::Result<()> {
    let mut buf = [0; MAX_BUF_SIZE];
    let mut out = [0; MAX_BUF_SIZE];
    let mut pacing = false;

    let addr: SocketAddr = "127.0.0.1:4433".parse().unwrap();
    let mut socket = UdpSocket::bind(addr)?;

    let mut poll = Poll::new()?;
    poll.registry()
        .register(&mut socket, QUIC_TOKEN, Interest::READABLE)?;

    let mut events = Events::with_capacity(128);
    let mut buf = [0u8; MAX_DATAGRAM_SIZE];
    let mut out = [0u8; MAX_DATAGRAM_SIZE];

    // Simplistic connection tracking
    let mut connections: HashMap<ConnectionId<'static>, (Connection, SocketAddr)> = HashMap::new();

    // QUIC config
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
    config.verify_peer(false);
    config.set_application_protos(&[b"\x05hq-29"]).unwrap();
    config.set_max_idle_timeout(5000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_streams_bidi(100);

    let rng = SystemRandom::new();
    let conn_id_seed =
        ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &rng).unwrap();

    let mut next_client_id = 0;
    let mut clients_ids = ClientIdMap::new();
    let mut clients = ClientMap::new();

    let mut continue_write = false;
    loop {
        // quiche server has timer check here, but implies they could do better.
        // let timeout = match continue_write {
        //     true => Some(std::time::Duration::from_secs(0)),

        //     false => clients.values().filter_map(|c| c.conn.timeout()).min(),
        // };

        poll.poll(&mut events, None)?;

        'read: loop {
            // If the event loop reported no events, it means that the timeout
            // has expired, so handle it without attempting to read packets. We
            // will then proceed with the send loop.
            if events.is_empty() && !continue_write {
                trace!("timed out");

                //clients.values_mut().for_each(|c| c.conn.on_timeout());

                break 'read;
            }

            for event in events.iter() {
                match event.token() {
                    QUIC_TOKEN => {
                        // loop until would bock.
                        loop {
                            match socket.recv_from(&mut buf) {
                                Ok((len, src)) => {
                                    let pkt_buf = &mut buf[..len];
                                    let hdr = match quiche::Header::from_slice(
                                        pkt_buf,
                                        quiche::MAX_CONN_ID_LEN,
                                    ) {
                                        Ok(v) => v,

                                        Err(e) => {
                                            error!("Parsing packet header failed: {:?}", e);
                                            continue 'read;
                                        }
                                    };

                                    let conn_id = ring::hmac::sign(&conn_id_seed, &hdr.dcid);
                                    let conn_id = &conn_id.as_ref()[..quiche::MAX_CONN_ID_LEN];
                                    let conn_id : ConnectionId<'static>= conn_id.to_vec().into();



                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                                Err(e) => return Err(e),
                            }
                        }
                    }
                    _ => {
                        warn!("unexpected event: {:?}", event);
                        continue;
                    }
                }
            }
        }
    }
}

// let hdr = match quiche::Header::from_slice(
//     &mut buf[..len],
//     quiche::MAX_CONN_ID_LEN,
// ) {
//     Ok(v) => v,
//     Err(_) => continue,
// };

// let conn_id = hdr.dcid.clone();
// let conn_entry = connections.entry(conn_id.clone()).or_insert_with(|| {
//     let scid = quiche::ConnectionId::from_ref(&hdr.dcid);
//     let mut conn =
//         quiche::accept(&scid, None, addr, src, &mut config).unwrap();
//     while let Ok((write, send_info)) = conn.send(&mut out) {
//         let _ = socket.send_to(&out[..write], src);
//     }

//     (conn, src)
// });

// let conn = &mut conn_entry.0;
// let recv_info = quiche::RecvInfo {
//     from: src,
//     to: addr,
// };
// let _ = conn.recv(&mut buf[..len], recv_info);

// // Handle application data
// while let Ok((stream_id, data)) = conn.stream_recv(0, &mut [0; 1024]) {
//     println!("Received stream {}: {} bytes", stream_id, data);
//     let _ = conn.stream_send(stream_id as u64, b"Hello from quiche!", true);
// }

// // Send outgoing packets
// while let Ok((write, send_info)) = conn.send(&mut out) {
//     let _ = socket.send_to(&out[..write], conn_entry.1);
// }
