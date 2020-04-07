
extern crate byteorder;
extern crate rand;
extern crate regex;
extern crate rustfft;
extern crate vxi11;

use std::{str, thread};
use std::io::{self, Cursor};
use std::time::Duration;

use byteorder::ReadBytesExt;
use lazy_static::lazy_static;
use rand::distributions::{IndependentSample, Range};
use regex::Regex;
use rustfft::FFTplanner;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

use vxi11::rpc::port_mapping::UdpPortMapperClient;

use vxi11::devices::sds1202x::SDS1202X;

lazy_static! {
    static ref TDIV_RE: Regex = Regex::new("TDIV\\s([^S]+)S").unwrap();
    static ref SARA_RE: Regex = Regex::new("SARA\\s(\\d+)(\\D)Sa/s").unwrap();
}

pub fn main() -> io::Result<()> {

	// TODO: search for IP addresses instead of needing them provided
	let host_sds1202x = "192.168.2.4";
	let host_sdg2042x = "192.168.2.3";
	let host_spd3303x = "192.168.2.2";

	let mut sds1202x = SDS1202X::new(host_sds1202x)?;
	let mut sdg2042x = vxi11::vxi11::CoreClient::new(host_sdg2042x)?;
	let mut spd3303x = vxi11::vxi11::CoreClient::new(host_spd3303x)?;

	println!("{:?}", sds1202x.get_full_state());

	sdg2042x.create_link()?;
	spd3303x.create_link()?;

    assert!(str::from_utf8(&(sdg2042x.ask(b"*IDN?")?)).unwrap().contains("SDG2042X"));
    assert!(str::from_utf8(&(spd3303x.ask(b"*IDN?")?)).unwrap().contains("SPD3303X"));

    for (host, name) in vec![(host_sds1202x, "SDS1202X"), (host_spd3303x, "SPD3303X"), (host_sdg2042x, "SDG2042X")] {
		let mut udp_port_mapper = UdpPortMapperClient::new(host).unwrap();
		match udp_port_mapper.dump() {
			Ok(mapping_dump) => {
				println!("\nPort mapping for {} at {}", name, host);

				for mapping in mapping_dump {
					println!("{:?}", mapping);
				}
			},
			Err(_) => println!("\n{} at {} not allowing a UDP connection for a port mapping dump", name, host)
		}
    }

    // TODO: Make variables at the top for the various durations
    println!("\nTesting SPD3303X");
	spd3303x.ask(b"OUTP:TRACK 0")?;
	spd3303x.ask(b"INST CH2")?;
	spd3303x.ask(b"CH2:VOLT 3")?;
	thread::sleep(Duration::from_secs_f32(0.5));

	spd3303x.ask(b"OUTP CH2,ON")?;
	thread::sleep(Duration::from_secs_f32(3.0));
	spd3303x.ask(b"OUTP CH2,OFF")?;

	let freq:f64 = {
		let between = Range::new(100f64, 1e6);
		let mut rng = rand::thread_rng();
		between.ind_sample(&mut rng)
	};

	println!("\nTesting SDG2042X and SDS1202X together, freq={:.1} [kHz]", (freq / 1000.0));

	// Set up waveform generator
	let waveform_setup_cmd:String = format!("C1:BSWV WVTP,SINE,FRQ,{:.3},AMP,4V", freq);
	sdg2042x.ask(waveform_setup_cmd.as_bytes())?;

	thread::sleep(Duration::from_secs_f32(0.5));
	sdg2042x.ask(b"C1:OUTP ON")?;
	
	// Set up oscilloscope
	sds1202x.set_voltage_div(1, 1.0)?;                            // Voltage division
	sds1202x.ask(b"WFSU SP,0,NP,0,FP,0")?;                         // Send all data points starting with the first one

	let tdiv_cmd:String = format!("TDIV {:.7}S", 3.0*freq.powi(-1));
	sds1202x.ask(tdiv_cmd.as_bytes())?; 	                       // Time division

	let actual_tdiv_str:String = str::from_utf8(&sds1202x.ask(b"TDIV?")?).unwrap().to_string();
	let actual_tdiv:f32 = TDIV_RE.captures(&actual_tdiv_str).unwrap().get(1).unwrap().as_str().parse::<f32>().unwrap();

	// Trigger once before the real thing to get an accurate sample rate
	sds1202x.ask(b"TRMD STOP")?;
	sds1202x.ask(b"TRMD SINGLE;ARM;FRTR")?;

    while !str::from_utf8(&(sds1202x.ask(b"SAST?")?)).unwrap().contains("SAST Stop") {
		thread::sleep(Duration::from_secs_f32(0.5));
    }

	let samp_rate_str:String = str::from_utf8(&sds1202x.ask(b"SARA?")?).unwrap().to_string();
	let samp_rate_caps = SARA_RE.captures(&samp_rate_str).unwrap();
	let samp_rate_sps:f32 = match (samp_rate_caps.get(1).unwrap().as_str(), samp_rate_caps.get(2).unwrap().as_str()) {
		(x, "M") => x.parse::<f32>().unwrap() * 1e6,
		(x, "G") => x.parse::<f32>().unwrap() * 1e9,
		(x, suffix) => {
			panic!("x={}, suffix={}", x, suffix)
		}
	};
	
	// Capture the samples that count
	sds1202x.ask(b"TRMD STOP")?;
	sds1202x.ask(b"TRMD SINGLE;ARM;FRTR")?;

    while !str::from_utf8(&(sds1202x.ask(b"SAST?")?)).unwrap().contains("SAST Stop") {
		thread::sleep(Duration::from_secs_f32(0.5));
    }

	sdg2042x.ask(b"C1:OUTP OFF")?;

	// Retrieve and decode data
    let ch1_data:Vec<u8> = sds1202x.ask(b"C1:WAVEFORM? DAT2")?;
	
	// TODO: process the rest of the header
	let (header, body) = ch1_data.split_at(21);
	let (_, length_str) = header.split_at(12);

	let length:usize = str::from_utf8(length_str).unwrap().parse::<usize>().unwrap();

	// TODO: make these configurable and/or populate using requests from device
	let vdiv = 1.0;
	let vofs = 0.0;
	let mut time_domain: Vec<Complex<f64>> = vec![];

	// TODO: create some kind of sample struct for the SDS-1202X with time and voltage
	//let mut time = -7.0 * actual_tdiv;
	let mut rdr = Cursor::new(body);
	for _ in 0..length {
		let raw_i8:i8 = rdr.read_i8()?;
		//time += 1.0 / samp_rate_sps;
		let voltage:f64 = (raw_i8 as f64)*(vdiv/25.0) - vofs;
		time_domain.push(Complex{ re: voltage, im: 0.0});
	}

    // Perform FFT
	let mut freq_domain: Vec<Complex<f64>> = vec![Complex::zero(); length];
	let mut planner = FFTplanner::new(false);
	let fft = planner.plan_fft(length);
	fft.process(&mut time_domain, &mut freq_domain);

	// Find the strongest frequency
	let mut best_freq:f32 = 0.0;
	let mut best_amp:f64 = 0.0;
	for (idx, fft_response) in (&freq_domain).into_iter().enumerate() {

		if best_amp < fft_response.norm_sqr() {
			best_amp = fft_response.norm_sqr();
			best_freq = if idx < (length/2) {
				(idx as f32 * samp_rate_sps) / (length as f32)
			} else {
				((length - idx) as f32 * samp_rate_sps) / (length as f32)
			};
		}
	}

	println!("{:.2} [kHz] vs {:.2} [kHz], {:.5}", freq / 1.0e3, best_freq / 1.0e3, (freq as f32)/best_freq);
	assert!((1.0 - (freq as f32)/best_freq).abs() < 0.06);

	// Destroy links
	sdg2042x.destroy_link()?;
	spd3303x.destroy_link()?;
	
	Ok(())
}
