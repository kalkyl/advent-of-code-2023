use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;

use aoc_2023_host::rpc;
use aoc_2023_icd::day3::{Engine, EngineReq, Number, WireError};
use aoc_2023_icd::{PID, VID};
use heapless::String;
use postcard_rpc::host_client::HostClient;
use tokio::fs;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let di = nusb::list_devices().unwrap().find(|d| d.vendor_id() == VID && d.product_id() == PID).expect("no device found");
    let device = di.open().expect("error opening device");
    let client: HostClient<WireError> = rpc::new_client(device, "error", 8);

    let _ = client.send_resp::<Engine>(&EngineReq::Reset).await.unwrap();
    let mut parts: HashSet<Number> = HashSet::new();
    let input = fs::read_to_string("../input/day3.txt").await?;
    for line in input.lines() {
        match client.send_resp::<Engine>(&EngineReq::Data(String::from_str(line).unwrap())).await {
            Ok(resp) => parts.extend(resp.result.into_iter()),
            _ => {
                println!("Something went wrong");
                break;
            }
        }
    }
    let sum_a = parts.iter().map(|p| p.value as u32).sum::<u32>();
    println!("Result A: {}", sum_a);

    Ok(())
}
