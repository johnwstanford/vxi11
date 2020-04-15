

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
    static ref BSWV_RE: Regex     = Regex::new("(C[12]):BSWV\\sWVTP,(SINE|SQUARE|RAMP|PULSE|NOISE|ARB|DC)").unwrap();
    static ref BSWV_DC_RE: Regex  = Regex::new("(C[12]):BSWV\\sWVTP,DC,OFST,([^V]+)V").unwrap();
    static ref BSWV_DEF_RE: Regex = Regex::new("(C[12]):BSWV\\sWVTP,(SINE|SQUARE|RAMP|PULSE|NOISE|ARB),FRQ,(\\d+)HZ,PERI,[^,]+,AMP,([^V]+)V,AMPVRMS,[^,]+,OFST,([^V]+)V,HLEV,[^,]+,LLEV,[^,]+,PHSE,([^,]+)").unwrap();
    //											(C[12]):BSWV\\sWVTP,(SINE|SQUARE|RAMP|PULSE|NOISE|ARB),FRQ,(\\d+)HZ,PERI,[^,]+,AMP,([^V]+)V,AMPVRMS,[^,]+,OFST,([^V]+)V,HLEV,2V,LLEV,-2V,PHSE,0\n
    static ref IDN_RE: Regex      = Regex::new("([^,]+),([^,]+),([^,]+),([^,\\s]+)").unwrap();
    static ref OUTP_RE: Regex     = Regex::new("C[12]:OUTP (ON|OFF),LOAD,([^,]+),PLRT,([^,]+)").unwrap();
}

pub const DEFAULT_TX_THROTTLE_DURATION_SEC:f32 = 0.1;

pub struct SDG2042X {
	core: CoreClient,
	tx_throttle_duration: Duration,
	pub state: Option<State>,
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

#[derive(Debug, Serialize, Deserialize)]
pub enum Wavetype {
	Sine,
	Square,
	Ramp,
	Pulse,
	Noise,
	Arb,
	DC,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelState {
	pub basic_wavetype: Wavetype,
	pub freq_hz:u32,
	pub amp_v:f32,
	pub offset_v:f32,
	pub phase_deg:f32,
}

fn match_str(opt_match:Option<Match>, err:&str) -> io::Result<String> {
	match opt_match {
		Some(m) => Ok(m.as_str().to_owned()),
		None    => Err(Error::new(ErrorKind::Other, err))
	}
}

pub fn chan_ok(n:u8) -> io::Result<()> {
	if n != 1 && n != 2 { Err(Error::new(ErrorKind::Other, "SDG2042X only has two channels")) }
	else { Ok(()) }		
}


impl SDG2042X {		

