
use std::io::{self, Error, ErrorKind};

#[derive(PartialEq)]
enum Transition { HighToLow, LowToHigh}

fn all_transitions(data:&[i8], hi_thres:i8, lo_thres:i8) -> Vec<(usize, Transition)> {
	let mut ans:Vec<(usize, Transition)> = vec![];

	let mut idx = 0;
	let mut currently_high:bool = data[idx] > (hi_thres+lo_thres)/2;

	while idx < data.len() {
		if currently_high && data[idx] < lo_thres {
			currently_high = false;
			ans.push((idx, Transition::HighToLow));
		} else if !currently_high && data[idx] > hi_thres {
			currently_high = true;
			ans.push((idx, Transition::LowToHigh));
		}
		idx += 1;
	}

	ans
}

fn transitions_to_pulses(transitions:&[(usize, Transition)]) -> Vec<(usize, bool)> {
	let mut ans:Vec<(usize, bool)> = vec![];
	let mut idx = 0;

	for (trans_idx, trans_type) in transitions {
		ans.push((*trans_idx - idx, *trans_type == Transition::HighToLow));
		idx = *trans_idx;
	}

	ans
}

pub fn uart(data:&[i8], sample_rate_sps:f32, baud_rate:f32, bits_per_frame:usize) -> io::Result<Vec<u8>> {

	let lo_level:i8 = data.iter().map(|x| *x).min().unwrap();
	let hi_level:i8 = data.iter().map(|x| *x).max().unwrap();

	let lo_thres:i8 = lo_level + ((hi_level-lo_level) / 5);
	let hi_thres:i8 = hi_level - ((hi_level-lo_level) / 5);

	let samples_per_symbol:f32 = sample_rate_sps / baud_rate;	// [samples/sec] / [symbols/sec]
	let samples_per_frame:f32  = samples_per_symbol * ((bits_per_frame + 2) as f32);  // The extra two bits are the start and stop bits 

	let transitions = all_transitions(data, hi_thres, lo_thres);
	let mut pulses  = transitions_to_pulses(&transitions);

	'search: while pulses.len() > 0 {
		// Loop until we find a high pulse at least the length of a frame
		let (len, logic) = pulses.remove(0); 

		if logic && len > (samples_per_frame as usize) {
			break 'search;
		}
	}

	let mut ans:Vec<u8> = vec![];

	// Decode one pulse at a time
	let mut current_byte:u8 = 0;
	let mut place_val:u8 = 1;
	let mut need_start_bit:bool = true;
	let mut need_stop_bit:bool  = false;

	for (len, logic) in pulses {
		let pulse_len:usize = ((len as f32) / samples_per_symbol).round() as usize;
		
		// Three possible states: awaiting start bit, building frame, awaiting stop bit
		if need_start_bit {
			if !logic {
				// Start bits are low voltage (logical 0), so this is the start bit we were waiting for, plus potentially a few data bits
				need_start_bit = false;
				current_byte = 0;
				place_val = 2u8.checked_pow((pulse_len-1) as u32).ok_or(Error::new(ErrorKind::Other, "Overflow in u8 exponentiation"))?;
			} else {
				return Err(Error::new(ErrorKind::Other, "Encountered high voltage when low was expected as a start bit"))
			}
		} else if need_stop_bit {
			if logic {
				// Stop bit is high voltage (logical 1) and there can't be any more data until another start bit (logical 0),
				// so call this frame done and any pulsewidth remaining after the stop bit is just space in between frames
				ans.push(current_byte);
				need_stop_bit  = false;
				need_start_bit = true;
			} else {
				return Err(Error::new(ErrorKind::Other, "Encountered low voltage when high was expected as a stop bit"))
			}
		} else {
			// We're just in the middle of building a frame
			for idx in 0..pulse_len {

				if logic { current_byte += place_val; }

				place_val = match place_val.checked_mul(2) {
					Some(x) => x,
					None => {
						// The next bit should be the last bit of this pulse and it should be a stop bit
						if idx == pulse_len-1 { 
							// If this is the last bit of this pulse, then that's fine, but the stop bit should be the next pulse
							need_stop_bit = true;
						} else {
							// If this is the second to last bit of this pulse, then the next one is the stop bit, so it needs to be high
							// It's okay if it's not the last clock cycle of this pulse because it can stay high longer than one clock width
							// in between frames
							if logic { 
								ans.push(current_byte);
								need_stop_bit  = false;
								need_start_bit = true;
							} else { 
								return Err(Error::new(ErrorKind::Other, "Encountered low voltage when high was expected")) 
							}
						} 

						1
					}
				};
			}
		}

	}


	Ok(ans)
}