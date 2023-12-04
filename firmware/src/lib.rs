#![no_std]
#![feature(type_alias_impl_trait)]

pub mod rpc;

pub mod bsp {
    use aoc_2023_icd::{PID, VID};
    use embassy_rp::peripherals::USB;
    use embassy_rp::usb::{self, Driver, In, InterruptHandler, Out};
    use embassy_rp::{bind_interrupts, Peripherals};
    use embassy_usb::msos::{self, windows_version};
    use embassy_usb::{Builder, Config};
    use static_cell::make_static;

    const DEVICE_INTERFACE_GUIDS: &[&str] = &["{AFB9A6FB-30BA-44BC-9232-806CFC875321}"];

    bind_interrupts!(struct Irqs {
        USBCTRL_IRQ => InterruptHandler<USB>;
    });

    pub struct UsbParts {
        pub usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>,
        pub reader: usb::Endpoint<'static, USB, Out>,
        pub writer: usb::Endpoint<'static, USB, In>,
    }
    pub struct Board {
        pub usb: UsbParts,
    }

    pub fn init(p: Peripherals) -> Board {
        let driver = Driver::new(p.USB, Irqs);

        let mut config = Config::new(VID, PID);
        config.manufacturer = Some("Embassy");
        config.product = Some("Advent of code 2023");
        config.serial_number = Some("12345678");
        config.max_power = 100;
        config.max_packet_size_0 = 64;
        config.device_class = 0xEF;
        config.device_sub_class = 0x02;
        config.device_protocol = 0x01;
        config.composite_with_iads = true;

        let mut builder = Builder::new(
            driver,
            config,
            make_static!([0; 256]),
            make_static!([0; 256]),
            make_static!([0; 256]),
            make_static!([0; 256]),
            make_static!([0; 64]),
        );

        builder.msos_descriptor(windows_version::WIN8_1, 0);
        builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
        builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
            "DeviceInterfaceGUIDs",
            msos::PropertyData::RegMultiSz(DEVICE_INTERFACE_GUIDS),
        ));

        // Add a vendor-specific function (class 0xFF)
        let mut function = builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0, 0, None);
        let reader = alt.endpoint_bulk_out(64);
        let writer = alt.endpoint_bulk_in(64);
        drop(function);

        let usb = builder.build();

        Board {
            usb: UsbParts { usb, reader, writer },
        }
    }
}

pub mod usb {
    use embassy_rp::peripherals::USB;
    use embassy_rp::usb::{self, In, Out};
    use embassy_usb::driver::{Endpoint as _, EndpointIn, EndpointOut};
    use embedded_io_async::{ErrorKind, ErrorType, Read, Write};
    pub struct RawUsb {
        reader: usb::Endpoint<'static, USB, Out>,
        writer: usb::Endpoint<'static, USB, In>,
    }

    impl RawUsb {
        pub fn new(reader: usb::Endpoint<'static, USB, Out>, writer: usb::Endpoint<'static, USB, In>) -> Self {
            Self { reader, writer }
        }
        pub async fn wait_connection(&mut self) {
            self.reader.wait_enabled().await;
        }
    }

    impl ErrorType for RawUsb {
        type Error = ErrorKind;
    }

    impl Read for RawUsb {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            self.reader.read(buf).await.map_err(|_| ErrorKind::BrokenPipe)
        }
    }

    impl Write for RawUsb {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            for chunk in buf.chunks(64) {
                self.writer.write(chunk).await.map_err(|_| ErrorKind::BrokenPipe)?;
            }
            Ok(buf.len())
        }
    }

    impl super::rpc::RpcServer<1024, 4096> for RawUsb {}
}
