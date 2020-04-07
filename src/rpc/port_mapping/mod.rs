
pub const PMAP_PROG:u32 = 100000;
pub const PMAP_VERS:u32 = 2;
pub const PMAP_PORT:u16 = 111;

pub const PMAPPROC_NULL:u32    = 0;     // (void) -> void
pub const PMAPPROC_SET:u32     = 1;     // (mapping) -> bool
pub const PMAPPROC_UNSET:u32   = 2;     // (mapping) -> bool
pub const PMAPPROC_GETPORT:u32 = 3;     // (mapping) -> unsigned int
pub const PMAPPROC_DUMP:u32    = 4;     // (void) -> pmaplist
pub const PMAPPROC_CALLIT:u32  = 5;     // (call_args) -> call_result

use std::io::{self, Error, ErrorKind};

use crate::xdr;

use super::{IPPROTO_TCP, IPPROTO_UDP};
use super::xdr_pack;
use super::tcp_clients::TcpClient;
use super::udp_clients::UdpClient;

#[derive(Debug)]
pub enum Protocol {
	TCP,
	UDP,
}

impl Protocol {
	pub fn to_u32(&self) -> u32 { match self {
		Protocol::TCP => IPPROTO_TCP,
		Protocol::UDP => IPPROTO_UDP,
	}}
}

#[derive(Debug)]
pub struct Mapping {
	pub program: u32,
	pub version: u32,
	pub protocol: Protocol,
	pub port: u32,				// TODO: consider changing this to be a u16, although maybe not because XDR encodes it as a u32 for alignment
}

pub struct TcpPortMapperClient {
	pub host: String,
	pub packer: xdr::Packer,
	pub unpacker: xdr::Unpacker,
	pub tcp_client: TcpClient,
}

impl TcpPortMapperClient {

	pub fn new(host:&str) -> io::Result<Self> {
		let packer = xdr::Packer::new();
		let unpacker = xdr::Unpacker::new();
		let tcp_client = TcpClient::connect((host, PMAP_PORT), PMAP_PROG, PMAP_VERS)?;
		Ok(Self{ host: host.to_owned(), packer, unpacker, tcp_client })
	}

	pub fn make_call(&mut self) -> io::Result<()> {
		// TODO: Have tcp_client read its own lastxid once testing is done
	    self.tcp_client.do_call(&self.packer.get_buf()?, &mut self.unpacker, self.tcp_client.lastxid)
	}

	pub fn start_call(&mut self, prc:u32) -> io::Result<()> {
	    self.tcp_client.lastxid += 1;
	    self.packer.reset();
	    xdr_pack::pack_callheader_no_auth(&mut self.packer, self.tcp_client.lastxid, self.tcp_client.prog, self.tcp_client.vers, prc)
	}


	pub fn get_port(&mut self, m:&Mapping) -> io::Result<u32> {
        self.start_call(PMAPPROC_GETPORT)?;
        xdr_pack::pack_mapping(&mut self.packer, m.program, m.version, m.protocol.to_u32(), m.port as u32)?;
        self.make_call()?;

       	let ans:u32 = self.unpacker.unpack_u32()?;

       	if self.unpacker.all_data_consumed() { Ok(ans) }
       	else { Err(Error::new(ErrorKind::Other, "Data unexpectedly left over in unpacker after unpacking port")) }
	}

}

pub struct UdpPortMapperClient {
	pub host: String,
	pub packer: xdr::Packer,
	pub unpacker: xdr::Unpacker,
	pub udp_client: UdpClient,
}

impl UdpPortMapperClient {
	
	pub fn new(host:&str) -> io::Result<Self> {
		let packer = xdr::Packer::new();
		let unpacker = xdr::Unpacker::new();
		let udp_client = UdpClient::connect((host, PMAP_PORT), PMAP_PROG, PMAP_VERS)?;
		Ok(UdpPortMapperClient{ host: host.to_owned(), packer, unpacker, udp_client })
	}

	pub fn start_call(&mut self, prc:u32) -> io::Result<()> {
        self.udp_client.lastxid += 1;
        self.packer.reset();
        xdr_pack::pack_callheader_no_auth(&mut self.packer, self.udp_client.lastxid, self.udp_client.prog, self.udp_client.vers, prc)
	}

	pub fn dump(&mut self) -> io::Result<Vec<Mapping>> {
		self.start_call(PMAPPROC_DUMP)?;
		self.udp_client.do_call(&self.packer.get_buf()?, &mut self.unpacker, self.udp_client.lastxid)?;

		let mut ans:Vec<Mapping> = vec![];
		while self.unpacker.unpack_u32()? == 1 {
			// TODO: consider moving this unpacking logic to xdr_unpack
	        let program:u32 = self.unpacker.unpack_u32()?;
	        let version:u32 = self.unpacker.unpack_u32()?;
	        let protocol = match self.unpacker.unpack_u32()? {
	        	IPPROTO_TCP => Protocol::TCP,
	        	IPPROTO_UDP => Protocol::UDP,
	        	_  => return Err(Error::new(ErrorKind::Other, "Unrecognized protocol type")),
	        };
	        let port:u32 = self.unpacker.unpack_u32()?;
	        ans.push(Mapping{ program, version, protocol, port });
		}

		Ok(ans)
	}
}

