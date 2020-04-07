
use std::io;

use crate::xdr::Packer;
use crate::rpc::{CALL, RPCVERSION, REPLY, MSG_ACCEPTED, SUCCESS};

pub fn pack_auth(packer:&mut Packer, flavor:i32, stuff:&[u8]) -> io::Result<()> {
	packer.pack_enum(flavor)?;
	packer.pack_variable_len_opaque(stuff)
}

pub fn pack_callheader(packer:&mut Packer, xid:u32, prog:u32, vers:u32, prc:u32, cred:(i32, &[u8]), verf:(i32, &[u8])) -> io::Result<()> {
	packer.pack_u32(xid)?;
	packer.pack_enum(CALL)?;
	packer.pack_u32(RPCVERSION)?;
	packer.pack_u32(prog)?;
	packer.pack_u32(vers)?;
	packer.pack_u32(prc)?;
	pack_auth(packer, cred.0, cred.1)?;
	pack_auth(packer, verf.0, verf.1)
}

pub fn pack_callheader_no_auth(packer: &mut Packer, xid:u32, prog:u32, vers:u32, prc:u32) -> io::Result<()> {
	pack_callheader(packer, xid, prog, vers, prc, (0, &[]), (0, &[]))
}

pub fn pack_replyheader(packer: &mut Packer, xid:u32, verf:(i32, &[u8])) -> io::Result<()> {
	packer.pack_u32(xid)?;
	packer.pack_enum(REPLY)?;
	packer.pack_i32(MSG_ACCEPTED)?;	
	pack_auth(packer, verf.0, verf.1)?;
	packer.pack_enum(SUCCESS)
}

pub fn pack_mapping(packer: &mut Packer, prog:u32, vers:u32, prot:u32, port:u32) -> io::Result<()> {
	packer.pack_u32(prog)?;
	packer.pack_u32(vers)?;
	packer.pack_u32(prot)?;
	packer.pack_u32(port)
}

pub fn pack_call_args(packer: &mut Packer, prog:u32, vers:u32, prc:u32, args:&[u8]) -> io::Result<()> {
	packer.pack_u32(prog)?;
	packer.pack_u32(vers)?;
	packer.pack_u32(prc)?;
	packer.pack_variable_len_opaque(args)
}
