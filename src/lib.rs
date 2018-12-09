pub use libusb;

use std::time::Duration;
use libusb::{Context, Direction, Error, Device, TransferType, DeviceDescriptor, Result};

const VENDOR_ID:u16 = 0x1209;     // InterBiometrics
const PRODUCT_ID:u16 = 0x5a22;    // ikari_01 sd2snes

#[derive(Debug, Default, Clone, Copy)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8
}

pub struct Usb2snes<'a> {
    handle: libusb::DeviceHandle<'a>,
    endpoint_in: Endpoint,
    endpoint_out: Endpoint,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Opcode {
    /// Get memory operation
    Get = 0,
    /// Put memory operation
    Put,
    /// Video RAM get memory operation
    Vget,
    /// Vido RAM put memory operation
    Vput,

    // file system operations
    Ls,
    Mkdir,
    Rm,
    Mv,

    // special operations
    Reset,
    Boot,
    PowerCycle,
    Info,
    MenuResut,
    Stream,
    Time,

    // response
    Respose,
}

pub enum Space
{
    File = 0,
    Snes,
    Msu,
    Cmd,
    Config,
}

pub enum Flags
{
    NoFlag = 0,
    SkipReset = 1,
    OnlyReset = 2,
    Clrx = 4,
    Setx = 8,
    StreamBurst = 16,
    Noresp = 64,
    Data64b = 128,
}


