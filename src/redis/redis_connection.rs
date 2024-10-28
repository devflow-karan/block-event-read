use dotenv::dotenv;
use std::env;
extern crate redis;
use chrono::{Utc, DateTime};
use chrono_tz::Tz;
use tokio::sync::OnceCell;
use std::sync::Arc;
use redis::Client;

// Define a global OnceCell for lazy initialization
static REDIS_CLIENT: OnceCell<Arc<Client>> = OnceCell::const_new();

pub fn init_redis_client() -> Arc<Client> {
    dotenv().ok();
    // redis://<user>:<password>@<endpoint>:<port>
    let redis_host: String = env::var("REDIS_CONNECTION").expect("REDIS CONNECTION not found");
    let client: Client = Client::open(redis_host).expect("Failed to create Redis client");
    let client = Arc::new(client);
    REDIS_CLIENT.set(client.clone()).expect("Failed to initialize Redis client");
    client
}

async fn get_redis_client() -> Arc<Client> {
    // Get the global REDIS_CLIENT or initialize it if it's not set
    REDIS_CLIENT
        .get()
        .cloned()
        .unwrap_or_else(init_redis_client)
}

pub async fn connect_redis() -> redis::RedisResult<()> {
    let current_time_str = get_time().unwrap();
    println!("Current Time: {}", current_time_str);
    // Set a value in Redis
    set_value("time", &current_time_str).await.unwrap();

    // Do something here
    Ok(())
}

pub async fn set_value(key: &str, value: &str) -> redis::RedisResult<()> {
    let client: Arc<Client> = get_redis_client().await;

    // Get a connection from the client
    let mut con = client.get_connection()?;
    redis::cmd("SET").arg(key).arg(value).query::<()>(&mut con)?; // Explicitly specify the unit type `()`

    Ok(())
}

pub async fn get_value(key: &str) -> redis::RedisResult<Option<String>> {
    let client: Arc<Client> = get_redis_client().await;

    // Get a connection from the client
    let mut con = client.get_connection()?;

    // Get the value from Redis
    let cache_value: Option<String> = redis::cmd("GET").arg(key).query(&mut con).ok();

    // Print the retrieved value or indicate a miss
    match &cache_value {
        Some(value) => println!("cache_value: {:?}", value),
        None => println!("No value found for key: {:?}", key),
    }

    Ok(cache_value)
}


fn get_time() -> Result<String, String> {
    // Define the timezone you want to use
    let tz: Tz = "Asia/Kolkata".parse().unwrap();

    // Get the current time in UTC
    let current_time_utc: DateTime<Utc> = Utc::now();

    // Convert UTC time to the desired timezone
    let current_time_local = current_time_utc.with_timezone(&tz);

    // Format the time to "YYYY-MM-DD HH:MM:SS"
    let current_time_str: String = current_time_local.format("%Y-%m-%d %H:%M:%S").to_string();

    Ok(current_time_str)
}