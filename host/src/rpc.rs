use std::collections::HashMap;

use nusb::transfer::{Completion, RequestBuffer};
use nusb::{Device, Interface};
use postcard::experimental::schema::Schema;
use postcard_rpc::accumulator::raw::{CobsAccumulator, FeedResult};
use postcard_rpc::headered::extract_header_from_bytes;
use postcard_rpc::host_client::{HostClient, ProcessError, RpcFrame, WireContext};
use postcard_rpc::Key;
use serde::de::DeserializeOwned;
use tokio::select;
use tokio::sync::mpsc::Sender;

pub fn new_client<E: DeserializeOwned + Schema>(device: Device, err_uri_path: &str, outgoing_depth: usize) -> HostClient<E> {
    let mut comm = UsbComm::new(device);
    let (client, wire) = HostClient::<E>::new_manual(err_uri_path, outgoing_depth);
    tokio::task::spawn(async move { comm.wire_worker(wire).await });
    client
}

struct UsbComm {
    interface: Interface,
}

impl UsbComm {
    pub fn new(device: Device) -> Self {
        Self {
            interface: device.claim_interface(0).unwrap(),
        }
    }

    pub async fn write(&mut self, data: &[u8]) -> Result<(), ()> {
        let _ = self.interface.bulk_out(0x01, data.into()).await;
        Ok(())
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        if let Completion { data, status: Ok(()) } = self.interface.bulk_in(0x81, RequestBuffer::new(4096)).await {
            buf[..data.len()].copy_from_slice(&data);
            return Ok(data.len());
        }
        Err(())
    }
    async fn wire_worker(&mut self, ctx: WireContext) {
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
                    let mut msg = cobs::encode_vec(&msg);
                    msg.push(0);


                    // And send it!
                    if self.write(&msg).await.is_err() {
                        // I guess the serial port hung up.
                        return;
                    }
                }
                inc = self.read(&mut buf) => {
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
