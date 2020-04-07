
extern crate byteorder;

use std::io::{self, Read, Write, Error, ErrorKind};
use std::net::{TcpStream, ToSocketAddrs};

use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

use crate::xdr;
use super::xdr_unpack;

pub struct TcpClient {
    pub stream: TcpStream,
    pub prog: u32,
    pub vers: u32,
    pub lastxid: u32,
	pub packer: xdr::Packer,
	pub unpacker: xdr::Unpacker,
}

impl TcpClient {
	
	pub fn connect<A: ToSocketAddrs>(addr: A, prog: u32, vers: u32) -> io::Result<Self> {
		let packer = xdr::Packer::new();
		let unpacker = xdr::Unpacker::new();
		Ok(Self{ stream: TcpStream::connect(addr)?, prog, vers, lastxid: 0, packer, unpacker })
	}

	pub fn do_call(&mut self) -> io::Result<()> {
		let call:Vec<u8> = self.packer.get_buf()?;
		if call.len() > 0 {
			let header:u32 = call.len() as u32 | 0x80000000;

			let mut send_bytes:Vec<u8> = vec![];
			send_bytes.write_u32::<BigEndian>(header)?;
			send_bytes.extend_from_slice(&call);
			self.stream.write_all(&send_bytes)?;
		}

		'outer: loop {
			let mut reply:Vec<u8> = vec![];

			let mut last:bool = false;
			while !last {
				let x:u32 = self.stream.read_u32::<BigEndian>()?;

		        last = (x & 0x80000000) != 0;
		        let n:u32 = x & 0x7fffffff;

		        let mut frag:Vec<u8> = Vec::with_capacity(n as usize);
		        let mut buff:[u8; 4] = [0; 4];
		        while frag.len() < n as usize {
			        self.stream.read_exact(&mut buff)?;
			        frag.extend_from_slice(&buff);		        	
		        }

		        reply.append(&mut frag);
			}

	        // Load the response into the unpacker and make sure the xid matches
	        self.unpacker.reset(&reply);

	        let (xid, _) = xdr_unpack::unpack_replyheader(&mut self.unpacker)?;
	        if xid == self.lastxid {
				// Packet from the present
				return Ok(());
	        } else if xid < self.lastxid {
		        // Packet from the past
		        continue 'outer;
	        } else {
		        // Packet from the future?
	        	return Err(Error::new(ErrorKind::Other, "Somehow got a packet from the future"));
	        }
		}

	}

}
