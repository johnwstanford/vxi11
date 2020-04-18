

extern crate regex;
extern crate serde;

use std::io::{self, Error, ErrorKind};
use std::ops::Drop;
use std::str;
use std::thread;
use std::time::Duration;

use regex::{Captures, Match, Regex};
use serde::{Serialize, Deserialize};

use crate::vxi11::CoreClient;

lazy_static! {
    static ref IDN_RE: Regex      = Regex::new("([^,]+),([^,]+),([^,]+),([^,\\s]+)").unwrap();
}

pub const DEFAULT_TX_THROTTLE_DURATION_SEC:f32 = 0.1;

pub struct SPD3303X {
	core: CoreClient,
	tx_throttle_duration: Duration,
	pub state: Option<State>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelState {
	voltage: f32
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
	pub manufacturer: String,
	pub model: String,
	pub serial_num: String,
	pub fw_version: String,
	pub ch1: ChannelState,
	pub ch2: ChannelState,
}

fn match_str(opt_match:Option<Match>, err:&str) -> io::Result<String> {
	match opt_match {
		Some(m) => Ok(m.as_str().to_owned()),
		None    => Err(Error::new(ErrorKind::Other, err))
	}
}

fn err(msg:&str) -> io::Error { Error::new(ErrorKind::Other, msg) }

pub fn chan_ok(n:u8) -> io::Result<()> {
	if n != 1 && n != 2 { Err(Error::new(ErrorKind::Other, "SDG2042X only has two channels")) }
	else { Ok(()) }		
}


impl SPD3303X {		

	pub fn new(host:&str) -> io::Result<Self> {
		let mut core = CoreClient::new(host)?;

		core.create_link()?;

		match str::from_utf8(&(core.ask(b"*IDN?")?)) {
			Ok(idn_resp) => {
				if idn_resp.contains("SPD3303X") { /* Do nothing because this is what we expected */ }
				else { return Err(Error::new(ErrorKind::Other, "Successfully connected to a device but it doesn't appear to be the right model")); }
			},
			Err(_) => return Err(Error::new(ErrorKind::Other, "Received a response to *IDN? but unable to interpret as UTF-8")),
		}

		// TODO: make this configurable
		let tx_throttle_duration = Duration::from_secs_f32(DEFAULT_TX_THROTTLE_DURATION_SEC);

		Ok(Self{ core, tx_throttle_duration, state: None })
	}

	pub fn get_full_state(&mut self) -> io::Result<State> {
	    let str_idn:String      = str::from_utf8(&self.core.ask(b"*IDN?")?).map(|s| s.to_owned()).unwrap();
		let caps_idn:Captures   = IDN_RE.captures(&str_idn).unwrap();
		let manufacturer:String = match_str(caps_idn.get(1), "No match for manufacturer")?;
		let model:String        = match_str(caps_idn.get(2), "No match for model")?;
		let serial_num:String   = match_str(caps_idn.get(3), "No match for serial_num")?;
		let fw_version:String   = match_str(caps_idn.get(4), "No match for fw_version")?;

		let ch1 = self.get_channel_state(1)?;
		let ch2 = self.get_channel_state(2)?;

		Ok(State{ manufacturer, model, serial_num, fw_version, ch1, ch2 })
	}

	pub fn get_channel_state(&mut self, ch:u8) -> io::Result<ChannelState> {
		chan_ok(ch)?;

		let voltage:f32 = self.get_voltage(ch)?;

		Ok(ChannelState{ voltage })		
	}

	pub fn get_voltage(&mut self, ch:u8) -> io::Result<f32> {
		chan_ok(ch)?;

	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
	    // TODO: remove all unwraps
		let cmd:String   = format!("CH{}:VOLT?", ch);
	    let res:String   = self.ask_str(&cmd)?;
		Ok(res.trim().parse::<f32>().map_err(|_| err("Unable to parse voltage response as a float"))?)
	}

	pub fn ask(&mut self, data:&[u8]) -> io::Result<Vec<u8>> { 
		thread::sleep(self.tx_throttle_duration);
		self.core.ask(data) 
	}

	pub fn ask_str(&mut self, data:&str) -> io::Result<String> {
		str::from_utf8(&self.core.ask(data.as_bytes())?)
			.map(|s| s.to_owned())
			.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse response as UTF-8"))
	}

}

impl Drop for SPD3303X {

	fn drop(&mut self) { self.core.destroy_link().expect("Unable to destroy link for SPD3303X"); }

}

// Not Yet Implemented

// Partially implemented

// Implemented
// *IDN 	*IDN 		SYSTEM 		Gets identification from device.
