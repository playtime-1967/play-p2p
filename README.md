Built a peer-to-peer networking playground with [rust-libp2p](https://github.com/libp2p/rust-libp2p).

**Samples**:   


1- **Chat application**  

A P2P chat application built using libp2p. It enables decentralized communication without a central server. It:   
uses Gossipsub (a pub-sub protocol) for message broadcasting.  
uses mDNS (multicast DNS) to discover peers on the local network automatically.  
supports TCP and QUIC (UDP-based) connections.  
listens on all available network interfaces and lets the OS choose a port.  
reads user input from stdin and broadcasts it to connected peers.  


**How to run**:  

Open two or more terminals.  

Terminal 1:  
`cargo run --bin chat`  

Terminal 2 (or more):  
Start another instance of the application.  
`cargo run --bin chat`  

Wait until you see peer discovery logs indicating that peers have been found.  
Once discovered, the terminals can exchange messages with each other.  

Hi!...  
Ola!...


```The system handles several types of logs/events, including:  
Local node is listening on ...  
mDNS discovered a new peer: ...  
Dialing { peer_id:...}  
IncomingConnection { connection_id:... }  
ConnectionClosed { peer_id:...}  
Received message {...}  
ConnectionClosed { peer_id:...}  ```
