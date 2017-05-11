extern crate futures;
extern crate tokio_core;
extern crate sozu-command-futures as command;

use futures::future::Future;
use tokio_core::reactor::Core;
use sozu_command::config::Config;
use command::SozuCommandTransport;

fn main() {
  let mut core   = Core::new().unwrap();
  let handle     = core.handle();

  let stream = UnixStream::connect("/Users/geal/dev/rust/projects/yxorp/bin/sock",  &handle).unwrap();
  let transport = SozuCommandTransport::new(stream);
  core.run(
  ).unwrap()
}

