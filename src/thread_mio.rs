impl Server {
    // spawn a task for each connection; this task will start a new task for each stream (if it's a websocket or webtransport)
    fn run_mio(&self, thread: usize) -> std::io::Result<()> {
        use socket2::{Domain, Socket, Type};

        let addr: SocketAddr = self.config.host.parse().unwrap();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        //socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.listen(128)?;
        let mut listener = TcpListener::from_std(socket.into());

        let mut poll = Poll::new()?;
        poll.registry()
            .register(&mut listener, SERVER_TOKEN, Interest::READABLE)?;

        let mut events = Events::with_capacity(2048);
        let mut clients: Slab<TlsClient> = Slab::with_capacity(1024);
        let mut next_token = Token(SERVER_TOKEN.0 + 1);

        println!("TLS server listening on https://{}", addr);

        loop {
            match poll.poll(&mut events, None) {
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }

            for event in events.iter() {
                println!("Got event: {:?}", event);
                match event.token() {
                    SERVER_TOKEN => match listener.accept() {
                        Ok((mut stream, addr)) => {
                            println!("Accepted connection from {}", addr);
                            let entry = clients.vacant_entry();
                            let token = Token(entry.key());
                            poll.registry().register(
                                &mut stream,
                                token,
                                Interest::READABLE.add(Interest::WRITABLE),
                            )?;
                            let client = TlsClient::new(stream, self.tls_config.clone());
                            entry.insert(client);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    },
                    token => {
                        println!("Token {}", token.0);
                        if let Some(mut client) = clients.get_mut(token.0) {
                            let keep = match client.ready() {
                                Ok(keep) => keep,
                                Err(_) => false,
                            };
                            if keep {
                                let mut interest = Interest::READABLE;
                                if client.conn.wants_write() {
                                    interest = interest.add(Interest::WRITABLE);
                                }

                                poll.registry()
                                    .reregister(&mut client.socket, token, interest)?;
                            }
                        }
                    }
                }
            }
        }
    }
}
