#[macro_use] extern crate log;
#[macro_use] extern crate futures;
#[macro_use] extern crate serde_derive;
extern crate tokio_io;
extern crate tokio_uds;
extern crate bytes;
extern crate serde;

extern crate serde_json;
extern crate sozu_command_lib as sozu_command;

use bytes::{BufMut,BytesMut};
use std::iter::repeat;
use std::path::Path;
use std::io::{self, Error, ErrorKind};
use std::time::Duration;
use std::sync::{Arc,Mutex};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use futures::{Async, Poll, Sink, Stream, StartSend, Future, future};
use tokio_uds::UnixStream;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Decoder, Encoder, Framed};
use std::str::from_utf8;
use sozu_command::Order;
use sozu_command::data::{ConfigCommand,ConfigMessage,ConfigMessageAnswer,ConfigMessageStatus};

pub struct CommandCodec;

impl Decoder for CommandCodec {
    type Item  = ConfigMessageAnswer;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<ConfigMessageAnswer>, io::Error> {
        if let Some(pos) = (&buf[..]).iter().position(|&x| x == 0) {
            if let Ok(s) = from_utf8(&buf[..pos]) {
                match serde_json::from_str(s) {
                    Ok(message) => Ok(Some(message)),
                    Err(e) => {
                        Err(io::Error::new(io::ErrorKind::Other, format!("parse error: {:?}", e)))
                    }
                }
            } else {
                Err(io::Error::new(io::ErrorKind::InvalidData, String::from("could not parse UTF-8 data")))
            }
        } else {
            Ok(None)
        }
    }
}

impl Encoder for CommandCodec {
    type Item = ConfigMessage;
    type Error = io::Error;

    fn encode(&mut self, message: ConfigMessage, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match serde_json::to_string(&message) {//.map(|s| s.into_bytes()).unwrap_or(vec!()); 
        Ok(data) => {
            trace!("encoded message: {}", data);
            let buflen = buf.remaining_mut();
            if buflen < data.len() {
                buf.extend(repeat(0).take(data.len() - buflen));
            }
            let written = buf.put(&data[..]);
            trace!("written: {:?}", written);
            trace!("buffer content: {:?}", from_utf8(&buf[..]));
            Ok(()) },
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("serialization error: {:?}", e)))
        }
    }
}

pub struct SozuCommandTransport {
    upstream: Framed<UnixStream,CommandCodec>,
    received: HashMap<String,ConfigMessageAnswer>,
}

impl SozuCommandTransport {
    /*pub fn connect<P>(path: P, handle: u32) -> Result<SozuCommandTransport,io::Error>
        where P: AsRef<Path> {
            UnixStream::connect(path,  &handle).map(|stream| {
                SozuCommandTransport {
                    upstream: stream.framed(CommandCodec),
                }
            })
    }*/

    pub fn new(stream: UnixStream) -> SozuCommandTransport {
        SozuCommandTransport {
            upstream: stream.framed(CommandCodec),
            received: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct SozuCommandClient {
    transport: Arc<Mutex<SozuCommandTransport>>,
}

impl SozuCommandClient {
    pub fn new(stream: UnixStream) -> SozuCommandClient {
        SozuCommandClient {
            transport: Arc::new(Mutex::new(SozuCommandTransport::new(stream))),
        }
    }

    pub fn send(&mut self, message: ConfigMessage)  -> Box<Future<Item = ConfigMessageAnswer, Error = io::Error>> {
        trace!("will send message: {:?}", message);
        let tr = self.transport.clone();

        if let Ok(mut transport) = self.transport.lock() {
            trace!("lock");
            let id = message.id.clone();
            trace!("calling start_send");
            let res = transport.upstream.start_send(message).and_then(|_| transport.upstream.poll_complete());
            //trace!("start_send result: {:?}", res);

            //let res = transport.upstream.poll_complete();
            trace!("sent message, res {:?}", res);

            Box::new(future::poll_fn(move || {
                trace!("polling for id = {}", id);
                if let Ok(mut transport) = tr.try_lock() {
                    if let Some(message) = transport.received.remove(&id) {
                        if message.status == ConfigMessageStatus::Processing {
                            info!("processing: {:?}", message);
                        } else {
                            return Ok(Async::Ready(message))
                        }
                    }

                    let value = match transport.upstream.poll() {
                        Ok(Async::Ready(t)) => t,
                        Ok(Async::NotReady) => {
                            trace!("upstream poll gave NotReady");
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            trace!("upstream poll gave error: {:?}", e);
                            return Err(From::from(e));
                        },
                    };

                    if let Some(message) = value {
                        trace!("upstream poll gave message: {:?}", message);
                        if message.id != id {
                            transport.received.insert(message.id.clone(), message);
                            Ok(Async::NotReady)
                        } else {
                            if message.status == ConfigMessageStatus::Processing {
                                info!("processing: {:?}", message);
                                Ok(Async::NotReady)
                            } else {
                                Ok(Async::Ready(message))
                            }
                        }
                    } else {
                        trace!("upstream poll gave Ready(None)");
                        Ok(Async::NotReady)
                    }
                } else {
                    //FIXME: if we're there, it means the mutex failed
                    Err(
                        Error::new(ErrorKind::ConnectionAborted, format!("could not send message"))
                    )
                }

            }))
        
        } else {
            //FIXME: if we're there, it means the mutex failed
            Box::new(future::err(
                Error::new(ErrorKind::ConnectionAborted, format!("could not send message"))
            ))
        }
    }
}