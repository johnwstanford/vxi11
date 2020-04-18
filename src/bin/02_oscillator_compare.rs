
extern crate rppal;
extern crate serde;
extern crate vxi11;

use std::io::{self, Error, ErrorKind};
use std::time::Instant;

// use rppal::gpio::{Gpio, Mode, Level};
// use rppal::system::DeviceInfo;

use serde::{Serialize, Deserialize};

use vxi11::devices::sds1202x::{SDS1202X, TriggerMode};
use vxi11::devices::spd3303x::{SPD3303X};

pub const MAX_RECORDS:usize = 600;

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerResult {
	t_ms: u128,
	dt: f32,
}

pub fn main() -> io::Result<()> {

	// let device_info = match DeviceInfo::new() {
	// 	Ok(dev) => dev,
	// 	Err(_) => return Err(Error::new(ErrorKind::Other, "Unable to get device info, may not be running on Raspberry Pi"))
	// };
	// eprintln!("Running on {} with {}", device_info.model(), device_info.soc());

	// TODO: search for IP addresses instead of needing them provided
	let host_spd3303x = "192.168.2.2";
	let host_sds1202x = "192.168.2.3";

	let mut spd3303x:SPD3303X = SPD3303X::new(host_spd3303x)?;
	let mut sds1202x:SDS1202X = SDS1202X::new(host_sds1202x)?;

	// Set up DC power supply
	let spd3303x_initial_state = spd3303x.get_full_state()?;
	eprintln!("{}", serde_json::to_string_pretty(&spd3303x_initial_state)?);
	// TODO: check that the voltage is initially off for channel 2 (cold start for GPSDOs)
	spd3303x.set_voltage(1, 3.30)?;
	spd3303x.set_voltage(2, 12.0)?;
	spd3303x.set_current(1, 0.5)?;
	spd3303x.set_current(2, 2.0)?;
	
	spd3303x.ask_str("OUTP:TRACK 0")?;
	spd3303x.enable_output(1)?;
	spd3303x.enable_output(2)?;

	// Set up oscilloscope
	sds1202x.set_time_division(2.0e-6)?;
	for ch in &[1,2] {
		sds1202x.set_voltage_div(*ch, 1.0)?;
		sds1202x.set_trace_display_enabled(*ch, true)?;
		sds1202x.set_voltage_ofs(*ch, 0.0)?;
	}
	// TODO: set up the trigger
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?; // Send all data points starting with the first one when requested

	// Reset the counters using GPIO
	let t0 = Instant::now();
	let mut ans:Vec<TriggerResult> = vec![];

	while ans.len() < MAX_RECORDS {
		// Trigger the samples
		sds1202x.set_trigger_mode(TriggerMode::Single)?;
		sds1202x.wait()?;
		let t_ms = t0.elapsed().as_millis();

		// Retrieve the waveforms
		let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
		let ch2:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;
		let fs:f32 = sds1202x.get_sample_rate()?;

		let ch2_min:i8 = *(ch2.iter().min().ok_or(Error::new(ErrorKind::Other, "Unable to find min value of CH2"))?);
		let ch2_max:i8 = *(ch2.iter().max().ok_or(Error::new(ErrorKind::Other, "Unable to find max value of CH2"))?);
		let ch2_mid:i8 = ch2_min + ((ch2_max - ch2_min) / 2);	// This way avoids the overflow risk
		
		// If channel 1 starts off higher than the midpoint or ends up lower, break out of this loop and reset the counters
		// TODO: when this runs on the RPi, leave the outer loop the same, but keep a Vec<TriggerResult> for every reset and when this
		// condition is met, commit the current Vec to a master list, command a reset, and start a new one
		if ch1[0] > ch2_mid || *(ch1.last().unwrap()) < ch2_mid { 
			break; 
		}

		// Use the channel 2 midpoint for both because we're triggering off channel 2, so we know it has a transition
		let (ch1_midx, _) = ch1.iter().enumerate().find(|(_, x)| x > &&ch2_mid).ok_or(Error::new(ErrorKind::Other, "Unable to find a crossing in CH1"))?;
		let (ch2_midx, _) = ch2.iter().enumerate().find(|(_, x)| x > &&ch2_mid).ok_or(Error::new(ErrorKind::Other, "Unable to find a crossing in CH2"))?;

		let dt:f32 = (ch2_midx as f32 - ch1_midx as f32) / fs;

		if ans.len() > 1 {
			let last_result = ans.last().unwrap();
			let dt_inner:f32 = dt - last_result.dt;
			let dcycles:f32 = dt_inner * 10.0e6;
			eprintln!("{:.2} [min], dt={:.3e} [sec], CH1 cross at {:?}, CH2 cross at {:?}, {:.6} [Hz]", 
				(t_ms as f32) / 6.0e4, dt, ch1_midx, ch2_midx, dcycles / (0.001*(t_ms as f32 - last_result.t_ms as f32)));
		}

		ans.push(TriggerResult{ t_ms, dt });
	}

	println!("{}", serde_json::to_string_pretty(&ans).unwrap());

	// Clean up
	spd3303x.disable_output(1)?;
	spd3303x.disable_output(2)?;

	Ok(())
}
