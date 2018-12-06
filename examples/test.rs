use usb2snes::Usb2snes;
use libusb;

fn main() {
    let mut context = libusb::Context::new().unwrap();
    let mut usb2snes = Usb2snes::new(&context).unwrap();
    //usb2snes.send_command();

    loop
    {
        let res = usb2snes.get_memory(0x7E079B, 7).unwrap();

        if res.len() >= 7 {
            println!("current room rmb       {:x}", res[0]);
            println!("First byte of room mdb {:x}", res[1]);
            println!("Region number          {:x}", res[2]);
            println!("Room's X coord         {:x}{:x}", res[3],res[4]);
            println!("Room's Y coord         {:x}{:x}", res[5],res[6]);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
