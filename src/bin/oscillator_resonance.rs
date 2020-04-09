
extern crate serde;
extern crate vxi11;

use std::io;
use std::thread;
use std::time::Duration;

use vxi11::devices::sds1202x::{SDS1202X, TriggerMode};
use vxi11::devices::sdg2042x::{SDG2042X, Wavetype};

pub fn main() -> io::Result<()> {

	// TODO: search for IP addresses instead of needing them provided
	let host_sds1202x = "192.168.2.2";
	let host_sdg2042x = "192.168.2.3";

	let expected_resonance_freq_hz = 16.0e6;
	let min_freq_hz:f32 = 15.8e6;
	let max_freq_hz:f32 = 16.2e6;
	let freq_step_hz:f32 = 1.0e3;

	let amp_v:f32 = 4.0;

	let mut sds1202x = SDS1202X::new(host_sds1202x)?;
	let mut sdg2042x = SDG2042X::new(host_sdg2042x)?;

	println!("Initial device states");
	println!("{}", serde_json::to_string_pretty(&(sds1202x.get_full_state()?)).unwrap());
	println!("{}", serde_json::to_string_pretty(&(sdg2042x.get_full_state()?)).unwrap());

	// Set up oscilloscope with state that doesn't change with frequency
	// TODO: ensure that both channels are active
	sds1202x.set_voltage_div(1, 0.1)?;                            // Voltage division
	sds1202x.set_voltage_div(2, 1.0)?;                            // Voltage division
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one

	// Step through frequencies
	let mut current_freq_hz:f32 = min_freq_hz;
	while current_freq_hz < max_freq_hz {

		// Set up waveform generator
		sdg2042x.set_basic_wavetype(1, Wavetype::Sine, current_freq_hz as u32, amp_v, 0.0, 0.0)?;
		sdg2042x.set_output(1, true)?;
		thread::sleep(Duration::new(2,0));

		// Set up oscilloscope
		sds1202x.set_time_division(10.0 / current_freq_hz)?;

		// Acquire once before the real thing to get an accurate sample rate
		sds1202x.acquire()?;
		sds1202x.wait()?;

		let samp_rate_sps:f32 = sds1202x.get_sample_rate()?;

		// Capture the samples that count
		sds1202x.acquire()?;
		sds1202x.wait()?;

		// Increment the frequency for the next step
		current_freq_hz += freq_step_hz;

	}
	sdg2042x.set_output(1, false)?;

	

	// // Retrieve and decode data
 //    let ch1_data:Vec<u8> = sds1202x.ask(b"C1:WAVEFORM? DAT2")?;
	
	// // TODO: process the rest of the header
	// let (header, body) = ch1_data.split_at(21);
	// let (_, length_str) = header.split_at(12);

	// let length:usize = str::from_utf8(length_str).unwrap().parse::<usize>().unwrap();

	// // TODO: make these configurable and/or populate using requests from device
	// let vdiv = 1.0;
	// let vofs = 0.0;
	// let mut time_domain: Vec<Complex<f64>> = vec![];

	// // TODO: create some kind of sample struct for the SDS-1202X with time and voltage
	// //let mut time = -7.0 * actual_tdiv;
	// let mut rdr = Cursor::new(body);
	// for _ in 0..length {
	// 	let raw_i8:i8 = rdr.read_i8()?;
	// 	//time += 1.0 / samp_rate_sps;
	// 	let voltage:f64 = (raw_i8 as f64)*(vdiv/25.0) - vofs;
	// 	time_domain.push(Complex{ re: voltage, im: 0.0});
	// }

 //    // Perform FFT
	// let mut freq_domain: Vec<Complex<f64>> = vec![Complex::zero(); length];
	// let mut planner = FFTplanner::new(false);
	// let fft = planner.plan_fft(length);
	// fft.process(&mut time_domain, &mut freq_domain);

	// // Find the strongest frequency
	// let mut best_freq:f32 = 0.0;
	// let mut best_amp:f64 = 0.0;
	// for (idx, fft_response) in (&freq_domain).into_iter().enumerate() {

	// 	if best_amp < fft_response.norm_sqr() {
	// 		best_amp = fft_response.norm_sqr();
	// 		best_freq = if idx < (length/2) {
	// 			(idx as f32 * samp_rate_sps) / (length as f32)
	// 		} else {
	// 			((length - idx) as f32 * samp_rate_sps) / (length as f32)
	// 		};
	// 	}
	// }

	// println!("{:.2} [kHz] vs {:.2} [kHz], {:.5}, {}", freq / 1.0e3, best_freq / 1.0e3, (freq as f32)/best_freq, sds1202x.read_cymometer().unwrap());
	// assert!((1.0 - (freq as f32)/best_freq).abs() < 0.06);

	// // Destroy links
	// sdg2042x.destroy_link()?;
	// spd3303x.destroy_link()?;
	
	Ok(())
}
