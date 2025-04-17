use simpleweb::linux::uring::web_hello;



pub fn main() {
    web_hello("127.0.0.1:8443".to_string()).expect("Failed to run web server with io_uring");
}