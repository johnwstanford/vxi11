
extern crate vxi11;

use std::io;

use vxi11::rpc::{xdr_pack, IPPROTO_TCP};
use vxi11::rpc::port_mapping::{PMAP_PORT, PMAP_PROG, PMAP_VERS, PMAPPROC_GETPORT};
use vxi11::rpc::udp_clients::BroadcastUdpClient;
use vxi11::vxi11::{DEVICE_CORE_PROG, DEVICE_CORE_VERS};

pub fn main() -> io::Result<()> {

	let mut pmap = BroadcastUdpClient::bind(PMAP_PORT, PMAP_PROG, PMAP_VERS)?;
	pmap.start_call(PMAPPROC_GETPORT)?;
	xdr_pack::pack_mapping(&mut pmap.packer, DEVICE_CORE_PROG, DEVICE_CORE_VERS, IPPROTO_TCP, 0)?;

	pmap.make_call()?;
	
	Ok(())
}
