
extern crate rppal;
extern crate serde;
extern crate vxi11;

use std::io::{self, Error, ErrorKind};
use std::thread;
use std::time::{Duration, Instant};

use rppal::gpio::{Gpio, Mode, Level};
use rppal::system::DeviceInfo;

use serde::{Serialize, Deserialize};

use vxi11::devices::sds1202x::{SDS1202X, TriggerMode};
use vxi11::devices::sdg2042x::{SDG2042X, Wavetype};
use vxi11::devices::spd3303x::{SPD3303X};

#[derive(Debug, Serialize, Deserialize)]
struct TriggerResult {
	t_ms: u128,
	ch1: Vec<i8>,
	ch2: Vec<i8>,
}

pub fn main() -> io::Result<()> {

	// let device_info = match DeviceInfo::new() {
	// 	Ok(dev) => dev,
	// 	Err(_) => return Err(Error::new(ErrorKind::Other, "Unable to get device info, may not be running on Raspberry Pi"))
	// };
	// println!("Running on {} with {}", device_info.model(), device_info.soc());

	// TODO: search for IP addresses instead of needing them provided
	let host_spd3303x = "192.168.2.2";
	let host_sds1202x = "192.168.2.3";

	let mut spd3303x:SPD3303X = SPD3303X::new(host_spd3303x)?;
	let mut sds1202x:SDS1202X = SDS1202X::new(host_sds1202x)?;

	// Set up DC power supply
	eprintln!("{}", serde_json::to_string_pretty(&spd3303x.get_full_state()?)?);

	// Set up oscilloscope
	sds1202x.set_voltage_div(1, 1.0)?;
	sds1202x.set_voltage_div(2, 1.0)?;
	sds1202x.set_time_division(1.0e-6)?;
	for ch in &[1,2] {
		sds1202x.set_trace_display_enabled(*ch, true)?;
		sds1202x.set_voltage_ofs(*ch, 0.0)?;						  // Voltage offset
	}
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one

	// Reset the counters using GPIO
	let t0 = Instant::now();

	// Trigger the samples
	sds1202x.set_trigger_mode(TriggerMode::Single)?;
	sds1202x.wait()?;
	let t_ms = t0.elapsed().as_millis();

	// Retrieve the waveforms
	let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
	let ch2:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;

	let ans = TriggerResult{ t_ms, ch1, ch2 };	

	// println!("{}", serde_json::to_string_pretty(&ans).unwrap());

	sds1202x.set_trigger_mode(TriggerMode::Norm)?;

	Ok(())
}
