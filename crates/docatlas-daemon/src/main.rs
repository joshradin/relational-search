use tokio::net::TcpListener;

mod config;

use future_utils::FutureExt;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(("localhost", 5676))
        .map(|i| {

        });

    println!("opened listener {listener:#?}");
}

