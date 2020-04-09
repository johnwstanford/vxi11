
extern crate serde;
extern crate vxi11;

use std::io;

use vxi11::devices::sds1202x::SDS1202X;

pub fn main() -> io::Result<()> {

	// TODO: search for IP addresses instead of needing them provided
	let host_sds1202x = "192.168.2.2";

	let mut sds1202x = SDS1202X::new(host_sds1202x)?;

	eprintln!("Initial device states");
	eprintln!("{}", serde_json::to_string_pretty(&(sds1202x.get_full_state()?)).unwrap());

	// Set up oscilloscope with state that doesn't change with frequency
	// TODO: ensure that both channels are active
	sds1202x.set_voltage_div(1, 1.0)?;                            // Voltage division
	sds1202x.set_voltage_ofs(1, 0.0)?;							  // Voltage offset
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one

	// Set up oscilloscope
	//sds1202x.set_time_division(1.0 / current_freq_hz)?;

	// Acquire once before the real thing to get an accurate sample rate
	sds1202x.arm_single()?;
	sds1202x.force_trigger()?;
	sds1202x.wait()?;
	let samp_rate_sps:f32 = sds1202x.get_sample_rate()?;

	// Capture the samples that count
	sds1202x.arm_single()?;
	sds1202x.wait()?;

	let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
	println!("{:?}", ch1);

	Ok(())
}
