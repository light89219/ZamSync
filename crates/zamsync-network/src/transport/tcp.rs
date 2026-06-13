use crate::protocol;
use std::collections::HashMap;
use std::io::BufWriter;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use zamsync_core::ports::Transport;
use zamsync_core::{NodeId, SyncMessage, ZamError, ZamResult};

pub struct TcpTransport {
    listener: TcpListener,
    peers: HashMap<u32, TcpStream>,
}

impl TcpTransport {
    pub fn bind(addr: &str) -> ZamResult<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        log::info!("listening on {}", addr);
        Ok(Self {
            listener,
            peers: HashMap::new(),
        })
    }

    pub fn connect(&mut self, peer_id: NodeId, addr: &str) -> ZamResult<()> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_millis(10)))?;
        self.peers.insert(peer_id.0, stream);
        log::info!("connected to peer {} at {}", peer_id.0, addr);
        Ok(())
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, peer_id: NodeId, message: &SyncMessage) -> ZamResult<()> {
        let stream = self
            .peers
            .get_mut(&peer_id.0)
            .ok_or_else(|| ZamError::Protocol(format!("no connection to peer {}", peer_id.0)))?;
        let mut writer = BufWriter::new(stream as &TcpStream);
        protocol::encode(message, &mut writer)
    }

    fn receive(&mut self) -> ZamResult<Option<(NodeId, SyncMessage)>> {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                log::debug!("incoming connection from {}", addr);
                stream.set_read_timeout(Some(Duration::from_millis(10)))?;
                drop(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.into()),
        }

        let peer_ids: Vec<u32> = self.peers.keys().cloned().collect();
        for peer_id_raw in peer_ids {
            if let Some(stream) = self.peers.get_mut(&peer_id_raw) {
                match protocol::decode(stream) {
                    Ok(msg) => return Ok(Some((NodeId(peer_id_raw), msg))),
                    Err(ZamError::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(None)
    }
}
