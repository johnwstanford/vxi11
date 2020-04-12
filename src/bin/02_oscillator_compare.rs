
extern crate serde;
extern crate vxi11;

use std::io::{self, Error, ErrorKind};
use std::thread;
use std::time::Duration;

use vxi11::devices::sds1202x::{SDS1202X, TriggerMode};

pub fn main() -> io::Result<()> {

	// TODO: search for IP addresses instead of needing them provided
	let host_sds1202x = "192.168.2.2";

	let mut sds1202x:SDS1202X = SDS1202X::new(host_sds1202x)?;

	eprintln!("Initial device state");
	eprintln!("{}", serde_json::to_string_pretty(&(sds1202x.get_full_state()?))?);

	// Set up both channels
	for ch in &[1,2] {
		sds1202x.set_trace_display_enabled(*ch, true)?;
		sds1202x.set_voltage_div(*ch, 1.0)?;                          // Voltage division
		sds1202x.set_voltage_ofs(*ch, 0.0)?;						  // Voltage offset
	}
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one

	// loop {
		// Capture the samples
		sds1202x.set_trigger_mode(TriggerMode::Single)?;
		sds1202x.wait()?;
	
		let ch1a:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
		let ch2a:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;

		println!("{}", sds1202x.ask_str("STO C1,M1")?);
		thread::sleep(Duration::new(1, 0));

		sds1202x.set_trigger_mode(TriggerMode::Single)?;
		sds1202x.wait()?;

		let ch1b:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
		let ch2b:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;

		// Record the sample rate after acquisition
		let samp_rate_sps:f32 = sds1202x.get_sample_rate()?;
		eprintln!("Captured two waveforms at {:.1e} [samples/sec]", samp_rate_sps);

		println!("{{\"ch1a\":{:?}, \"ch2a\":{:?}, \"ch1b\":{:?}, \"ch2b\":{:?}}}", ch1a, ch2a, ch1b, ch2b);
	// }

	Ok(())
}
