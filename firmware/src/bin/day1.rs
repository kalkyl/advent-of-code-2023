#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use aoc_2023_firmware::bsp;
use aoc_2023_icd::day1::{ClientToHost, HostToClient};
use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Endpoint, In, Out};
use embassy_usb::driver::{Endpoint as _, EndpointError, EndpointIn, EndpointOut};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut board = bsp::init(p);
    spawner.must_spawn(usb_task(board.usb.reader, board.usb.writer));
    board.usb.usb.run().await;
}

#[embassy_executor::task]
async fn usb_task(mut reader: Endpoint<'static, USB, Out>, mut writer: Endpoint<'static, USB, In>) {
    loop {
        reader.wait_enabled().await;
        info!("Connected");
        let mut sum_a = 0;
        let mut sum_b = 0;
        loop {
            let mut data = [0; 64];
            match reader.read(&mut data).await {
                Ok(n) => {
                    if let Ok(msg) = postcard::from_bytes::<HostToClient>(&data[..n]) {
                        match msg {
                            HostToClient::Data(line) => process_data(&line, &mut sum_a, &mut sum_b),
                            HostToClient::GetResultA => {
                                info!("Sum A: {}", sum_a);
                                if respond(&mut writer, sum_a).await.is_err() {
                                    break;
                                };
                            }
                            HostToClient::GetResultB => {
                                info!("Sum B: {}", sum_b);
                                if respond(&mut writer, sum_b).await.is_err() {
                                    break;
                                };
                            }
                            HostToClient::Reset => {
                                sum_a = 0;
                                sum_b = 0;
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        info!("Disconnected");
    }
}

async fn respond(writer: &mut Endpoint<'_, USB, In>, value: u32) -> Result<(), EndpointError> {
    let mut buf = [0; 8];
    if let Ok(response) = postcard::to_slice(&ClientToHost::Result(value), &mut buf) {
        writer.write(response).await?;
    }
    Ok(())
}

fn process_data(line: &str, sum_a: &mut u32, sum_b: &mut u32) {
    fn find_first_last(items: &[Option<(usize, u32)>]) -> (u32, u32) {
        let (_, first) = items.into_iter().filter_map(|&x| x).min_by_key(|&(i, _)| i).unwrap();
        let (_, last) = items.into_iter().filter_map(|&x| x).max_by_key(|&(i, _)| i).unwrap();
        (first, last)
    }

    let mut numbers = line.char_indices().filter(|(_, c)| c.is_numeric()).map(|(i, c)| (i, c.to_digit(10).unwrap()));
    let first_num = numbers.next();
    let last_num = numbers.last();
    let (first, last) = find_first_last(&[first_num, last_num]);
    *sum_a += first * 10 + last;

    const NUMBERS: &[&str] = &["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
    let first_letters = NUMBERS
        .iter()
        .enumerate()
        .filter_map(|(n, s)| line.match_indices(s).min_by_key(|&(i, _)| i).map(|(i, _)| (i, n as u32)))
        .min_by_key(|&(i, _)| i);
    let last_letters = NUMBERS
        .iter()
        .enumerate()
        .filter_map(|(n, s)| line.match_indices(s).max_by_key(|&(i, _)| i).map(|(i, _)| (i, n as u32)))
        .max_by_key(|&(i, _)| i);
    let (first, last) = find_first_last(&[first_num, last_num, first_letters, last_letters]);
    *sum_b += first * 10 + last;
}
