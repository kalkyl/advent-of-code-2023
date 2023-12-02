use core::str::FromStr;
use std::error::Error;

use aoc_2023_icd::day1::{ClientToHost, HostToClient};
use aoc_2023_icd::{PID, VID};
use nusb::transfer::{Completion, RequestBuffer};
use nusb::Interface;
use tokio::fs;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let di = nusb::list_devices().unwrap().find(|d| d.vendor_id() == VID && d.product_id() == PID).expect("no device found");
    let device = di.open().expect("error opening device");
    let interface = device.claim_interface(0).expect("error claiming interface");

    let mut usb = UsbComm {
        bulk_in_ep: 0x81,
        bulk_out_ep: 0x01,
        interface,
    };

    usb.send(HostToClient::Reset).await.unwrap();

    let input = fs::read_to_string("../input/day1.txt").await?;
    for line in input.lines() {
        usb.send(HostToClient::Data(heapless::String::from_str(line).unwrap())).await.unwrap();
    }

    usb.send(HostToClient::GetResultA).await.unwrap();
    let ClientToHost::Result(result_a) = usb.receive().await.unwrap();
    println!("Result A: {result_a}");

    usb.send(HostToClient::GetResultB).await.unwrap();
    let ClientToHost::Result(result_b) = usb.receive().await.unwrap();
    println!("Result B: {result_b}");

    Ok(())
}

#[derive(Debug)]
enum CommError {
    IO,
    Postcard(postcard::Error),
}

struct UsbComm {
    bulk_out_ep: u8,
    bulk_in_ep: u8,
    interface: Interface,
}

impl UsbComm {
    pub async fn send(&mut self, message: HostToClient) -> Result<(), CommError> {
        let data = postcard::to_stdvec(&message).map_err(CommError::Postcard)?;
        let _ = self.interface.bulk_out(self.bulk_out_ep, data.into()).await;
        Ok(())
    }

    pub async fn receive(&mut self) -> Result<ClientToHost, CommError> {
        if let Completion { data, status: Ok(()) } = self.interface.bulk_in(self.bulk_in_ep, RequestBuffer::new(64)).await {
            return postcard::from_bytes(&data).map_err(CommError::Postcard);
        }
        Err(CommError::IO)
    }
}
