#![allow(warnings)]

use dotenv::dotenv;
use either::Either;
use futures::prelude::*;
use libp2p::{
    core::transport::upgrade::Version,
    gossipsub, identify,
    multiaddr::Protocol,
    noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, Transport,
};
use std::{env, error::Error, fs, path::Path, str::FromStr};
use tokio::{io, io::AsyncBufReadExt, select, time::Duration};
use tracing_subscriber::EnvFilter;

// We create a custom network behaviour that combines gossipsub, ping and identify.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let ipfs_path = get_ipfs_path();
    println!("using IPFS_PATH {ipfs_path:?}");

    let pre_shared_key: Option<PreSharedKey> = get_pre_shared_key(&ipfs_path)?
        .map(|text| PreSharedKey::from_str(&text))
        .transpose()?;

    if let Some(pre_shared_key) = pre_shared_key {
        println!(
            "using swarm key with fingerprint: {}",
            pre_shared_key.fingerprint()
        );
    }

    // Create a Gosspipsub topic
    let gossipsub_topic = gossipsub::IdentTopic::new("psgs");

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_other_transport(|key| {
            let noise_config = noise::Config::new(key).unwrap();
            let yamux_config = yamux::Config::default();

            let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));

            //A PreSharedKey (PSK) or swarm.key: is a mechanism used to secure private networks in libp2p. It ensures that only nodes with the same PSK can join and communicate in the network.
            let maybe_encrypted = match pre_shared_key {
                Some(pre_shared_key) => {
                    //Represents an encrypted transport using the PreSharedKey.
                    Either::Left(base_transport.and_then(move |socket, _| {
                        PnetConfig::new(pre_shared_key).handshake(socket)
                    }))
                }
                // /Represents an unencrypted transport.
                None => Either::Right(base_transport),
            };

            maybe_encrypted
                .upgrade(Version::V1Lazy) //ensures compatibility with lazy connections
                .authenticate(noise_config) //secures communication(cryptographic handshake).
                .multiplex(yamux_config)
        })?
        .with_dns()?
        .with_behaviour(|key| {
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .max_transmit_size(262144)
                .build()
                .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.
            Ok(MyBehaviour {
                gossipsub: gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("Valid configuration"),
                //identify protocol exchanges information/metadata to verify the other peer's identity
                identify: identify::Behaviour::new(identify::Config::new(
                    "/ipfs/0.1.0".into(),
                    key.public(),
                )),
                ping: ping::Behaviour::new(ping::Config::new()),
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX))) //keep connections open indefinitely (u64::MAX) when idle
        .build();

    println!("Subscribing to {gossipsub_topic:?}");
    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&gossipsub_topic)
        .unwrap();

    // Reach out to other nodes if specified
    for to_dial in std::env::args().skip(1) {
        let addr: Multiaddr = parse_legacy_multiaddr(&to_dial)?;
        swarm.dial(addr)?;
        println!("Dialed {to_dial:?}")
    }

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Kick it off
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(gossipsub_topic.clone(), line.as_bytes())
                {
                    println!("Publish error: {e:?}");
                }
            },
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {address:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                        println!("identify: {event:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::GossipsubNotSupported {
                        peer_id,
                    })) => {
                        println!("peer_id: {} does not support Gossipsub protocol", peer_id);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    })) => {
                        println!(
                            "Received message: {} with id: {} from peer: {:?}",
                            String::from_utf8_lossy(&message.data),
                            id,
                            peer_id
                        )
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Ping(event)) => {
                        match event {
                            ping::Event {
                                peer,
                                result: Result::Ok(rtt),
                                ..
                            } => {
                                println!(
                                    "ping: rtt to {} is {} ms",
                                    peer.to_base58(),
                                    rtt.as_millis()
                                );
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Timeout),
                                ..
                            } => {
                                println!("ping: timeout to {}", peer.to_base58());
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Unsupported),
                                ..
                            } => {
                                println!("ping: {} does not support ping protocol", peer.to_base58());
                            }
                            ping::Event {
                                peer,
                                result: Result::Err(ping::Failure::Other { error }),
                                ..
                            } => {
                                println!("ping: ping::Failure with {}: {error}", peer.to_base58());
                            }
                        }
                    }
                    connection_event => println!("{connection_event:?}"),
                }
            }
        }
    }
}

/// Get the current ipfs repo path, either from the IPFS_PATH environment variable or hard-coded
fn get_ipfs_path() -> Box<Path> {
    env::var("IPFS_PATH")
        .map(|ipfs_path| Path::new(&ipfs_path).into())
        .unwrap_or_else(|_| Path::new("C:\\Users\\SamanNamnik\\.ipfs").into())
}

/// Read the pre shared key file from the given ipfs directory
fn get_pre_shared_key(path: &Path) -> std::io::Result<Option<String>> {
    let swarm_key_file = path.join("swarm.key");
    match fs::read_to_string(swarm_key_file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// modifies a Multiaddr by removing the /p2p/peer_id part. libp2p's dial function does not directly support dialing an address with a peer_id attached.
/// The peer_id is managed separately by the Swarm layer.
fn strip_peer_id(addr: &mut Multiaddr) {
    let last = addr.pop();
    println!("last {:?}", last);
    match last {
        Some(Protocol::P2p(peer_id)) => {
            let mut addr = Multiaddr::empty();
            addr.push(Protocol::P2p(peer_id));
            println!("removing peer id {addr} so this address can be dialed by rust-libp2p");
        }

        Some(other) => {
            println!("other {:?}", other);
            addr.push(other)
        }
        _ => {}
    }
}

/// parse a legacy multiaddr (replace ipfs with p2p), and strip the peer id so it can be dialed by rust-libp2p
fn parse_legacy_multiaddr(text: &str) -> Result<Multiaddr, Box<dyn Error>> {
    let sanitized = text
        .split('/')
        .map(|part| if part == "ipfs" { "p2p" } else { part })
        .collect::<Vec<_>>()
        .join("/");
    let mut res = Multiaddr::from_str(&sanitized)?;
    strip_peer_id(&mut res);
    Ok(res)
}

//How to run:
// 1- Run IPFS Daemon in one terminal:
//ipfs daemon --enable-pubsub-experiment
//2- Run your terminal with one of the addresses that IPFS is listening
//cargo run --bin psgs /ip4/127.0.0.1/tcp/4001/p2p/12D3KooWS3CapWihK8GyX9A1CC28NzSGDinbEwrJxXLzXZHiSm2z

//3- to publish/subscribe messages from gossip
//Run another terminal
//hi
//ipfs pubsub pub psgs {file_path}