impl<'a> Usb2snes<'a> {
    ///
    /// Creates a Usb2snes instance. This function will assume the default Vendor Id (0x1209) and
    /// Product Id (0x5a22) for the SD2SNES USB connection.
    ///
    pub fn new(context: &'a Context) -> Result<Usb2snes<'a>> {
        Self::new_from_vid_pid(context, VENDOR_ID, PRODUCT_ID)
    }

    pub fn new_from_vid_pid(context: &'a Context, vendor_id: u16, product_id: u16) -> Result<Usb2snes<'a>> {
        let (mut device, desc, mut handle) = Self::open_device(context, vendor_id, product_id)?;

        // Try bulk based
        let endpoints = Self::get_end_points(&mut device, &desc, TransferType::Bulk);

        if let Some(ends) = endpoints {
            /*
            if let Err(e) = Self::configure_endpoint(&mut handle, &ends.0) {
                println!("Unable to configure endpoint 0 {:?}", e);
                return Err(Error::Other);
            }
            */

            if let Err(e) = Self::configure_endpoint(&mut handle, &ends.1) {
                println!("Unable to configure endpoint 1 {:?}", e);
                return Err(Error::Other);
            }

            println!("Found connection to snes! (bulk)");

            if let Some(ends) = endpoints {
                return Ok(Usb2snes {
                    handle: handle,
                    endpoint_in: ends.0,
                    endpoint_out: ends.1,
                });
            }
        }

        println!("Found no connection to snes! :(");

        // No end points found
        Err(Error::Other)
    }

    fn open_device(context: &'a libusb::Context, vid: u16, pid: u16) ->
        Result<(libusb::Device<'a>, libusb::DeviceDescriptor, libusb::DeviceHandle<'a>)>
    {
        let devices = context.devices()?;

        for device in devices.iter() {
            let desc = match device.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue
            };

            if desc.vendor_id() == vid && desc.product_id() == pid {
                println!("found snes id!");
                match device.open() {
                    Ok(handle) => return Ok((device, desc, handle)),
                    Err(e) => return Err(e),
                }
            }
        }

        // Should be not found
        Err(Error::Other)
    }

    fn configure_endpoint(handle: &mut libusb::DeviceHandle, endpoint: &Endpoint) -> libusb::Result<()> {
        let has_kernel_driver = match handle.kernel_driver_active(endpoint.iface) {
            Ok(true) => {
                println!("Detaching kernel driver");
                handle.detach_kernel_driver(endpoint.iface)?;
                true
            },
            _ => false
        };

        println!(" - kernel driver? {}", has_kernel_driver);

        //handle.set_active_configuration(endpoint.config)?;
        //handle.claim_interface(endpoint.iface)?;
        //handle.set_alternate_setting(endpoint.iface, endpoint.setting)?;

        if has_kernel_driver {
            handle.attach_kernel_driver(endpoint.iface)?;
        }

        Ok(())
    }

    fn get_end_points(device: &mut Device, device_desc: &DeviceDescriptor, transfer_type: TransferType) -> Option<(Endpoint, Endpoint)> {
        let mut endpoint_in = None;
        let mut endpoint_out = None;

        for n in 0..device_desc.num_configurations() {
            let config_desc = match device.config_descriptor(n) {
                Ok(c) => c,
                Err(_) => continue
            };

            for interface in config_desc.interfaces() {
                for interface_desc in interface.descriptors() {
                    for endpoint_desc in interface_desc.endpoint_descriptors() {
                        if endpoint_desc.transfer_type() != transfer_type {
                            continue;
                        }

                        if endpoint_desc.direction() == Direction::In {
                            endpoint_in = Some(Endpoint {
                                config: config_desc.number(),
                                iface: interface_desc.interface_number(),
                                setting: interface_desc.setting_number(),
                                address: endpoint_desc.address()
                            });
                        } else if endpoint_desc.direction() == Direction::Out {
                            endpoint_out = Some(Endpoint {
                                config: config_desc.number(),
                                iface: interface_desc.interface_number(),
                                setting: interface_desc.setting_number(),
                                address: endpoint_desc.address()
                            });
                        }
                    }
                }
            }
        }

        if endpoint_in.is_some() && endpoint_out.is_some() {
            Some((endpoint_in.unwrap(), endpoint_out.unwrap()))
        } else {
            None
        }
    }

    fn fill_header(data: &mut [u8], op_code: Opcode) {
        data[0] = b'U';
        data[1] = b'S';
        data[2] = b'B';
        data[3] = b'A';
        data[4] = op_code as u8;
        data[5] = Space::Snes as u8;
        data[6] = Flags::Noresp as u8;
    }

    pub fn get_memory(&self, offset: u32, size: u32) -> Result<Vec<u8>> {
        let mut command: [u8; 512] = [0; 512];
        let mut output: [u8; 512] = [0; 512];

        Self::fill_header(&mut command, Opcode::Get);

        // max 5 milisec waiting as we need real-time performance
        let timeout = Duration::from_millis(500);

        // Memory offset
        command[256] = ((offset >> 24) & 0xff) as u8;
        command[257] = ((offset >> 16) & 0xff) as u8;
        command[258] = ((offset >> 8) & 0xff) as u8;
        command[259] = ((offset >> 0) & 0xff) as u8;

        // size
        command[252] = ((size >> 24) & 0xff) as u8;
        command[253] = ((size >> 16) & 0xff) as u8;
        command[254] = ((size >> 8) & 0xff) as u8;
        command[255] = ((size >> 0) & 0xff) as u8;

        self.clear_read();

        //println!("Writing to {:?}", self.endpoint_out);
        // TODO: Make sure that we write as much as we expect
        match self.handle.write_bulk(self.endpoint_out.address, &command, timeout) {
            Ok(_) => (),
            Err(err) => {
                println!("could not write to endpoint: {}", err);
                return Err(Error::Other);
            }
        }

        let mut fail_counts = 0;
        let mut size_count = size as i32;
        let mut result = Vec::with_capacity(size as usize);

        loop
        {
            match self.handle.read_bulk(self.endpoint_in.address, &mut output, timeout) {
                Ok(len) => {
                    //println!("len back {}", len);
                    size_count -= len as i32;

                    for t in output.iter() {
                        result.push(*t);
                    }
                }

                Err(_err) => {
                    fail_counts += 1;
                }
            }

            if fail_counts == 1000 {
                return Err(Error::Other);
            }

            if size_count <= 0 {
                break;
            }
        }

        Ok(result)
    }


    pub fn clear_read(&self) {
        let mut temp: [u8; 64] = [0; 64];
        let timeout = Duration::from_millis(50);

        loop {
            let len = match self.handle.read_bulk(self.endpoint_in.address, &mut temp, timeout) {
                Ok(len) => { println!("clear read: {}", len); len }
                Err(err) => { println!("nothing to read {}", err); 0 },
            };

            if len == 0 {
                break;
            }
        }
    }
}

