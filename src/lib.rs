#[macro_use]
extern crate futures;
#[macro_use]
extern crate serde_derive;
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
use futures::{Async, Poll, Sink, Stream, StartSend, Future, future};
use tokio_uds::UnixStream;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Decoder, Encoder, Framed};
use std::str::from_utf8;
use sozu_command::Order;
use sozu_command::data::{ConfigCommand,ConfigMessage,ConfigMessageAnswer};  

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
            let buflen = buf.remaining_mut();
            if buflen < data.len() {
                buf.extend(repeat(0).take(data.len() - buflen));
            }
            buf.put(&data[..]);
            Ok(()) },
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("serialization error: {:?}", e)))
        }
    }
}

pub struct SozuCommandTransport {
  upstream: Framed<UnixStream,CommandCodec>,
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
        }
    }
}