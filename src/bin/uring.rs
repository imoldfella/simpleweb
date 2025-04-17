use simpleweb::linux::uring::web_hello;



pub fn main() {
    web_hello().expect("Failed to run web server with io_uring");
}