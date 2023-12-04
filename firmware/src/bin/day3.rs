#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use aoc_2023_firmware::{bsp, rpc};
use aoc_2023_icd::day3::{Engine, EngineReq, EngineResp, Number, Symbol};
use defmt::info;
use embassy_executor::Spawner;
use heapless::{String, Vec};
use postcard_rpc::headered::extract_header_from_bytes;
use postcard_rpc::Endpoint;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut board = bsp::init(p);
    spawner.must_spawn(usb_task(rpc::Server::new(board.usb.reader, board.usb.writer)));
    board.usb.usb.run().await;
}

#[embassy_executor::task]
async fn usb_task(mut server: rpc::Server) {
    loop {
        server.wait_connection().await;
        info!("Connected");
        let mut y = 0;
        let mut prev_line: Option<(Vec<Number, 128>, Vec<Symbol, 128>)> = None;
        loop {
            match server.receive().await {
                Ok(frame) => {
                    if let Ok((hdr, body)) = extract_header_from_bytes(&frame) {
                        match hdr.key {
                            Engine::REQ_KEY => {
                                let msg = postcard::from_bytes::<<Engine as postcard_rpc::Endpoint>::Request>(body).unwrap();
                                match msg {
                                    EngineReq::Reset => {
                                        info!("RESET");
                                        prev_line = None;
                                        y = 0;
                                        server.reply::<Engine>(hdr.seq_no, &EngineResp { result: Vec::new() }).await.unwrap();
                                    }
                                    EngineReq::Data(line) => {
                                        let mut numbers: Vec<Number, 128> = Vec::new();
                                        let mut symbols: Vec<Symbol, 128> = Vec::new();
                                        let mut current_number: String<3> = String::new();

                                        for (x, c) in line.char_indices() {
                                            if c.is_numeric() {
                                                current_number.push(c).unwrap();
                                                if x < line.chars().count() - 1 {
                                                    continue;
                                                }
                                            } else if c != '.' {
                                                symbols.push(Symbol { x: x as u8, y, symbol: c }).ok();
                                            }
                                            if current_number.len() > 0 {
                                                numbers
                                                    .push(Number {
                                                        x: ((x.saturating_sub(current_number.len()) as u8), x as u8),
                                                        y,
                                                        value: current_number.parse().unwrap(),
                                                    })
                                                    .ok();
                                                current_number.clear();
                                            }
                                        }
                                        let parts = numbers
                                            .clone()
                                            .into_iter()
                                            .filter(|n| symbols.iter().any(|s| (n.x.0..=n.x.1).contains(&(s.x + 1)) || (n.x.0..=n.x.1).contains(&(s.x))));
                                        let mut out: Vec<Number, 64> = Vec::from_iter(parts);

                                        if let Some((nums, syms)) = prev_line.take() {
                                            let parts2 = numbers
                                                .clone()
                                                .into_iter()
                                                .filter(|n| syms.iter().any(|ps| (n.x.0..=n.x.1).contains(&(ps.x + 1)) || (n.x.0..=n.x.1).contains(&(ps.x))));
                                            out.extend(parts2);
                                            let parts3 = nums
                                                .into_iter()
                                                .filter(|n| symbols.iter().any(|s| (n.x.0..=n.x.1).contains(&(s.x + 1)) || (n.x.0..=n.x.1).contains(&(s.x))));
                                            out.extend(parts3);
                                        }

                                        //let res = Vec::from_iter(out.iter().map(|n: &Number| n.value));
                                        server.reply::<Engine>(hdr.seq_no, &EngineResp { result: out }).await.unwrap();
                                        y += 1;
                                        prev_line.replace((numbers, symbols));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => break,
            }
        }
        info!("Disconnected");
    }
}
