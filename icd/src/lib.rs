#![no_std]

pub const VID: u16 = 0xc0de;
pub const PID: u16 = 0xcafe;

pub mod day1 {
    use serde::{Serialize, Deserialize};
    use heapless::String;

    #[derive(Serialize, Deserialize, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum HostToClient {
        Data(String<64>),
        GetResultA,
        GetResultB,
        Reset,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ClientToHost {
        Result(u32)
    }
}