use mio::tcp::*;
use mio;
use mio::{Token, Handler, EventLoop, EventSet, PollOpt, TryRead, TryWrite, Evented};
use slab::Slab;
use std::io::{self, Cursor, Write, Read};
use std::net::{self, SocketAddr};
use std::time::Duration;
use std::mem;
use std::thread;
use std::sync::mpsc::{Receiver, Sender, channel};
use result::{ThrustResult, ThrustError};
use tangle::{Future, Async};
use bytes::buf::Buf;
use rand;
use std::collections::HashMap;

/// Communication into the Mio event loop happens with a `Message`. For each new Mio
/// event loop, a mio-specific `Sender<Message>` is returned.
pub enum Message {
    /// `Connect` establishes a new `TcpStream` with a specified remote. The
    /// `Sender` channel part is used to communicate back with the initiator on
    /// certain socket events.
    ///
    /// XXX: We should provide a way to accept a blocking `net::TcpStream` and convert
    /// it into a non-blocking mio `TcpStream`.
    Connect(SocketAddr, Sender<Dispatch>),
    /// To give a tighter feedback loop, a `Bind` message will accept a normal
    /// Rust blocking net::TcpListener. This allows the user to more easily handle
    /// binding errors before sending it into the event loop where you need to
    /// handle any errors asynchronously.
    Bind(net::TcpListener, SocketAddr, Sender<Dispatch>),
    /// Initiate an `Rpc` request. Each request needs to know which `Token` the respective
    /// `Connection` is associated with. The `Reactor` also knows nothing about Thrift
    /// and simply works at the binary level.
    Rpc(usize, Vec<u8>),
    /// Completely shutdown the `Reactor` and event loop. All current listeners
    /// and connections will be dropped.
    Shutdown
}

/// Communication from the `Reactor` to outside components happens with a `Dispatch` message
/// and normal Rust channels instead of Mio's variant.
pub enum Dispatch {
    /// Each connection and listener is tagged with a Mio `Token` so we can differentiate between
    /// them. As soon as we create the respective resource, we need a `Dispatch::Id` message
    /// containing the newly allocated `Token`.
    ///
    /// This is used to send RPC calls or further differentiate each resource outside the
    /// event loops.
    Id(usize),
    /// When a socket has been read, the `Reactor` will send the `Dispatch::Data` message
    /// to the associating channel.
    Data(Vec<u8>)
}

pub enum Timeout {
    Reconnect(Token, SocketAddr)
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Reading,
    Writing,
    Closed
}

pub struct Connection {
    stream: TcpStream,
    pub token: Token,
    state: State,
    chan: Sender<Dispatch>,
    rbuffer: Vec<u8>,
    wbuffer: Cursor<Vec<u8>>
}

impl Connection {
    pub fn new(stream: TcpStream, token: Token, chan: Sender<Dispatch>) -> Self {
        Connection {
            stream: stream,
            token: token,
            state: State::Reading,
            chan: chan,
            rbuffer: vec![],
            wbuffer: Cursor::new(vec![])
        }
    }

    pub fn ready(&mut self, event_loop: &mut EventLoop<Reactor>, events: EventSet) {
        match self.state {
            State::Reading if events.is_readable() => {
                self.readable();
                self.reregister(event_loop, self.token);
            },
            State::Writing if events.is_writable() => {
                self.writable();
                self.reregister(event_loop, self.token);
            },
            _ => {
                self.reregister(event_loop, self.token);
            }
        }
    }

    pub fn read(&mut self) -> ThrustResult<Vec<u8>> {
        match self.stream.try_read_buf(&mut self.rbuffer) {
            Ok(Some(_)) => Ok(mem::replace(&mut self.rbuffer, vec![])),
            Ok(None) => Err(ThrustError::NotReady),
            Err(err) => Err(ThrustError::Other)
        }
    }

    pub fn writable(&mut self) -> ThrustResult<()> {
        // Flush the whole buffer. The socket can, at any time, be unwritable. Thus, we
        // need to keep track of what we've written so far.
        while self.wbuffer.has_remaining() {
            self.flush();
        }

        self.state = State::Reading;

        Ok(())
    }

    pub fn readable(&mut self) -> ThrustResult<()> {
        while let Ok(buf) = self.read() {
            self.chan.send(Dispatch::Data(buf));
        }

        self.state = State::Writing;

        Ok(())
    }

    fn register(&mut self, event_loop: &mut EventLoop<Reactor>, token: Token) -> ThrustResult<()> {
        try!(event_loop.register(&self.stream, token, EventSet::readable(),
                            PollOpt::edge() | PollOpt::oneshot()));
        Ok(())
    }

    pub fn reregister(&self, event_loop: &mut EventLoop<Reactor>, token: Token) -> ThrustResult<()> {
        let event_set = match self.state {
            State::Reading => EventSet::readable(),
            State::Writing => EventSet::writable(),
            _ => EventSet::none()
        };

        try!(event_loop.reregister(&self.stream, self.token, event_set, PollOpt::oneshot()));
        Ok(())
    }
}

impl Write for Connection {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        try!(self.wbuffer.get_mut().write(data));
        try!(self.flush());
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.stream.try_write_buf(&mut self.wbuffer) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Ok(()),
            Err(err) => Err(err)
        }
    }
}

