

extern crate regex;

use std::io::{self, Error, ErrorKind};
use std::ops::Drop;
use std::str;

use regex::{Captures, Match, Regex};

use crate::vxi11::CoreClient;

lazy_static! {
    static ref IDN_RE: Regex  = Regex::new("([^,]+),([^,]+),([^,]+),([^,\\s]+)").unwrap();
    static ref TDIV_RE: Regex = Regex::new("TDIV\\s([^S]+)S").unwrap();
    static ref SARA_RE: Regex = Regex::new("SARA\\s(\\d+)(\\D)Sa/s").unwrap();
    static ref VDIV_RE: Regex = Regex::new("(C\\d):VDIV\\s(.+)V\\s").unwrap();
}

pub struct SDS1202X {
	core: CoreClient,
	pub state: Option<State>,
}

#[derive(Debug)]
pub struct State {
	pub manufacturer: String,
	pub model: String,
	pub serial_num: String,
	pub fw_version: String,
}

#[derive(Debug)]
pub struct ChannelState {
	pub voltage_division: f32,
}

fn match_str(opt_match:Option<Match>, err:&str) -> io::Result<String> {
	match opt_match {
		Some(m) => Ok(m.as_str().to_owned()),
		None    => Err(Error::new(ErrorKind::Other, err))
	}
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

		Ok(Self{ core, state: None })
	}

	pub fn get_full_state(&mut self) -> io::Result<State> {
	    let str_idn:String      = str::from_utf8(&self.core.ask(b"*IDN?")?).map(|s| s.to_owned()).unwrap();
		let caps_idn:Captures   = IDN_RE.captures(&str_idn).unwrap();
		let manufacturer:String = match_str(caps_idn.get(1), "No match for manufacturer")?;
		let model:String        = match_str(caps_idn.get(2), "No match for model")?;
		let serial_num:String   = match_str(caps_idn.get(3), "No match for serial_num")?;
		let fw_version:String   = match_str(caps_idn.get(4), "No match for fw_version")?;

		Ok(State{ manufacturer, model, serial_num, fw_version })
	}

	pub fn get_channel_state(&mut self, chan_num:u8) -> io::Result<ChannelState> {
		if chan_num != 1 && chan_num != 2 { return Err(Error::new(ErrorKind::Other, "SDS1202X only has two channels")) }

		let str_vdiv_cmd:String  = format!("C{}:VDIV?", chan_num);
	    let str_vdiv:String      = str::from_utf8(&self.core.ask(str_vdiv_cmd.as_bytes())?).map(|s| s.to_owned()).unwrap();
		let caps_vdiv:Captures   = VDIV_RE.captures(&str_vdiv).unwrap();
	    // TODO: check group 1 of the captures to make sure it matches the channel we asked for
	    let voltage_division:f32 = (match_str(caps_vdiv.get(2), "No match for voltage_division")?).parse::<f32>().unwrap();

		Ok(ChannelState{ voltage_division })
	}

	pub fn ask(&mut self, data:&[u8]) -> io::Result<Vec<u8>> { self.core.ask(data) }

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
// CYMT	CYMOMETER			FUNCTION
// SAST	SAMPLE_STATUS		ACQUISITION
// SARA	SAMPLE_RATE			ACQUISITION
// TDIV	TIME_DIV			ACQUISITION
// TRMD	TRIG_MODE			ACQUISITION
// VDIV	VOLT_DIV			ACQUISITION

// Partially implemented

// Implemented
// *IDN?	*IDN?				MISCELLANEOUS
