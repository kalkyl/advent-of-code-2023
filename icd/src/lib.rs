#![no_std]
#![feature(async_fn_in_trait)]

pub const VID: u16 = 0xc0de;
pub const PID: u16 = 0xcafe;

pub mod day1 {
    use heapless::String;
    use serde::{Deserialize, Serialize};

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
        Result(u32),
    }
}

pub mod day2 {
    use heapless::String;
    use serde::{Deserialize, Serialize};

    pub trait StateMachine {
        async fn start(&mut self) -> Result<(), ()>;
        async fn write_next_game(&mut self) -> Result<(), ()>;
        async fn end(&mut self) -> Result<(u32, u32), ()>;
    }

    pub async fn process(sm: &mut impl StateMachine) -> Result<(u32, u32), ()> {
        sm.start().await?;
        while sm.write_next_game().await.is_ok() {}
        sm.end().await
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum HostToClient {
        Start,
        GameData(String<1024>),
        End,
        GetResult,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ClientToHost {
        Started,
        GameDataWritten,
        Result((u32, u32)),
    }
}
