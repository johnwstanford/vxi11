
use std::time::{Duration, Instant};

use vxi11::devices::sds1202x::SDS1202X;

const TDIV_SEC: f32 = 50.0;

fn main() -> Result<(), &'static str> {

    let mut dev = SDS1202X::new("25.0.0.2").unwrap();

    println!("{:#?}", dev.get_full_state());

    dev.set_time_division(TDIV_SEC).unwrap();
    dev.set_voltage_div(1, 1.0).unwrap();

    dev.arm_single().unwrap();
    std::thread::sleep(Duration::from_secs(2));

    dev.force_trigger().unwrap();
    std::thread::sleep(Duration::from_secs_f32(TDIV_SEC*15.0));

    let wf = dev.transfer_waveform(1).unwrap();
    let wf_json = serde_json::to_string_pretty(&wf).unwrap();
    std::fs::write("./ex100.json", wf_json.as_bytes()).unwrap();

    Ok(())
}