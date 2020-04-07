
use std::io::{self, Error, ErrorKind};

use crate::xdr::Unpacker;
use crate::rpc::{REPLY, MSG_DENIED, RPC_MISMATCH, AUTH_ERROR, MSG_ACCEPTED, PROG_UNAVAIL, PROG_MISMATCH, GARBAGE_ARGS, SUCCESS};

pub fn unpack_auth(unpacker:&mut Unpacker) -> io::Result<(i32, Vec<u8>)> {
	let flavor:i32    = unpacker.unpack_enum()?;
	let stuff:Vec<u8> = unpacker.unpack_variable_len_opaque()?;
	Ok((flavor, stuff))
}

// TODO: consider making an auth struct instead of just using (i32, Vec<u8>)
pub fn unpack_replyheader(unpacker:&mut Unpacker) -> io::Result<(u32, (i32, Vec<u8>))> {
    let xid:u32 = unpacker.unpack_u32()?;

    let mtype:i32 = unpacker.unpack_enum()?;
    if mtype != REPLY { return Err(Error::new(ErrorKind::Other, "Expected REPLY message type in unpack_replyheader")); }

    match unpacker.unpack_enum()? {
		MSG_DENIED => {
	    	match unpacker.unpack_enum()? {
	    		RPC_MISMATCH => {
	    			unpacker.unpack_u32()?;	// This u32 gives the low value
					unpacker.unpack_u32()?;	// This u32 gives the high value
					return Err(Error::new(ErrorKind::Other, "Message denied due to RPC_MISMATCH in unpack_replyheader"))
	    		},
	    		AUTH_ERROR => {
					unpacker.unpack_u32()?;	// This u32 gives us another status code that might have more detail if needed
					return Err(Error::new(ErrorKind::Other, "Message denied due to AUTH_ERROR in unpack_replyheader"))
	    		}
	    		_ => return Err(Error::new(ErrorKind::Other, "Message denied for an unknown reason in unpack_replyheader")),
	    	}
	    },
	    MSG_ACCEPTED => { },
	    _            => return Err(Error::new(ErrorKind::Other, "Neither MSG_DENIED nor MSG_ACCEPTED in unpack_replyheader")),
    }

    let verf = unpack_auth(unpacker)?;

    match unpacker.unpack_enum()? {
    	PROG_UNAVAIL  => return Err(Error::new(ErrorKind::Other, "Program unavailable in unpack_replyheader")),
    	PROG_MISMATCH => {
			unpacker.unpack_u32()?;	// This u32 gives the low value
			unpacker.unpack_u32()?;	// This u32 gives the high value
    		return Err(Error::new(ErrorKind::Other, "Program mismatch in unpack_replyheader"))
    	},
    	GARBAGE_ARGS  => return Err(Error::new(ErrorKind::Other, "Garbage args in unpack_replyheader")),
    	SUCCESS => { },
    	_ => return Err(Error::new(ErrorKind::Other, "Call failed for unknown reason in unpack_replyheader")),
    }

	Ok((xid, verf))
}

