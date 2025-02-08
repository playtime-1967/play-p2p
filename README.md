Built a peer-to-peer networking playground with [rust-libp2p](https://github.com/libp2p/rust-libp2p).

**Samples**:   


1- **Chat application**  

A P2P chat application built using libp2p. It enables decentralized communication without a central server. It:   
Uses Gossipsub (a pub-sub protocol) for message broadcasting.  
Uses mDNS (multicast DNS) to discover peers on the local network automatically.  
Supports TCP and QUIC (UDP-based) connections.  
Listens on all available network interfaces and lets the OS choose a port.  
Reads user input from stdin and broadcasts it to connected peers.  


**How to run**:  

Open two or more terminals and run:  

`cargo run --bin chat`  

Wait until you see peer discovery logs indicating that peers have been found.  
Once discovered, the terminals can exchange messages with each other.

```
Terminal 1, type Hi!...  
Terminal 2, type Ola!...
```
The application handles several types of events, including:   
``` 
Local node is listening on ...  
mDNS discovered a new peer: ...  
Dialing { peer_id:...}  
IncomingConnection { connection_id:... }  
ConnectionClosed { peer_id:...}  
Received message {...}  
ConnectionClosed { peer_id:...} 
```

------------------------------------------------------------------------------
2- **Distributed key-value store** 

A distributed key-value store using libp2p’s Kademlia DHT. It:  
Allows nodes to store and retrieve values across a peer-to-peer network.  
Listens for new peers and updates routing.  
Responds to key-value queries.  
Finds peers that have stored the key.  


**How to run**:  

Open two or more terminals:  

`cargo run --bin key-val-store`   

``` 
Terminal 1, type PUT my-key my-value  
Terminal 2/3, type GET my-key  
Terminal 2, type PUT_PROVIDER my-key  
Terminal 1/3, type GET_PROVIDERS my-key  
``` 

------------------------------------------------------------------------------
3- **IPFS PubSub Peer** 

An app that acts as an IPFS peer, enabling messaging with other peers in a private network or the IPFS public network. It:  
Sets up a private network (if a swarm.key exists), allowing only authorized peers to join.  
Uses Gossipsub to broadcast messages and listens for incoming messages.  
Uses ping protocol to measure round-trip time between peers.  

**How to run**: 
To run the app, install the IPFS daemon CLI (Kubo) from [here](https://docs.ipfs.tech/install/command-line/#install-official-binary-distributions).  

After installing the IPFS daemon CLI, follow these steps to run the app:  
1- Run IPFS daemon in a terminal:  

`ipfs daemon --enable-pubsub-experiment`  

![ipfs daemon terminal](https://github.com/playtime-1967/play-p2p/raw/ipfs-daemon.jpg)

2- Run IPFS subscriber in a terminal (where "play-ipfs" is the topic name set as an environment variable in the app):  

`ipfs pubsub sub play-ipfs`  

3- Run the app as both a subscriber and publisher in a terminal (Locate the IP and IPFS_PeerId in the IPFS daemon's terminal):  

`cargo run --bin ipfs-pubsub {IP}/p2p/{IPFS_PeerId}` 

For example:  

`cargo run --bin ipfs-pubsub /ip4/127.0.0.1/tcp/4001/p2p/12D3KooWHT8vxeYkGSYuNfpa9JnR5jSPwSYTXgYXuFWqEEZ7XTR3`  

4- Run the IPFS Publisher in a terminal (the data to be published is sent in the HTTP request body as multipart/form-data):  

`ipfs pubsub pub play-ipfs {file_path}`  

Now we have two publishers and two subscribers. The app publishes plain text, while the IPFS Publisher publishes files as mentioned above.  

The application handles several types of events, including:   

``` 
ping: rtt to ...  
ConnectionEstablished ...  
NewExternalAddrOfPeer { peer_id: ... }  
Subscribed { peer_id: ...}  
Unsubscribed { peer_id: ...}  
Received message ...   
```