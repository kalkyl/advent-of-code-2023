
    use embassy_rp::peripherals::USB;
    use embassy_rp::usb::{self, In, Out};
    use embassy_usb::driver::{Endpoint as _, EndpointIn, EndpointOut};
    use heapless::Vec;
    use postcard_rpc::accumulator::raw::{CobsAccumulator, FeedResult};
    use postcard_rpc::{headered, Endpoint, Topic};
    use serde::Serialize;

    pub struct Server {
        reader: usb::Endpoint<'static, USB, Out>,
        writer: usb::Endpoint<'static, USB, In>,
    }

    impl Server {
        pub fn new(reader: usb::Endpoint<'static, USB, Out>, writer: usb::Endpoint<'static, USB, In>) -> Self {
            Self { reader, writer }
        }

        pub async fn wait_connection(&mut self) {
            self.reader.wait_enabled().await;
        }

        pub async fn receive(&mut self) -> Result<Vec<u8, 1024>, ()> {
            let mut raw_buf = [0; 64];
            let mut cobs_buf: CobsAccumulator<1024> = CobsAccumulator::new();
            while let Ok(ct) = self.reader.read(&mut raw_buf).await {
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
            Err(())
        }

        pub async fn reply<E: Endpoint>(&mut self, seq_no: u32, msg: &E::Response) -> Result<(), ()>
        where
            E::Response: Serialize,
        {
            let mut buf = [0; 4096];
            let data = headered::to_slice_cobs(seq_no, E::PATH, msg, &mut buf).expect("SER");
            for chunk in data.chunks(64){
                self.writer.write(chunk).await.map_err(drop)?;
            }
            Ok(())
        }

        pub async fn publish<T: Topic>(&mut self, seq_no: u32, msg: &T::Message) -> Result<(), ()>
        where
            T::Message: Serialize,
        {
            let mut buf = [0; 4096];
            let data = headered::to_slice_cobs(seq_no, T::PATH, msg, &mut buf).unwrap();
            for chunk in data.chunks(64){
                self.writer.write(chunk).await.map_err(drop)?;
            }
            Ok(())
        }
    }

