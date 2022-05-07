use std::time::Duration;
use vxi11::devices::spd3303x::SPD3303X;

fn main() -> Result<(), &'static str> {

    let mut dev = SPD3303X::new("25.0.0.1").unwrap();

    println!("{:#?}", dev.get_full_state().unwrap());

    dev.enable_output(1).unwrap();

    std::thread::sleep(Duration::from_secs(5));

    dev.disable_output(1).unwrap();

    Ok(())
}