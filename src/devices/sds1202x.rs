

extern crate regex;

use std::io::{self, Error, ErrorKind};
use std::ops::Drop;
use std::str;
use std::thread;
use std::time::Duration;

use regex::{Captures, Match, Regex};
use serde::{Serialize, Deserialize};

use crate::vxi11::CoreClient;

lazy_static! {
    static ref IDN_RE: Regex  = Regex::new("([^,]+),([^,]+),([^,]+),([^,\\s]+)").unwrap();
    static ref SARA_RE: Regex = Regex::new("SARA\\s(\\d+)(\\D)Sa/s").unwrap();
    static ref TDIV_RE: Regex = Regex::new("TDIV\\s([^S]+)S").unwrap();
    static ref VDIV_RE: Regex = Regex::new("(C\\d):VDIV\\s(.+)V\\s").unwrap();
}

pub const DEFAULT_TX_THROTTLE_DURATION_SEC:f32 = 0.1;

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
	pub ch1: ChannelState,
	pub ch2: ChannelState,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelState {
	pub voltage_division: f32,
}

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

		let ch1 = self.get_channel_state(1)?;
		let ch2 = self.get_channel_state(2)?;

		Ok(State{ manufacturer, model, serial_num, fw_version, time_division, ch1, ch2 })
	}

	pub fn get_channel_state(&mut self, chan_num:u8) -> io::Result<ChannelState> {
		chan_ok(chan_num)?;

	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
	    // TODO: remove all unwraps
	    let voltage_division:f32 = {
			let cmd:String   = format!("C{}:VDIV?", chan_num);
		    let res:String   = self.ask_str(&cmd)?;
			let cap:Captures = VDIV_RE.captures(&res).unwrap();
	    	(match_str(cap.get(2), "No match for voltage_division")?).parse::<f32>().unwrap()
	    };

		Ok(ChannelState{ voltage_division })
	}

	pub fn get_time_division(&mut self) -> io::Result<f32> {
	    let res:String   = self.ask_str("TDIV?")?;
	    let cap:Captures = TDIV_RE.captures(&res).unwrap();
    	(match_str(cap.get(1), "No match for time_division")?).parse::<f32>().map_err(|_| Error::new(ErrorKind::Other, "SDS1202X only has two channels"))
	}

	pub fn set_time_division(&mut self, tdiv:f32) -> io::Result<()> {
		// The fine scale of voltage division is 10 [mV] so 2 decimal places is all we need
		let cmd:String = format!("TDIV {:.7}S", tdiv);
	    self.ask_str(&cmd)?;

		Ok(())
	}

	pub fn set_voltage_div(&mut self, chan_num:u8, vdiv:f32) -> io::Result<()> {
		// TODO add options for whether to enable a full, partial, or no state update after commanding a configuration change
		chan_ok(chan_num)?;

		// The fine scale of voltage division is 10 [mV] so 2 decimal places is all we need
		let cmd:String  = format!("C{}:VDIV {:.2}", chan_num, vdiv);
	    self.ask_str(&cmd)?;

		Ok(())
	}

	pub fn read_cymometer(&mut self) -> io::Result<String> {
		// TODO: decode to a float
		Ok(str::from_utf8(&self.core.ask(b"CYMT?")?).map(|s| s.to_owned()).unwrap())
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
// ARM	ARM_ACQUISITION		ACQUISITION
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
// FRTR	FORCE_TRIGGER		ACQUISITION
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
// OFST	OFFSET				ACQUISITION
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
// TRA	TRACE				DISPLAY
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
// WF	WAVEFORM			WAVEFORMTRANS
// WFSU	WAVEFORM_SETUP		WAVEFORMTRANS
// XYDS	XY_DISPLAY			DISPLAY
// ASET	AUTO_SETUP			ACQUISITION
// BUZZ	BUZZER				MISCELLANEOUS
// SAST	SAMPLE_STATUS		ACQUISITION
// SARA	SAMPLE_RATE			ACQUISITION
// TDIV	TIME_DIV			ACQUISITION
// TRMD	TRIG_MODE			ACQUISITION

// Partially implemented

// Implemented
// *IDN?		*IDN?				MISCELLANEOUS
// CYMT			CYMOMETER			FUNCTION
// VDIV			VOLT_DIV			ACQUISITION
