use usb2snes::Usb2snes;

fn main() {
    let context = rusb::Context::new().unwrap();
    //context.set_log_level(libusb::LogLevel::Debug);
    let usb2snes = Usb2snes::new(&context).unwrap();
    //usb2snes.send_command();

    loop
    {
        if let Ok(res) = usb2snes.get_memory(0xF50000, 2048) {
            if res.len() >= 7 {
                println!("current room rmb       {:x}{:x}", res[0x79b + 1],res[0x79b + 0]);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
