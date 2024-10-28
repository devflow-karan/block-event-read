use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use web3::contract::Contract;
use web3::ethabi::{LogParam, RawLog, Token};
use web3::transports::Http;
use web3::types::{Address,};
use web3::types::{FilterBuilder, Log};
use web3::Web3;
use web3::types::U64;
use crate::redis::redis_connection;

// Define a struct to hold both decoded log and event name
struct DecodedEvent {
    event_name: String,
    decoded_log: Vec<LogParam>,
}

#[allow(dead_code)]
#[derive(Debug)]
enum ValueType {
    Address(String),
    Uint(u64),
    Int(i64),
    Bool(bool),
    String(String),
    Unsupported,
}

pub async fn read_event() -> web3::Result<()> {
    dotenv().ok();
    let block_key_name: String =  env::var("BLOCK_READ_NAME").expect("LAST BLOCK not found");
    let node_url: String = env::var("NODE_URL").expect("NODE URL not found");
    let contract_address_str: String =  env::var("CONTRACT_ADDRESS").expect("CONTRACT ADDRESS not found");

    let redis_val = redis_connection::get_value(&block_key_name).await.unwrap();
    println!("Latest redis_val redis_val: {:?}", redis_val);

    let default_block: String = env::var("STARTING_BLOCK").expect("STARTING BLOCK not found");
    // Check if redis_val is None and use default_block if it is
    let starting_block_str = match redis_val {
        Some(value) => value,          // Use the value from Redis if it exists
        None => default_block,    // Use the starting block from environment if Redis value is None
    };

    let starting_block: u64 = starting_block_str.parse().expect("Invalid starting block");

    let block_range_str: String = env::var("BLOCK_RANGE").expect("BLOCK RANGE not found"); // New env variable for block range
    let block_range: u64 = block_range_str.parse().expect("Invalid block range"); // Block range to add

    let transport: Http = Http::new(&node_url).expect("Failed to create HTTP transport");
    let web3: Web3<Http> = Web3::new(transport);

    let from_block: u64 = starting_block; // Starting block number
    let to_block: u64 = starting_block + block_range; // Add the block range from env
    let block_number:U64 = web3.eth().block_number().await?;
 
    // Check if `to_block` is greater than `block_number` and adjust if necessary
    let final_to_block: u64 = if to_block > block_number.as_u64() {
        block_number.as_u64() // Set to_block to block_number if it's greater
    } else {
        to_block // Keep the original to_block
    };

    let next_block_read = final_to_block + 1;
    let next_block_read_str: String = convert_to_string(next_block_read);

    println!("Latest Block Number: {:?}", block_number);
    println!("Block From {:?}", from_block);
    println!("Block To {:?}", to_block);
    redis_connection::set_value(&block_key_name, &next_block_read_str).await.unwrap();

    let contract_address: Address = contract_address_str
        .parse()
        .expect("Invalid contract address");
    let contract_abi = include_bytes!("abi.json");

    let contract: Contract<Http> = Contract::from_json(web3.eth(), contract_address, contract_abi)
        .expect("Failed to instantiate contract");

    let filter: web3::types::Filter = FilterBuilder::default()
        .address(vec![contract_address])
        .from_block(from_block.into())
        .to_block(to_block.into())
        .build();

    let logs: Vec<Log> = web3.eth().logs(filter).await.expect("Logs Not Available");

    let mut index: i32 = 0;
    for log in logs {
        process_event(log, &contract);
        index += 1;
    }

    if index < 1 {
        println!("No Event Available for Now");
    }
    Ok(())
}

fn process_event(log: Log, contract: &Contract<Http>) {
    let block_number: Option<u64> = log.block_number.map(|bn| bn.as_u64());
    if let Some(decoded_event) = decode_event_name(log, contract) {
        println!("Block Number: {}", block_number.unwrap_or(0));
        println!("Event Name: {}", decoded_event.event_name);

        let block_number: u64 = block_number.unwrap_or(0);
        let event_name: String = decoded_event.event_name;
        let decoded_log: Vec<LogParam> = decoded_event.decoded_log;
        let mut json_map: HashMap<String, ValueType> = HashMap::new();

        for decode in decoded_log {
            // Value to be treated as a key
            let key_value: String = decode.name.clone();
            let value_as_string: ValueType = token_to_value(&decode.value);
            json_map.insert(key_value, value_as_string);
        }
        json_map.insert("block_number".to_string(), ValueType::Uint(block_number));
        json_map.insert("event_name".to_string(), ValueType::String(event_name));

        println!("Data json_map: {:?}", json_map);
    } else {
        println!("No matching event found or failed to decode.");
    }
}

fn decode_event_name(log: Log, contract: &Contract<Http>) -> Option<DecodedEvent> {
    if !log.topics.is_empty() {
        // The first topic contains the event signature (the hash of the event name and its arguments)
        let event_signature = log.topics[0];

        // Match the event signature against the contract's ABI to determine the event name
        for event in contract.abi().events() {
            let raw_log = RawLog {
                topics: log.topics.clone(),
                data: log.data.0.clone(),
            };
            if let Ok(decoded_log) = event.parse_log(raw_log) {
                // Check if the event matches the expected signature
                if event.signature() == event_signature {
                    // Return both decoded log and event name as a custom object
                    return Some(DecodedEvent {
                        event_name: event.name.clone(),
                        decoded_log: decoded_log.params,
                    });
                }
            }
        }
    }
    None
}

// fn token_to_string(token: &Token) -> String {
//     match token {
//         Token::Address(addr) => format!("{:?}", addr),
//         Token::Uint(value) => format!("{}", value),
//         Token::Int(value) => format!("{}", value),
//         Token::Bool(b) => format!("{}", b),
//         Token::String(s) => s.clone(),
//         _ => "Unsupported token type".to_string(), // Handle other cases as needed
//     }
// }

fn token_to_value(token: &Token) -> ValueType {
    match token {
        Token::Address(addr) => ValueType::Address(format!("{:?}", addr)),
        Token::Uint(value) => ValueType::Uint(value.low_u64()), // Adjust for large Uints if needed
        Token::Int(value) => ValueType::Int(value.low_u64() as i64),
        Token::Bool(b) => ValueType::Bool(*b),
        Token::String(s) => ValueType::String(s.clone()),
        _ => ValueType::Unsupported,
    }
}

fn convert_to_string<T: ToString>(value: T) -> String {
    value.to_string()
}