	pub fn new(host:&str) -> io::Result<Self> {
		let mut core = CoreClient::new(host)?;

		core.create_link()?;

		match str::from_utf8(&(core.ask(b"*IDN?")?)) {
			Ok(idn_resp) => {
				if idn_resp.contains("SDG2042X") { /* Do nothing because this is what we expected */ }
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

	pub fn get_channel_state(&mut self, chan_num:u8) -> io::Result<ChannelState> {
		chan_ok(chan_num)?;

		let bswv_cmd:String   = format!("C{}:BSWV?", chan_num);
		let bswv_res:String   = self.ask_str(&bswv_cmd)?;
		let bswv_cap:Captures = match BSWV_RE.captures(&bswv_res) {
			Some(c) => c,
			_ => return Err(Error::new(ErrorKind::Other, "Unable to match expression for basic wavetype")),
		};

		// TODO: check that capture 1 represents the same channel we asked for
		match (match_str(bswv_cap.get(2), "No match for basic wavetype")?).as_str() {
			"DC" => {
				let cap:Captures = match BSWV_DC_RE.captures(&bswv_res) {
					Some(c) => c,
					_ => return Err(Error::new(ErrorKind::Other, "Unable to match expression for DC wavetype")),
				};

				let offset_v:f32 = (match_str(cap.get(2), "No match for offset_v")?).parse::<f32>()
					.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse matched offset_v as an f32"))?;

				Ok(ChannelState{ basic_wavetype: Wavetype::DC, freq_hz:0, amp_v:0.0, offset_v, phase_deg:0.0 })
			},
			s => {
				let cap:Captures = match BSWV_DEF_RE.captures(&bswv_res) {
					Some(c) => c,
					_ => return Err(Error::new(ErrorKind::Other, "Unable to match expression for default wavetype")),
				};

				let basic_wavetype:Wavetype = match s {
					"SINE"   => Wavetype::Sine,
					"SQUARE" => Wavetype::Square,
					"RAMP"   => Wavetype::Ramp,
					"PULSE"  => Wavetype::Pulse,
					"NOISE"  => Wavetype::Noise,
					"ARB"    => Wavetype::Arb,
					_        => return Err(Error::new(ErrorKind::Other, "Unrecognized basic wavetype")),	
				};

				let freq_hz:u32 = (match_str(cap.get(3), "No match for freq_hz")?).parse::<u32>()
					.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse matched freq_hz as a u32"))?;
				let amp_v:f32 = (match_str(cap.get(4), "No match for amp_v")?).parse::<f32>()
					.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse matched amp_v as an f32"))?;
				let offset_v:f32 = (match_str(cap.get(5), "No match for offset_v")?).parse::<f32>()
					.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse matched offset_v as an f32"))?;
				let phase_deg:f32 = (match_str(cap.get(6), "No match for phase_deg")?).trim_end().parse::<f32>()
					.map_err(|_| Error::new(ErrorKind::Other, "Unable to parse matched phase_deg as an f32"))?;

				Ok(ChannelState{ basic_wavetype, freq_hz, amp_v, offset_v, phase_deg })
			}
		}
	}

	pub fn set_output(&mut self, chan_num:u8, on:bool) -> io::Result<()> {
		chan_ok(chan_num)?;

		if self.get_output(chan_num)? == on {
			// Already in the commanded state, so don't do anything
		} else {
			let outp_cmd:String   = format!("C{}:OUTP {}", chan_num, if on {"ON"} else {"OFF"});
			self.ask_str(&outp_cmd)?;
		}

		Ok(())
	}

	pub fn get_output(&mut self, chan_num:u8) -> io::Result<bool> {
		chan_ok(chan_num)?;

		let cmd:String = format!("C{}:OUTP?", chan_num);
		let res:String = self.ask_str(&cmd)?;

		// TODO: parse impedance and polarity
		let cap:Captures = OUTP_RE.captures(&res).unwrap();
		let on:bool = cap.get(1).map(|m| m.as_str()) == Some("ON");

		Ok(on)
	}

	pub fn set_basic_wavetype(&mut self, chan_num:u8, wvtp:Wavetype, freq_hz:u32, amp_v:f32, offset_v:f32, phase_deg:f32) -> io::Result<()> {
		chan_ok(chan_num)?;

		let wvtp_str:&str = match wvtp {
			Wavetype::Sine   => "SINE",
			Wavetype::Square => "SQUARE",
			Wavetype::Ramp   => "RAMP",
			Wavetype::Pulse  => "PULSE",
			Wavetype::Noise  => "NOISE",
			Wavetype::Arb    => "ARB",
			Wavetype::DC     => "DC"
		};

		// TODO: figure out exactly how many digits matter in the f32 formatting
		let cmd:String  = format!("C{}:BSWV WVTP,{},FRQ,{},AMP,{:.6}V,OFST,{:.6}V,PHSE,{:.6}", chan_num, wvtp_str, freq_hz, amp_v, offset_v, phase_deg);

		self.ask_str(&cmd)?;
		Ok(())
	}

	pub fn opc(&mut self) -> io::Result<bool> {
		Ok((self.ask_str("*OPC?")?).trim_end() == "1")
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

impl Drop for SDG2042X {

	fn drop(&mut self) { self.core.destroy_link().expect("Unable to destroy link for SDG2042X"); }

}

// Not Yet Implemented
// *CLS *CLS SYSTEM Clears all the status data registers.
// *ESE *ESE SYSTEM Sets or gets the Standard Event Status Enable register (ESE).
// *ESR *ESR SYSTEM Reads and clears the contents of the Event Status Register (ESR).
// *RST *RST SYSTEM Initiates a device reset.
// *SRE *SRE SYSTEM Sets the Service Request Enable register (SRE).
// *STB *STB SYSTEM Gets the contents of the IEEE 488.2 defined status register.
// *TST *TST SYSTEM Performs an internal self-test.
// *WAI *WAI SYSTEM Wait to continue command.
// DDR DDR SYSTEM Reads and clears the Device Dependent Register (DDR).
// CMR CMR SYSTEM Reads and clears the command error register.
// CHDR COMM_HEADER SIGNAL Sets or gets the command returned format
// OUTP OUTPUT SIGNAL Sets or gets output state.
// MDWV MODULATEWAVE SIGNAL Sets or gets modulation parameters.
// SWWV SWEEPWAVE SIGNAL Sets or gets sweep parameters.
// BTWV BURSTWAVE SIGNAL Sets or gets burst parameters.
// PACP PARACOPY SIGNAL Copies parameters from one channel to the other.
// ARWV ARBWAVE DATA Changes arbitrary wave type.
// SYNC SYNC SIGNAL Sets or gets synchronization signal.
// NBFM NUMBER_FORMAT SYSTEM Sets or gets data format.
// LAGG LANGUAGE SYSTEM Sets or gets language.
// SCFG SYS_CFG SYSTEM Sets or gets the power-on system setting way.
// BUZZ BUZZER SYSTEM Sets or gets buzzer state.
// SCSV SCREEN_SAVE SYSTEM Sets or gets screen save state.
// ROSC ROSCILLATOR SIGNAL Sets or gets state of clock source.
// FCNT FREQCOUNTER SIGNAL Sets or gets frequency counter parameters.
// INVT INVERT SIGNAL Sets or gets polarity of current channel.
// COUP COUPLING SIGNAL Sets or gets coupling parameters.
// VOLTPRT VOLTPRT SYSTEM Sets or gets protection.
// STL STORELIST SIGNAL Lists all stored waveforms.
// WVDT WVDT SIGNAL Sets and gets arbitrary wave data.
// VKEY VIRTUALKEY SYSTEM Sets the virtual keys.
// SYST:COMM:LAN:IPAD SYSTEM The Command can set and get system IP address.
// SYST:COMM:LAN:SMAS SYSTEM The Command can set and get system subnet mask.
// SYST:COMM:LAN:GAT  SYSTEM The Command can set and get system Gateway.
// SRATE 			  SAMPLERATE Sets or gets sampling rate. You can only use it in TrueArb mode
// HARM HARMonic SIGNAL Sets or gets harmonic information.
// CMBN CoMBiNe SIGNAL Sets or gets wave combine information.

// Partially implemented
// BSWV 	BASIC_WAVE 	SIGNAL 		Sets or gets basic wave parameters.

// Implemented
// *IDN 	*IDN 		SYSTEM 		Gets identification from device.
// *OPC 	*OPC 		SYSTEM 		Gets or sets the OPC bit (0) in the Event Status Register (ESR).
