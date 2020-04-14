
use std::io::{self, Error, ErrorKind};
use std::net::{UdpSocket, ToSocketAddrs};
use std::time::Duration;

use crate::xdr;
use super::{xdr_pack, xdr_unpack};

pub struct UdpClient {
	pub socket: UdpSocket,
    pub prog: u32,
    pub vers: u32,
    pub lastxid: u32,
    recv_buff: [u8; 8092],
}

impl UdpClient {

	pub fn connect<A: ToSocketAddrs>(addr: A, prog: u32, vers: u32) -> io::Result<Self> {
		let socket:UdpSocket = (3600..3900).map(|port| { UdpSocket::bind(("0.0.0.0", port as u16)) })
			.find(|sock_result| sock_result.is_ok() )
			.unwrap_or(Err(Error::new(ErrorKind::Other, "No available ports found in the range scanned")))?;
		socket.connect(addr)?;
		Ok(Self{ socket, prog, vers, lastxid: 0, recv_buff: [0; 8092] })
	}

	pub fn do_call(&mut self, call:&[u8], unpacker:&mut xdr::Unpacker, lastxid:u32) -> io::Result<()> {
        
        match self.socket.send(call) {
        	Ok(n) => {
        		if n < call.len() { return Err(Error::new(ErrorKind::Other, "Unable to send all bytes")) }
        		else if n > call.len() { return Err(Error::new(ErrorKind::Other, "Somehow sent more bytes than expected")) }
        		// If n == call.len(), then do nothing and continue because that's what we expected
        	},
        	Err(e) => return Err(e),
        }

    	// TODO: consider resending if the first recv fails
    	let n = self.socket.recv(&mut self.recv_buff)?;

    	unpacker.reset(&self.recv_buff[0..n as usize]);
    	
    	let (xid, _) = xdr_unpack::unpack_replyheader(unpacker)?;
    	if xid == lastxid { Ok(())                                                             }
    	else              { Err(Error::new(ErrorKind::Other, "Wrong xid in received message")) }
	}
}

pub struct BroadcastUdpClient {
	pub socket: UdpSocket,
    pub prog: u32,
    pub vers: u32,
    pub port: u16,
    pub lastxid: u32,
    pub packer: xdr::Packer,
    pub unpacker: xdr::Unpacker,
    recv_buff: [u8; 8092],
}

impl BroadcastUdpClient {

	// https://stackoverflow.com/questions/61045602/how-do-you-broadcast-a-udp-datagram-and-receive-the-responses-in-rust?noredirect=1#comment107997707_61045602

	pub fn bind(port:u16, prog: u32, vers: u32) -> io::Result<Self> {
		let socket:UdpSocket = UdpSocket::bind("0.0.0.0:0")?;
		socket.set_read_timeout(Some(Duration::new(5, 0)))?;
		socket.set_broadcast(true)?;

		let packer = xdr::Packer::new();
		let unpacker = xdr::Unpacker::new();

		Ok(Self{ socket, prog, vers, port: port as u16, lastxid: 0, packer, unpacker, recv_buff: [0; 8092] })
	}


    pub fn start_call(&mut self, prc:u32) -> io::Result<()> {
        self.lastxid += 1;
        self.packer.reset();
        xdr_pack::pack_callheader_no_auth(&mut self.packer, self.lastxid, self.prog, self.vers, prc)
    }

    pub fn make_call(&mut self) -> io::Result<()> {
	    // Function arguments should have just been packed when this message is called
	    let call:Vec<u8> = self.packer.get_buf()?;
	    match self.socket.send_to(&call, ("255.255.255.255", self.port)) {
	    	Ok(n) => {
	    		if n != call.len() {
	    			return Err(Error::new(ErrorKind::Other, "Sent the wrong number of bytes"))
	    		}
	    		else {
	    			// Do nothing because we sent the number of bytes we expected to send
	    		}
	    	},
	    	Err(e) => return Err(e),
	    }

	    while let Ok((n, addr)) = self.socket.recv_from(&mut self.recv_buff) {
		    self.unpacker.reset(&self.recv_buff[0..(n as usize)]);
		    let (xid, _) = xdr_unpack::unpack_replyheader(&mut self.unpacker)?;

		    // Only keep messages with the correct xid
		    if xid == self.lastxid {
		        let reply:Vec<u8> = self.unpacker.get_remaining_bytes()?;
		        println!("From {:?}: {:?}", addr, reply);
		    }

	    }

    	// TODO: think about returning the replies instead of storing them as state
    	Ok(())
    }


}

