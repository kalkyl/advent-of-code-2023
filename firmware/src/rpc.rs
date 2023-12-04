use embedded_io_async::{ErrorType, Read, Write};
use heapless::Vec;
use postcard_rpc::accumulator::raw::{CobsAccumulator, FeedResult};
use postcard_rpc::{headered, Endpoint, Topic};
use serde::Serialize;

pub trait RpcServer<const READ_SIZE: usize, const WRITE_SIZE: usize>: Read + Write {
    async fn receive(&mut self) -> Result<Vec<u8, READ_SIZE>, <Self as ErrorType>::Error> {
        let mut raw_buf = [0; 64];
        let mut cobs_buf: CobsAccumulator<READ_SIZE> = CobsAccumulator::new();
        loop {
            let ct = self.read(&mut raw_buf).await?;
            let buf = &raw_buf[..ct];
            let mut window = &buf[..];
            'cobs: while !window.is_empty() {
                window = match cobs_buf.feed(&window) {
                    FeedResult::Consumed => break 'cobs,
                    FeedResult::OverFull(new_wind) => new_wind,
                    FeedResult::DeserError(new_wind) => new_wind,
                    FeedResult::Success { data, .. } => {
                        return Ok(Vec::from_slice(data).unwrap());
                    }
                };
            }
        }
    }

    async fn reply<E: Endpoint>(&mut self, seq_no: u32, msg: &E::Response) -> Result<(), <Self as ErrorType>::Error>
    where
        E::Response: Serialize,
    {
        let mut buf = [0; WRITE_SIZE];
        let data = headered::to_slice_cobs(seq_no, E::PATH, msg, &mut buf).unwrap();
        self.write_all(data).await
    }

    async fn publish<T: Topic>(&mut self, seq_no: u32, msg: &T::Message) -> Result<(), <Self as ErrorType>::Error>
    where
        T::Message: Serialize,
    {
        let mut buf = [0; WRITE_SIZE];
        let data = headered::to_slice_cobs(seq_no, T::PATH, msg, &mut buf).unwrap();
        self.write_all(data).await
    }
}
