
extern crate byteorder;
extern crate regex;

use std::io::{self, Error, ErrorKind, Cursor};
use std::ops::Drop;
use std::str;
use std::thread;
use std::time::Duration;

use byteorder::ReadBytesExt;
use regex::{Captures, Match, Regex};
use serde::{Serialize, Deserialize};

use crate::vxi11::CoreClient;

lazy_static! {
    static ref IDN_RE: Regex  = Regex::new("([^,]+),([^,]+),([^,]+),([^,\\s]+)").unwrap();
    static ref OFST_RE: Regex = Regex::new("C(\\d):OFST\\s(.+)V\\s").unwrap();
    static ref SARA_RE: Regex = Regex::new("SARA\\s(\\d+)(\\D)Sa/s").unwrap();
    static ref TDIV_RE: Regex = Regex::new("TDIV\\s([^S]+)S").unwrap();
    static ref TRA_RE: Regex  = Regex::new("C(\\d):TRA\\s(ON|OFF)").unwrap();
    static ref TRMD_RE: Regex = Regex::new("TRMD\\s(AUTO|NORM|SINGLE|STOP)").unwrap();
    static ref VDIV_RE: Regex = Regex::new("C(\\d):VDIV\\s(.+)V\\s").unwrap();
}

pub const DEFAULT_SHORT_DURATION_SEC:f32 = 0.01;
pub const DEFAULT_TX_THROTTLE_DURATION_SEC:f32 = 0.1;

pub mod protocol_decode;

