use std::collections::HashSet;
use std::error::Error;
use std::str::FromStr;
use heapless::String;
use aoc_2023_icd::day3::{WireError, Engine, EngineReq, Number};
use aoc_2023_icd::{PID, VID};
use nusb::transfer::{Completion, RequestBuffer};
use nusb::Interface;
use postcard_rpc::host_client::HostClient;
use tokio::fs;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let di = nusb::list_devices().unwrap().find(|d| d.vendor_id() == VID && d.product_id() == PID).expect("no device found");
    let device = di.open().expect("error opening device");
    let interface = device.claim_interface(0).expect("error claiming interface");
    let port = UsbComm::new(interface);

    let (client, wire) = HostClient::<WireError>::new_manual("error", 8);
    tokio::task::spawn(async move { rpc::wire_worker(port, wire).await });

    let _ = client.send_resp::<Engine>(&EngineReq::Reset).await.unwrap();
    let mut parts: HashSet<Number> = HashSet::new();
    let input = fs::read_to_string("../input/day3.txt").await?;
    for line in input.lines() {
        match client.send_resp::<Engine>(&EngineReq::Data(String::from_str(line).unwrap())).await {

            Ok(resp) => 
                parts.extend(resp.result.into_iter()),
                _ => {
                    println!("Error");
                    break;
                }
        }
    }
    let sum_a = parts.iter().map(|p| p.value as u32).sum::<u32>();
    println!("Result A: {}", sum_a);

    Ok(())
}

mod usb {
    use super::*;
    
    pub struct UsbComm {
        bulk_out_ep: u8,
        bulk_in_ep: u8,
        interface: Interface,
    }

    impl UsbComm {
        pub fn new(interface: Interface) -> Self {
            Self {
                bulk_in_ep: 0x81,
                bulk_out_ep: 0x01,
                interface,
            }
        }
    
        pub async fn write(&mut self, data: &[u8]) -> Result<(), ()> {
            let _ = self.interface.bulk_out(self.bulk_out_ep, data.into()).await;
            Ok(())
        }
    
        pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
            if let Completion { data, status: Ok(()) } = self.interface.bulk_in(self.bulk_in_ep, RequestBuffer::new(4096)).await {
                buf[..data.len()].copy_from_slice(&data);
                return Ok(data.len());
            }
            Err(())
        }
    }
}

mod rpc {

    use std::collections::HashMap;

    use cobs::encode_vec;
    use postcard_rpc::accumulator::raw::*;
    use postcard_rpc::headered::extract_header_from_bytes;
    use postcard_rpc::host_client::*;
    use postcard_rpc::Key;
    use tokio::select;
    use tokio::sync::mpsc::Sender;
    use super::usb::UsbComm;

    pub async fn wire_worker(mut port: UsbComm, ctx: WireContext) {
        let mut buf = [0u8; 1024];
        let mut acc = CobsAccumulator::<1024>::new();
        let mut subs: HashMap<Key, Sender<RpcFrame>> = HashMap::new();

        let WireContext { mut outgoing, incoming, mut new_subs } = ctx;

        loop {
            // Wait for EITHER a serialized request, OR some data from the embedded device
            select! {
                sub = new_subs.recv() => {
                    let Some(si) = sub else {
                        return;
                    };

                    subs.insert(si.key, si.tx);
                }
                out = outgoing.recv() => {
                    // Receiver returns None when all Senders have hung up
                    let Some(msg) = out else {
                        return;
                    };

                    // Turn the serialized message into a COBS encoded message
                    //
                    // TODO: this is a little wasteful, payload is already a vec,
                    // then we serialize it to a second vec, then encode that to
                    // a third cobs-encoded vec. Oh well.
                    let msg = msg.to_bytes();
                    let mut msg = encode_vec(&msg);
                    msg.push(0);


                    // And send it!
                    if port.write(&msg).await.is_err() {
                        // I guess the serial port hung up.
                        return;
                    }
                }
                inc = port.read(&mut buf) => {
                    // if read errored, we're done
                    let Ok(used) = inc else {
                        return;
                    };
                    let mut window = &buf[..used];

                    'cobs: while !window.is_empty() {
                        window = match acc.feed(window) {
                            // Consumed the whole USB frame
                            FeedResult::Consumed => break 'cobs,
                            // Silently ignore line errors
                            // TODO: probably add tracing here
                            FeedResult::OverFull(new_wind) => new_wind,
                            FeedResult::DeserError(new_wind) => new_wind,
                            // We got a message! Attempt to dispatch it
                            FeedResult::Success { data, remaining } => {
                                // Attempt to extract a header so we can get the sequence number
                                if let Ok((hdr, body)) = extract_header_from_bytes(data) {
                                    // Got a header, turn it into a frame
                                    let frame = RpcFrame { header: hdr.clone(), body: body.to_vec() };

                                    // Give priority to subscriptions. TBH I only do this because I know a hashmap
                                    // lookup is cheaper than a waitmap search.
                                    if let Some(tx) = subs.get_mut(&hdr.key) {
                                        // Yup, we have a subscription
                                        if tx.send(frame).await.is_err() {
                                            // But if sending failed, the listener is gone, so drop it
                                            subs.remove(&hdr.key);
                                        }
                                    } else {
                                        // Wake the given sequence number. If the WaitMap is closed, we're done here
                                        if let Err(ProcessError::Closed) = incoming.process(frame) {
                                            return;
                                        }
                                    }
                                }

                                remaining
                            }
                        };
                    }
                }
            }
        }
    }
}
