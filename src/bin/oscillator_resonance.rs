
extern crate rustfft;
extern crate serde;
extern crate vxi11;

use std::io;
use std::thread;
use std::time::Duration;

use rustfft::FFTplanner;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

use vxi11::devices::sds1202x::SDS1202X;
use vxi11::devices::sdg2042x::{SDG2042X, Wavetype};

pub fn main() -> io::Result<()> {

	// TODO: search for IP addresses instead of needing them provided
	let host_sds1202x = "192.168.2.2";
	let host_sdg2042x = "192.168.2.3";

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
	sds1202x.set_voltage_div(2, 1.0)?;                            
	sds1202x.set_voltage_ofs(1, 0.0)?;							  // Voltage offset
	sds1202x.set_voltage_ofs(1, 0.0)?;
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                        // Send all data points starting with the first one

	// Step through frequencies
	let mut current_freq_hz:f32 = min_freq_hz;
	while current_freq_hz < max_freq_hz {

		// Set up waveform generator
		sdg2042x.set_basic_wavetype(1, Wavetype::Sine, current_freq_hz as u32, amp_v, 0.0, 0.0)?;
		sdg2042x.set_output(1, true)?;
		thread::sleep(Duration::new(2,0));

		// Set up oscilloscope
		sds1202x.set_time_division(1.0 / current_freq_hz)?;

		// Capture the samples that count
		sds1202x.arm_single()?;
		sds1202x.force_trigger()?;
		sds1202x.wait()?;

		// let samp_rate_sps:f32 = sds1202x.get_sample_rate()?;

		let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
		let ch2:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;
		let product:Vec<i16> = ch1.iter().zip(ch2.iter()).map(|(a,b)| (*a as i16)*(*b as i16)).collect();

		let mut ch1_cpx:Vec<Complex<f32>> = ch1.iter().map(|x| Complex{re: *x as f32, im: 0.0}).collect();
		let mut ch2_cpx:Vec<Complex<f32>> = ch2.iter().map(|x| Complex{re: *x as f32, im: 0.0}).collect();
		let mut product_cpx:Vec<Complex<f32>> = product.iter().map(|x| Complex{re: *x as f32, im: 0.0}).collect();

		// FFTs of original channels and product
		let mut ch1_freq_domain:Vec<Complex<f32>>     = vec![Complex::zero(); ch1_cpx.len()];
		let mut ch2_freq_domain:Vec<Complex<f32>>     = vec![Complex::zero(); ch2_cpx.len()];
		let mut product_freq_domain:Vec<Complex<f32>> = vec![Complex::zero(); product_cpx.len()];

		let mut planner = FFTplanner::new(false);
		let fft = planner.plan_fft(product.len());

		fft.process(&mut ch1_cpx,     &mut ch1_freq_domain);
		fft.process(&mut ch2_cpx,     &mut ch2_freq_domain);
		fft.process(&mut product_cpx, &mut product_freq_domain);

		let chn1_norms:Vec<f32> = ch1_freq_domain.iter().map(|c| c.norm()).collect();
		let chn2_norms:Vec<f32> = ch2_freq_domain.iter().map(|c| c.norm()).collect();
		let prod_norms:Vec<f32> = product_freq_domain.iter().map(|c| c.norm()).collect();
		println!("chn1={:?}", chn1_norms);
		println!("chn2={:?}", chn2_norms);
		println!("prod={:?}", prod_norms);

		// Increment the frequency for the next step
		current_freq_hz += freq_step_hz;

	}
	sdg2042x.set_output(1, false)?;

	

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