pub struct SDS1202X {
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
	pub time_division: f32,
	pub trigger_mode: TriggerMode,
	pub ch1: ChannelState,
	pub ch2: ChannelState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelState {
	pub voltage_division: f32,
	pub voltage_offset: f32,
	pub trace_display_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TriggerMode { Auto, Norm, Single, Stop }

fn match_str(opt_match:Option<Match>, err:&str) -> io::Result<String> {
	match opt_match {
		Some(m) => Ok(m.as_str().to_owned()),
		None    => Err(Error::new(ErrorKind::Other, err))
	}
}

fn err(msg:&str) -> io::Error { Error::new(ErrorKind::Other, msg) }

fn chan_ok(n:u8) -> io::Result<()> {
	if n != 1 && n != 2 { Err(err("SDS1202X only has two channels")) }
	else { Ok(()) }		
}


impl SDS1202X {		

	pub fn new(host:&str) -> io::Result<Self> {
		let mut core = CoreClient::new(host)?;

		core.create_link()?;

		match str::from_utf8(&(core.ask(b"*IDN?")?)) {
			Ok(idn_resp) => {
				if idn_resp.contains("SDS1202X") { /* Do nothing because this is what we expected */ }
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

	    let time_division:f32 = self.get_time_division()?;
	    let trigger_mode:TriggerMode = self.get_trigger_mode()?;

		let ch1 = self.get_channel_state(1)?;
		let ch2 = self.get_channel_state(2)?;

		Ok(State{ manufacturer, model, serial_num, fw_version, time_division, trigger_mode, ch1, ch2 })
	}

	pub fn get_channel_state(&mut self, chan_num:u8) -> io::Result<ChannelState> {
		chan_ok(chan_num)?;

	    let voltage_division:f32 = self.get_voltage_div(chan_num)?;
	    let voltage_offset:f32 = self.get_voltage_ofs(chan_num)?;
	    let trace_display_enabled:bool = self.get_trace_display_enabled(chan_num)?;

		Ok(ChannelState{ voltage_division, voltage_offset, trace_display_enabled })
	}

	pub fn get_time_division(&mut self) -> io::Result<f32> {
	    let res:String   = self.ask_str("TDIV?")?;
	    let cap:Captures = TDIV_RE.captures(&res).unwrap();
    	(match_str(cap.get(1), "No match for time_division")?).parse::<f32>().map_err(|_| Error::new(ErrorKind::Other, "Unable to parse time division into f32"))
	}

	pub fn set_time_division(&mut self, tdiv:f32) -> io::Result<()> {
		// The fine scale of voltage division is 10 [mV] so 2 decimal places is all we need
		let cmd:String = format!("TDIV {:.7}S", tdiv);
	    self.ask_str(&cmd).map(|_| ())
	}

	pub fn get_sample_rate(&mut self) -> io::Result<f32> {
		let res:String   = self.ask_str("SARA?")?;
		let cap:Captures = SARA_RE.captures(&res).unwrap();
		let samp_rate_sps:f32 = match (cap.get(1).unwrap().as_str(), cap.get(2).unwrap().as_str()) {
			(x, "k") => x.parse::<f32>().unwrap() * 1e3,
			(x, "M") => x.parse::<f32>().unwrap() * 1e6,
			(x, "G") => x.parse::<f32>().unwrap() * 1e9,
			(_, _)   => return Err(err("Unrecognized suffix in sample rate response"))
		};

		Ok(samp_rate_sps)
	}

	pub fn get_trigger_mode(&mut self) -> io::Result<TriggerMode> {
	    let res:String   = self.ask_str("TRMD?")?;
	    let cap:Captures = TRMD_RE.captures(&res).unwrap();
    	let ans:TriggerMode = match (match_str(cap.get(1), "No match for time_division")?).as_str() {
    		"AUTO"   => TriggerMode::Auto,
    		"NORM"   => TriggerMode::Norm,
    		"SINGLE" => TriggerMode::Single,
    		"STOP"   => TriggerMode::Stop,
    		_        => return Err(err("Invalid value for trigger mode"))
    	};
    	
    	Ok(ans)
	}

	pub fn set_trigger_mode(&mut self, trmd:TriggerMode) -> io::Result<()> {
		let trmd_str = match trmd {
    		TriggerMode::Auto   => "AUTO",
    		TriggerMode::Norm   => "NORM",
    		TriggerMode::Single => "SINGLE",
    		TriggerMode::Stop   => "STOP"
		};
		self.ask_str(&format!("TRMD {}", &trmd_str))?;

		Ok(())
	}

	pub fn arm_single(&mut self) -> io::Result<()> {
		self.set_trigger_mode(TriggerMode::Single)?;
		self.arm()
	}

	// One-liners
	pub fn arm(&mut self)            -> io::Result<()>     { self.ask_str("ARM").map(|_| ())  }
	pub fn force_trigger(&mut self)  -> io::Result<()>     { self.ask_str("FRTR").map(|_| ()) }
	pub fn read_cymometer(&mut self) -> io::Result<String> { self.ask_str("CYMT?")            } // TODO: decode to a float


	pub fn wait(&mut self) -> io::Result<()> {
		let t = Duration::from_secs_f32(DEFAULT_SHORT_DURATION_SEC);
	    while !self.ask_str("SAST?")?.contains("SAST Stop") { 
	    	thread::sleep(t); 
	    }		

	    Ok(())
	}

	pub fn transfer_waveform_raw(&mut self, chan_num:u8) -> io::Result<Vec<i8>> {

		chan_ok(chan_num)?;

		// TODO: get the rest of the data not stored in DAT2 (small amount in the cases I've checked)
	    // let ch_all:Vec<u8> = self.ask(b"C1:WAVEFORM? ALL")?;
	    // println!("ch_all={:?}", ch_all);

	    let cmd:String = format!("C{}:WAVEFORM? DAT2", chan_num);
	    let ch_dat2:Vec<u8> = self.ask(&(cmd.as_bytes()))?;

		// TODO: process the rest of the header
		let (header, body) = ch_dat2.split_at(21);
		let (_, length_str) = header.split_at(12);

		// // Retrieve and decode data
		let length:usize = str::from_utf8(length_str).unwrap().parse::<usize>().unwrap();

		let mut rdr = Cursor::new(body);
		let mut ans:Vec<i8> = vec![];
		for _ in 0..length {
			ans.push(rdr.read_i8()?);
		}

		Ok(ans)
	}

	pub fn get_voltage_div(&mut self, chan_num:u8) -> io::Result<f32> {
		chan_ok(chan_num)?;

	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
	    // TODO: remove all unwraps
		let cmd:String   = format!("C{}:VDIV?", chan_num);
	    let res:String   = self.ask_str(&cmd)?;
		let cap:Captures = VDIV_RE.captures(&res).unwrap();
    	
		Ok((match_str(cap.get(2), "No match for voltage_division")?).parse::<f32>().unwrap())

	}

	pub fn get_voltage_ofs(&mut self, chan_num:u8) -> io::Result<f32> {
		chan_ok(chan_num)?;

	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
		let cmd:String   = format!("C{}:OFST?", chan_num);
	    let res:String   = self.ask_str(&cmd)?;
	    let cap:Captures = OFST_RE.captures(&res).unwrap();
    	
		Ok((match_str(cap.get(2), "No match for voltage_offset")?).parse::<f32>().unwrap())

	}

	pub fn get_trace_display_enabled(&mut self, chan_num:u8) -> io::Result<bool> {
		chan_ok(chan_num)?;

	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
		let cmd:String   = format!("C{}:TRA?", chan_num);
	    let res:String   = self.ask_str(&cmd)?;
    	let cap:Captures = match TRA_RE.captures(&res) {
    		Some(c) => c,
    		None    => return Err(err("No match for TRA_RE"))
    	};

		Ok(cap.get(2).map(|m| m.as_str()) == Some("ON"))
	}

	pub fn transfer_waveform(&mut self, chan_num:u8) -> io::Result<Vec<(f32, f32)>> {
		let raw_data:Vec<i8> = self.transfer_waveform_raw(chan_num)?;

		let vdiv = self.get_voltage_div(chan_num)?;
		let vofs = self.get_voltage_ofs(chan_num)?;
		let sps = self.get_sample_rate()?;

		let mut time:f32 = 0.0;
		let mut time_domain: Vec<(f32, f32)> = vec![];
		for raw in raw_data {
			let voltage:f32 = (raw as f32)*(vdiv/25.0) - vofs;
			time_domain.push((time, voltage));
			time += 1.0 / sps;
		}

		Ok(time_domain)
	}

	pub fn set_trace_display_enabled(&mut self, chan_num:u8, b:bool) -> io::Result<()> {
		// TODO add options for whether to enable a full, partial, or no state update after commanding a configuration change
		chan_ok(chan_num)?;

		// The fine scale of voltage division is 10 [mV] so 2 decimal places is all we need
		let cmd:String  = format!("C{}:TRA {}", chan_num, if b {"ON"} else {"OFF"});
	    self.ask_str(&cmd).map(|_| ())
	}

	pub fn set_voltage_div(&mut self, chan_num:u8, vdiv:f32) -> io::Result<()> {
		// TODO add options for whether to enable a full, partial, or no state update after commanding a configuration change
		chan_ok(chan_num)?;

		// The fine scale of voltage division is 10 [mV] so 2 decimal places is all we need
		let cmd:String  = format!("C{}:VDIV {:.2}", chan_num, vdiv);
	    self.ask_str(&cmd).map(|_| ())
	}

	pub fn set_voltage_ofs(&mut self, chan_num:u8, vofs:f32) -> io::Result<()> {
		chan_ok(chan_num)?;

		// TODO: Figure out how many decimal places actually matter
		let cmd:String = format!("C{}:OFST {:.6}", chan_num, vofs);
	    self.ask_str(&cmd).map(|_| ())
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

impl Drop for SDS1202X {

	fn drop(&mut self) { self.core.destroy_link().expect("Unable to destroy link for SDS1202X"); }

}

// Not Yet Implemented
// ALST?	ALL_STATUS?			STATUS
// ATTN	ATTENUATION			ACQUISITION
// ACAL	AUTO_CALIBRATE		MISCELLANEOUS
// AUTTS	AUTO_TYPESET		ACQUISITION
// AVGA	AVERAGE_ACQUIRE		ACQUISITION
// BWL	BANDWIDTH_LIMIT		ACQUISITION
// *CAL?	*CAL?				MISCELLANEOUS
// CHDR	COMM_HEADER			COMMUNICATION
// *CLS	*CLS				STATUS
// CMR?	CMR?				STATUS
// CONET	COMM_NET			COMMUNICATION
// CPL		COUPLING			ACQUISITION
// CRMS	CURSOR_MEASURE		CURSOR
// CRST?	CURSOR_SET?			CURSOR
// CRVA?	CURSOR_VALUE?		CURSOR
// CRAU	CURSOR_AUTO			CURSOR
// CSVS	CSV_SAVE			SAVE/RECALL
// COUN	COUNTER				FUNCTION
// DATE	DATE				MISCELLANEOUS
// DDR?	DDR?				STATUS
// DEF	DEFINE?				FUNCTION
// DELF	DELETE_FILE			MASS STORAGE
// DIR	DIRECTORY			MASS STORAGE
// DTJN	DOT_JOIN			DISPLAY
// *ESE	*ESE				STATUS
// *ESR?	*ESR?				STATUS
// EXR?	EXR?				STATUS
// FLNM	FILENAME			MASS STORAGE
// FVDISK	FORMAT_VDISK		MASS STORAGE
// FILT	FILTER				FUNCTION
// FILTS	FILT_SET			FUNCTION
// FFTW	FFT_WINDOW			FUNCTION
// FFTZ	FFT_ZOOM			FUNCTION
// FFTS	FFT_SCALE			FUNCTION
// FFTF	FFT_FULLSCREEN		FUNCTION
// GRDS	GRID_DISPLAY		DISPLAY
// GCSV	GET_CSV				WAVEFORMTRANS
// HMAG	HOR_MAGNIFY			DISPLAY
// HPOS	HOR_POSITION		DISPLAY
// HCSU	HARDCOPY_SETUP		HARD COPY
// INTS	INTENSITY			DISPLAY
// ILVD	INTERLEAVED			ACQUISITION
// INR?	INR?				STATUS
// INVS	INVERT_SET			DISPLAY
// LOCK	LOCK				MISCELLANEOUS
// MENU	MENU				DISPLAY
// MTVP	MATH_VERT_POS		ACQUISITION
// MTVD	MATH_VERT_DIV		ACQUISITION
// MEAD	MEASURE_DELY		FUNCTION
// *OPC	*OPC				STATUS
// *OPT?	*OPT?				MISCELLANEOUS
// PACL	PARAMETER_CLR		CURSOR
// PACU	PARAMETER_CUSTOM	CURSOR
// PAVA?	PARAMETER_VALUE?	CURSOR
// PDET	PEAK_DETECT			ACQUISITION
// PERS	PERSIST				DISPLAY
// PESU	PERSIST_SETUP		DISPLAY
// PNSU	PANEL_SETUP			SAVE/RECALL
// PFDS	PF_DISPLAY			FUNCTION
// PFST	PF_SET				FUNCTION
// PFSL	PF_SAVELOAD			SAVE/RECALL
// PFCT	PF_CONTROL			FUNCTION
// PFCM	PF_CREATEM			FUNCTION
// PFDD	PF_DATEDIS			FUNCTION
// *RCL	*RCL				SAVE/RECALL
// REC	RECALL				WAVEFORMTRANS
// RCPN	RECALL_PANEL		SAVE/RECALL
// *RST	*RST				SAVE/RECALL
// REFS	REF_SET				FUNCTION
// *SAV	*SAV				SAVE/RECALL
// SCDP	SCREEN_DUMP			HARD COPY
// SCSV	SCREEN_SAVE			DISPLAY
// *SRE	*SRE				STATUS
// *STB?	*STB?				STATUS
// STOP	STOP				ACQUISITION
// STO	STORE				WAVEFORMTRANS
// STPN	STORE_PANEL			SAVE/RECALL
// STST	STORE_SETUP			WAVEFORMTRANS
// SANU	SAMPLE_NUM			ACQUISITION
// SKEW	SKEW				ACQUISITION
// SET50	SETTO%50			FUNCTION
// SXSA	SINXX_SAMPLE		ACQUISITION
// TMPL	TEMPLATE			WAVEFORM TRANSFER
// *TRG	*TRG				ACQUISITION
// TRCP	TRIG_COUPLING		ACQUISITION
// TRDL	TRIG_DELAY			ACQUISITION
// TRLV	TRIG_LEVEL			ACQUISITION
// TRSE	TRIG_SELECT			ACQUISITION
// TRSL	TRIG_SLOPE			ACQUISITION
// UNIT	UNIT				ACQUISITION
// VPOS	VERT_POSITION		DISPLAY
// VTCL	VERTICAL			ACQUISITION
// WAIT	WAIT				ACQUISITION
// WFSU	WAVEFORM_SETUP		WAVEFORMTRANS
// XYDS	XY_DISPLAY			DISPLAY
// ASET	AUTO_SETUP			ACQUISITION
// BUZZ	BUZZER				MISCELLANEOUS

// Partially implemented
// SAST			SAMPLE_STATUS		ACQUISITION
// WF			WAVEFORM			WAVEFORMTRANS

// Implemented
// *IDN?		*IDN?				MISCELLANEOUS
// ARM			ARM_ACQUISITION		ACQUISITION
// CYMT			CYMOMETER			FUNCTION
// FRTR			FORCE_TRIGGER		ACQUISITION
// OFST			OFFSET				ACQUISITION
// SARA			SAMPLE_RATE			ACQUISITION
// TDIV			TIME_DIV			ACQUISITION
// TRA			TRACE				DISPLAY
// TRMD	 		TRIG_MODE			ACQUISITION
// VDIV			VOLT_DIV			ACQUISITION
