
extern crate rppal;
extern crate serde;
extern crate vxi11;

use std::io::{self, Error, ErrorKind};
use std::thread;
use std::time::{Instant, Duration};

use rppal::gpio::{Gpio, Mode, Level};
use rppal::system::DeviceInfo;

use serde::{Serialize, Deserialize};

use vxi11::devices::sds1202x::{SDS1202X, TriggerMode};
use vxi11::devices::spd3303x::{SPD3303X};

pub const MAX_RECORDS:usize = 600;
const GPIO_RESET:u8 = 18;

#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerResult {
	since_last_reset_ms: u128,
	since_start_ms: u128,
	dt: f32,
}

fn reset_counters(gpio:&mut Gpio) {
    // Reset the counters by pulling the line high for 1 second
    gpio.write(GPIO_RESET, Level::High);
    thread::sleep(Duration::from_millis(1000));
    gpio.write(GPIO_RESET, Level::Low);
}

pub fn main() -> io::Result<()> {

    // Get GPIO interface for reset commands
	let device_info = match DeviceInfo::new() {
		Ok(dev) => dev,
		Err(_) => return Err(Error::new(ErrorKind::Other, "Unable to get device info, may not be running on Raspberry Pi"))
	};
	eprintln!("Running on {} with {}", device_info.model(), device_info.soc());

    let mut gpio = Gpio::new().unwrap();
    gpio.set_mode(GPIO_RESET, Mode::Output);

	// TODO: search for IP addresses instead of needing them provided
	let host_spd3303x = "192.168.2.2";
	let host_sds1202x = "192.168.2.3";

	let mut spd3303x:SPD3303X = SPD3303X::new(host_spd3303x)?;
	let mut sds1202x:SDS1202X = SDS1202X::new(host_sds1202x)?;

	// Set up DC power supply
	let spd3303x_initial_state = spd3303x.get_full_state()?;
	eprintln!("{}", serde_json::to_string_pretty(&spd3303x_initial_state)?);
	if spd3303x_initial_state.ch2.measured_current > 0.0 {
	    eprintln!("Expected no current in CH2 (cold start for GPSDOs) but measured current is {} [A]", spd3303x_initial_state.ch2.measured_current);
	    
	    // Turn of CH2 so it's ready for next time, then return an error
    	spd3303x.disable_output(2)?;
    	
    	return Err(Error::new(ErrorKind::Other, "Non-zero initial current on CH2"))
	}
	spd3303x.set_voltage(1, 3.30)?;
	spd3303x.set_voltage(2, 12.0)?;
	spd3303x.set_current(1, 0.5)?;
	spd3303x.set_current(2, 2.0)?;
	
	spd3303x.ask_str("OUTP:TRACK 0")?;  // Independent mode
	spd3303x.enable_output(1)?;
	spd3303x.enable_output(2)?;
	let start_time = Instant::now();    // The start time is when we first applied power to the GPSDOs

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
	reset_counters(&mut gpio);
	let mut last_reset_time = Instant::now();
	let mut ans:Vec<TriggerResult> = vec![];

	while ans.len() < MAX_RECORDS {
		// Trigger the samples
		sds1202x.set_trigger_mode(TriggerMode::Single)?;
		sds1202x.wait()?;
		let since_last_reset_ms = last_reset_time.elapsed().as_millis();
		let since_start_ms = start_time.elapsed().as_millis();

		// Retrieve the waveforms and actual sample rate
		let ch1:Vec<i8> = sds1202x.transfer_waveform_raw(1)?;
		let ch2:Vec<i8> = sds1202x.transfer_waveform_raw(2)?;
		let fs:f32 = sds1202x.get_sample_rate()?;

	    // Use the channel 2 midpoint for both because we're triggering off channel 2, so we know it has a transition
		let ch2_min:i8 = *(ch2.iter().min().ok_or(Error::new(ErrorKind::Other, "Unable to find min value of CH2"))?);
		let ch2_max:i8 = *(ch2.iter().max().ok_or(Error::new(ErrorKind::Other, "Unable to find max value of CH2"))?);
		let ch2_mid:i8 = ch2_min + ((ch2_max - ch2_min) / 2);	// This way avoids the overflow risk
		
		if ch1[0] > ch2_mid || *(ch1.last().unwrap()) < ch2_mid { 
		    // If channel 1 starts off higher than the midpoint or ends up lower, then reset the counters
	        reset_counters(&mut gpio);
	        last_reset_time = Instant::now();
		} else {
		    // Both transitions are in view
		    let (ch1_midx, _) = ch1.iter().enumerate().find(|(_, x)| x > &&ch2_mid).ok_or(Error::new(ErrorKind::Other, "Unable to find a crossing in CH1"))?;
		    let (ch2_midx, _) = ch2.iter().enumerate().find(|(_, x)| x > &&ch2_mid).ok_or(Error::new(ErrorKind::Other, "Unable to find a crossing in CH2"))?;

		    let dt:f32 = (ch2_midx as f32 - ch1_midx as f32) / fs;

		    if since_last_reset_ms > 30000 {
		        // If it's been at least 30 seconds since the last reset, output status information to STDERR
			    if let Some(last_result) = ans.last() {
			        let dt_inner:f32 = dt - last_result.dt;
			        let dcycles:f32 = dt_inner * 10.0e6;
			        eprintln!("{:.2} [min], dt={:.3e} [sec], CH1 cross at {:?}, CH2 cross at {:?}, {:.6} [Hz]", 
				        (since_start_ms as f32) / 6.0e4, dt, ch1_midx, ch2_midx, 
				        dcycles / (0.001*(since_last_reset_ms as f32 - last_result.since_last_reset_ms as f32)));
			    }
		    }

		    ans.push(TriggerResult{ since_last_reset_ms, since_start_ms, dt });
		}

	}

    // This is the only output to STDOUT so this can just be redirected to a JSON file in the shell
	println!("{}", serde_json::to_string_pretty(&ans).unwrap());

	// Clean up
	spd3303x.disable_output(2)?;

	Ok(())
}
