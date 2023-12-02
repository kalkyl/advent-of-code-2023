#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use aoc_2023_firmware::bsp;
use aoc_2023_icd::day2::{process, ClientToHost, HostToClient, StateMachine};
use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{self, In, Out};
use embassy_usb::driver::{Endpoint as _, EndpointIn, EndpointOut};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut board = bsp::init(p);
    let client = Client::new(board.usb.reader, board.usb.writer);
    spawner.must_spawn(usb_task(client));
    board.usb.usb.run().await;
}

#[embassy_executor::task]
async fn usb_task(mut client: Client) {
    loop {
        client.wait_connection().await;
        info!("Connected");
        loop {
            if process(&mut client).await.is_err() {
                break;
            }
        }
    }
}

pub struct Client {
    reader: usb::Endpoint<'static, USB, Out>,
    writer: usb::Endpoint<'static, USB, In>,
    game_counter: u32,
    result_a: u32,
    result_b: u32,
}

impl Client {
    pub fn new(reader: usb::Endpoint<'static, USB, Out>, writer: usb::Endpoint<'static, USB, In>) -> Self {
        Self {
            reader,
            writer,
            result_a: 0,
            result_b: 0,
            game_counter: 1,
        }
    }

    pub async fn receive(&mut self) -> Result<HostToClient, ()> {
        let mut raw_buf = [0; 64];
        let mut cobs_buf: CobsAccumulator<1024> = CobsAccumulator::new();
        while let Ok(ct) = self.reader.read(&mut raw_buf).await {
            let buf = &raw_buf[..ct];
            let mut window = &buf[..];
            'cobs: while !window.is_empty() {
                window = match cobs_buf.feed::<HostToClient>(&window) {
                    FeedResult::Consumed => break 'cobs,
                    FeedResult::OverFull(new_wind) => new_wind,
                    FeedResult::DeserError(new_wind) => new_wind,
                    FeedResult::Success { data, .. } => return Ok(data),
                };
            }
        }
        Err(())
    }

    pub async fn send(&mut self, message: ClientToHost) -> Result<(), ()> {
        let mut buf = [0; 64];
        let data = postcard::to_slice(&message, &mut buf).map_err(drop)?;
        self.writer.write(data).await.map_err(drop)
    }

    pub async fn wait_connection(&mut self) {
        self.reader.wait_enabled().await;
    }
}

impl StateMachine for Client {
    async fn start(&mut self) -> Result<(), ()> {
        match self.receive().await? {
            HostToClient::Start => {
                self.game_counter = 1;
                self.result_a = 0;
                self.result_b = 0;
                self.send(ClientToHost::Started).await?;
                info!("START");
                Ok(())
            }
            _ => Err(()),
        }
    }

    async fn write_next_game(&mut self) -> Result<(), ()> {
        match self.receive().await? {
            HostToClient::GameData(line) => {
                if let Some((_, games)) = line.split_once(": ") {
                    let games = games.split("; ").map(Game::from_str);
                    // A
                    if games.clone().all(|g| g.blue <= 14 && g.green <= 13 && g.red <= 12) {
                        self.result_a += self.game_counter;
                    }
                    // B
                    let min_blue = games.clone().map(|g| g.blue).max().unwrap_or(0);
                    let min_green = games.clone().map(|g| g.green).max().unwrap_or(0);
                    let min_red = games.map(|g| g.red).max().unwrap_or(0);
                    self.result_b += min_blue * min_green * min_red;
                }
                self.send(ClientToHost::GameDataWritten).await?;
                self.game_counter += 1;
            }
            _ => return Err(()),
        }
        Ok(())
    }

    async fn end(&mut self) -> Result<(u32, u32), ()> {
        match self.receive().await? {
            HostToClient::GetResult => {
                self.send(ClientToHost::Result((self.result_a, self.result_b))).await?;
                info!("Result A: {}", self.result_a);
                info!("Result B: {}", self.result_a);
                Ok((self.result_a, self.result_b))
            }
            _ => Err(()),
        }
    }
}

#[derive(Default)]
struct Game {
    blue: u32,
    green: u32,
    red: u32,
}

impl Game {
    fn from_str(s: &str) -> Self {
        let mut game: Game = Default::default();
        for color in s.split(", ") {
            match color.trim().split_once(" ") {
                Some((n, "blue")) => game.blue = n.parse().unwrap(),
                Some((n, "green")) => game.green = n.parse().unwrap(),
                Some((n, "red")) => game.red = n.parse().unwrap(),
                _ => (),
            }
        }
        game
    }
}
