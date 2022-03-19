pub struct Server {}

impl Server {
    pub fn new() -> Self { Server {} }
}

#[test]
fn test_server_new() { let s = Server::new(); }