/// XXX: Generalize the listener. Allow multiple listeners
/// and multiple connections so we can multiplex a bunch
/// of Thrift services in one `EventLoop`.
pub struct Reactor {
    listeners: HashMap<Token, TcpListener>,
    connections: HashMap<Token, Connection>,
    sender: Sender<Dispatch>,
    servers: HashMap<Token, Sender<Dispatch>>,
    current_token: usize
}

impl Reactor {
    pub fn new(sender: Sender<Dispatch>) -> Self {
        Reactor {
            listeners: HashMap::new(),
            connections: HashMap::new(),
            sender: sender,
            servers: HashMap::new(),
            current_token: 0
        }
    }

    pub fn run(&mut self) -> ThrustResult<(EventLoop<Self>, mio::Sender<Message>)> {
        let mut event_loop = try!(EventLoop::new());

        // let mut buf = mem::replace(&mut self.buf, vec![]);
        // for stream in buf.into_iter() {
        //     let clone = self.sender.clone();
        //     let token = self.connections.insert_with(|token| {
        //         Connection::new(stream, token, clone)
        //     }).expect("Failed to insert a new connection in the slab");

        //     self.connections[token].register(&mut event_loop, token);
        // }

        let sender = event_loop.channel();
        Ok((event_loop, sender))
    }

    pub fn accept_connection(&mut self, event_loop: &mut EventLoop<Self>, token: Token) {
        let mut listener = self.listeners.get_mut(&token).expect("Listener was not found.");
        match listener.accept() {
            Ok(Some(socket)) => {
                let (stream, _) = socket;
                let clone = self.servers[&token].clone();
                let new_token = Token(self.current_token);
                let mut conn = Connection::new(stream, new_token, clone);

                self.connections.insert(new_token, conn);
                self.connections.get_mut(&new_token)
                    .unwrap()
                    .register(event_loop, new_token);

                self.current_token += 1;
            },
            _ => {}
        }
    }
}

impl Handler for Reactor {
    type Timeout = Timeout;
    type Message = Message;

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, events: EventSet) {
        if events.is_hup() {
            println!("Hup received. Socket disconnected.");
            if self.connections.contains_key(&token) {
                self.connections.remove(&token);
            }
            return;
        }

        if events.is_error() {
            println!("Err: {:?}", events);
            return;
        }

        if events.is_readable() && self.listeners.contains_key(&token) {
            self.accept_connection(event_loop, token);
            return;
        }

        if self.connections.contains_key(&token) {
            self.connections.get_mut(&token).expect("connection was not found.").ready(event_loop, events);
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, timeout: Timeout) {
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Message) {
        match msg {
            Message::Rpc(id, data) => {
                self.connections.get_mut(&Token(id)).expect("connection was not found.").write(&*data);
            },
            Message::Shutdown => {
                event_loop.shutdown();
            },
            Message::Connect(addr, tx) => {
                let mut mio_stream = TcpStream::connect(&addr).expect("MIO ERR");
                let new_token = Token(self.current_token);
                tx.send(Dispatch::Id(self.current_token));
                let mut conn = Connection::new(mio_stream, new_token, tx);

                self.connections.insert(new_token, conn);

                self.connections.get_mut(&new_token)
                    .unwrap()
                    .register(event_loop, new_token);

                self.current_token += 1;
            },
            Message::Bind(listener, addr, tx) => {
                let token = Token(self.current_token);
                let mut lis = TcpListener::from_listener(listener, &addr).unwrap();
                self.servers.insert(token, tx);

                event_loop.register(&lis, token, EventSet::readable(), PollOpt::edge()).unwrap();
                self.listeners.insert(token, lis);
                self.current_token += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use mio::EventLoop;
    use super::*;
    use std::io::Write;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use tangle::{Future, Async};
    use std::thread;
    use std::time::Duration;
    use std::net::{TcpListener, TcpStream, SocketAddr};

    #[test]
    fn create_reactor() {
        let (assert_tx, assert_rx) = channel();
        let (tx, rx) = channel();
        let mut reactor = Reactor::new(tx);
        let mut event_loop = EventLoop::new().unwrap();
        let sender = event_loop.channel();

        let handle = thread::spawn(move || {
            event_loop.run(&mut reactor);
        });

        // Establish a local TcpListener.
        let addr: SocketAddr = "127.0.0.1:5543".parse().unwrap();
        let listener = TcpListener::bind(addr.clone()).unwrap();
        let (s, r) = channel();

        sender.send(Message::Bind(listener, addr.clone(), s.clone()));

        // Connect to that socket.
        let (rpc_id_tx, rpc_id_rx) = channel();
        sender.send(Message::Connect(addr, rpc_id_tx));
        let id = match rpc_id_rx.recv().unwrap() {
            Dispatch::Id(n) => n,
            _ => panic!("Expected to receive the Connection id/token.")
        };
        sender.send(Message::Rpc(id, b"abc".to_vec()));

        let server = thread::spawn(move || {
            for msg in r.iter() {
                match msg {
                    Dispatch::Data(msg) => {
                        assert_tx.send(msg).expect("Could not assert_tx");
                    },
                    _ => {}
                }
            }
        });

        let v = assert_rx.recv().expect("Error trying to assert reactor test.");
        assert_eq!(v.len(), 3);
        assert_eq!(v, b"abc");
    }
}
