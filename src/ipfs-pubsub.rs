use dotenv::dotenv;
use either::Either;
use futures::prelude::*;
use libp2p::{
    core::transport::upgrade::Version,
    gossipsub, identify, noise, ping,
    pnet::{PnetConfig, PreSharedKey},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, Transport,
};
use std::{env, error::Error, str::FromStr};
use tokio::{io, io::AsyncBufReadExt, select, time::Duration};
mod utils;

//combines gossipsub, ping and identify.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    //a PSK(PreSharedKey) or swarm.key secures private libp2p networks, allowing only nodes with the same PSK to join and communicate.
    let pre_shared_key: Option<PreSharedKey> = utils::get_pre_shared_key()?
        .map(|text| PreSharedKey::from_str(&text))
        .transpose()?;

    if let Some(pre_shared_key) = pre_shared_key {
        println!(
            "using swarm key with fingerprint: {}",
            pre_shared_key.fingerprint()
        );
    }

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_other_transport(|key| {
            let noise_config = noise::Config::new(key).unwrap();
            let yamux_config = yamux::Config::default();

            let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
            let maybe_encrypted = match pre_shared_key {
                Some(pre_shared_key) => {
                    //a private netowork using the PreSharedKey.
                    Either::Left(base_transport.and_then(move |socket, _| {
                        PnetConfig::new(pre_shared_key).handshake(socket)
                    }))
                }
                //IPFS public network.
                None => Either::Right(base_transport),
            };

            maybe_encrypted
                .upgrade(Version::V1Lazy) //ensures compatibility with lazy connections
                .authenticate(noise_config)
                .multiplex(yamux_config)
        })?
        .with_dns()?
        .with_behaviour(|key| {
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .max_transmit_size(262144)
                .build()
                .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?;
            Ok(MyBehaviour {
                gossipsub: gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?,
                //identify protocol exchanges information/metadata to verify the other peer's identity
                identify: identify::Behaviour::new(identify::Config::new(
                    "/ipfs/0.1.0".into(),
                    key.public(),
                )),
                ping: ping::Behaviour::new(ping::Config::new()),
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
        .build();

    let topic_name = env::var("IPFS_TOPIC").unwrap_or("play-ipfs".to_string());
    let gossipsub_topic = gossipsub::IdentTopic::new(topic_name);

    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&gossipsub_topic)
        .unwrap();
    println!("Subscribing to {:#?}", gossipsub_topic);

    // dialling other nodes if specified
    for to_dial in std::env::args().skip(1) {
        let addr: Multiaddr = utils::parse_legacy_multiaddr(&to_dial)?;
        swarm.dial(addr)?;
        println!("Dialed {to_dial:?}")
    }

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut stdin = io::BufReader::new(io::stdin()).lines();

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
                                result: Result::Err(ping::Failure:: Other { error }),
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
