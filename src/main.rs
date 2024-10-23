use dotenv::dotenv;
use std::env;
use tokio::net::TcpListener as AsyncTcpListener; // Assuming tokio is used for async TCP listener
use tokio_cron_scheduler::{JobScheduler, Job};
use std::time::Duration;
use tokio::time::sleep;
mod redis;
mod event_read;

#[tokio::main] // This starts the tokio runtime
async fn main() {
    dotenv().ok();  // Load environment variables from .env file

    // Bind the TCP listener to a local address (port from .env)
    let port_no: String = env::var("SERVICE_PORT").expect("PORT not found");
    let host: String = env::var("SERVICE_HOST").unwrap_or("127.0.0.1".to_string());
    let listener: AsyncTcpListener = AsyncTcpListener::bind(format!("{}:{}", host, port_no)).await.unwrap();

    println!("Server running at {}:{}",host, port_no);
    
    let sched = JobScheduler::new().await.unwrap();
    let _ = redis::redis_connection::init_redis_client();
    let _ = redis::redis_connection::connect_redis().await.unwrap();

    let job = Job::new_async("1/5 * * * * *", |_uuid, _l| {
        Box::pin(async {
            println!("");
            println!("Cron job triggered!");
            // Simulating some async work
            let _ = event_read::event::read_event().await;
            println!("Async work done!");
        })
    })
    .unwrap();

    sched.add(job).await.unwrap();

    // Start the scheduler
    sched.start().await.unwrap();

    // Main loop for accepting connections
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                // Handle the accepted connection here
                println!("Connection _stream: {:?}", stream);
                sleep(Duration::from_secs(60)).await;
            }
            Err(e) => {
                println!("Connection failed: {}", e);
            }
        }
    }
}

