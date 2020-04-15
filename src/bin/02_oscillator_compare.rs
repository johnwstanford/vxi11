
extern crate serde;
extern crate vxi11;

use std::io;
use std::thread;
use std::time::{Duration, Instant};

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

	// TODO: search for IP addresses instead of needing them provided
	let host_spd3303x = "192.168.2.2";
	let host_sds1202x = "192.168.2.3";
	let host_sdg2402x = "192.168.2.4";

	let mut spd3303x:SPD3303X = SPD3303X::new(host_spd3303x)?;
	let mut sds1202x:SDS1202X = SDS1202X::new(host_sds1202x)?;
	let mut sdg2042x:SDG2042X = SDG2042X::new(host_sdg2402x)?;

	eprintln!("{:?}", spd3303x.get_full_state()?);

	eprintln!("{:?}", spd3303x.ask_str("INST CH1")?);
	thread::sleep(Duration::new(1, 0));
	eprintln!("{:?}", spd3303x.ask_str("INST?")?);

	eprintln!("{:?}", spd3303x.ask_str("INST CH2")?);
	thread::sleep(Duration::new(1, 0));
	eprintln!("{:?}", spd3303x.ask_str("INST?")?);

	eprintln!("{:?}", spd3303x.ask_str("INST CH3")?);
	thread::sleep(Duration::new(1, 0));
	eprintln!("{:?}", spd3303x.ask_str("INST?")?);

	// TODO: set up waveform generator channel

	// Set up both channels
	sds1202x.set_voltage_div(1, 2.0)?;
	sds1202x.set_voltage_div(2, 1.0)?;
	for ch in &[1,2] {
		sds1202x.set_trace_display_enabled(*ch, true)?;
		sds1202x.set_voltage_ofs(*ch, 0.0)?;						  // Voltage offset
	}
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one


	// Reset the counters
	// sdg2042x.set_output(1, true)?;
	// thread::sleep(Duration::new(1, 0));
	// sdg2042x.set_output(1, false)?;
	let t0 = Instant::now();

	// Trigger the samples
	sds1202x.set_trigger_mode(TriggerMode::Single)?;
	sds1202x.wait()?;
	let t_ms = t0.elapsed().as_millis();

	// Retrieve the waveforms
	let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
	let ch2:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;

	let ans = TriggerResult{ t_ms, ch1, ch2 };	

	println!("{}", serde_json::to_string_pretty(&ans).unwrap());

	sds1202x.set_trigger_mode(TriggerMode::Norm)?;

	Ok(())
}
