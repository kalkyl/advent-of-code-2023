#![feature(async_fn_in_trait)]

use core::str::FromStr;
use std::error::Error;

use aoc_2023_icd::day2::{process, ClientToHost, HostToClient, StateMachine};
use aoc_2023_icd::{PID, VID};
use nusb::transfer::{Completion, RequestBuffer};
use nusb::Interface;
use tokio::fs;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let di = nusb::list_devices().unwrap().find(|d| d.vendor_id() == VID && d.product_id() == PID).expect("no device found");
    let device = di.open().expect("error opening device");
    let interface = device.claim_interface(0).expect("error claiming interface");

    let input = fs::read_to_string("../input/day2.txt").await?;

    let mut host = Host {
        bulk_in_ep: 0x81,
        bulk_out_ep: 0x01,
        interface,
        lines: input.lines(),
    };

    let result = process(&mut host).await.unwrap();
    println!("Result A: {:?}", result);
    Ok(())
}

struct Host<'a> {
    bulk_out_ep: u8,
    bulk_in_ep: u8,
    interface: Interface,
    lines: std::str::Lines<'a>,
}

impl Host<'_> {
    pub async fn send(&mut self, message: HostToClient) -> Result<(), ()> {
        let data = postcard::to_stdvec_cobs(&message).map_err(drop)?;
        let _ = self.interface.bulk_out(self.bulk_out_ep, data.into()).await;
        Ok(())
    }

    pub async fn receive(&mut self) -> Result<ClientToHost, ()> {
        if let Completion { data, status: Ok(()) } = self.interface.bulk_in(self.bulk_in_ep, RequestBuffer::new(64)).await {
            return postcard::from_bytes(&data).map_err(drop);
        }
        Err(())
    }
}

impl StateMachine for Host<'_> {
    async fn start(&mut self) -> Result<(), ()> {
        println!("START");
        self.send(HostToClient::Start).await?;
        match self.receive().await? {
            ClientToHost::Started => Ok(()),
            _ => Err(()),
        }
    }

    async fn write_next_game(&mut self) -> Result<(), ()> {
        match self.lines.next() {
            Some(s) => {
                self.send(HostToClient::GameData(heapless::String::from_str(s).unwrap())).await?;
                match self.receive().await? {
                    ClientToHost::GameDataWritten => Ok(()),
                    _ => Err(()),
                }
            }
            _ => {
                self.send(HostToClient::End).await?;
                Err(())
            }
        }
    }

    async fn end(&mut self) -> Result<(u32, u32), ()> {
        self.send(HostToClient::GetResult).await?;
        match self.receive().await? {
            ClientToHost::Result(res) => Ok(res),
            _ => Err(()),
        }
    }
}
