
// Device core
pub const DEVICE_CORE_PROG:u32  = 0x0607af;
pub const DEVICE_CORE_VERS:u32  = 1;
pub const CREATE_LINK:u32       = 10;
pub const DEVICE_WRITE:u32      = 11;
pub const DEVICE_READ:u32       = 12;
pub const DEVICE_READSTB:u32    = 13;
pub const DEVICE_TRIGGER:u32    = 14;
pub const DEVICE_CLEAR:u32      = 15;
pub const DEVICE_REMOTE:u32     = 16;
pub const DEVICE_LOCAL:u32      = 17;
pub const DEVICE_LOCK:u32       = 18;
pub const DEVICE_UNLOCK:u32     = 19;
pub const DEVICE_ENABLE_SRQ:u32 = 20;
pub const DEVICE_DOCMD:u32      = 22;
pub const DESTROY_LINK:u32      = 23;
pub const CREATE_INTR_CHAN:u32  = 25;
pub const DESTROY_INTR_CHAN:u32 = 26;

pub const CLIENT_ID:i32 = 3333;
pub const DEFAULT_LOCK_TIMEOUT:u32 = 10000;

pub const OPERATION_FLAGS_END_ONLY:i32 = 8;

use std::io::{self, Error, ErrorKind};

use crate::rpc::port_mapping::{TcpPortMapperClient, Mapping, Protocol};
use crate::rpc::xdr_pack::{pack_callheader_no_auth};
use crate::rpc::tcp_clients::TcpClient;

fn err(msg:&str) -> io::Error { Error::new(ErrorKind::Other, msg) }

pub mod xdr_pack;

// TODO: implement abort and interrupt clients

pub struct CoreClient {
    client: TcpClient,
    opt_link: Option<Link>,      // TODO: consider trying to support multiple links at once, in which case this will become a Vec<u32>
}

pub struct Link {
    pub link_id: i32,
    pub abort_port: u32,
    pub max_recv_size: u32,
}

impl CoreClient {
    
    fn get_link(&self) -> io::Result<i32> {
        match self.opt_link {
            Some(Link{ link_id, .. }) => Ok(link_id),
            None => Err(err("No link")),
        }
    }

    pub fn new(host:&str) -> io::Result<Self> {

        // Find the port to use for the core program
        let mut pmap_client = TcpPortMapperClient::new(host)?;

        let mapping = Mapping {
            program: DEVICE_CORE_PROG,
            version: DEVICE_CORE_VERS,
            protocol: Protocol::TCP,
            port: 0,
        };
        
        let port = pmap_client.get_port(&mapping)?;

        // Connect on the port specified and create a packer and unpacker
        let client   = TcpClient::connect((host, port as u16), DEVICE_CORE_PROG, DEVICE_CORE_VERS)?;

        // Build and return the struct
        Ok(CoreClient {client, opt_link: None })
    }

    pub fn create_link(&mut self) -> io::Result<()> {
        if self.opt_link.is_some() {
            return Err(err("Already connected to a link"));
        }

        self.client.lastxid += 1;
        self.client.packer.reset();
        
        pack_callheader_no_auth(&mut self.client.packer, self.client.lastxid, DEVICE_CORE_PROG, DEVICE_CORE_VERS, CREATE_LINK)?;
        xdr_pack::pack_create_link_parms(&mut self.client.packer, CLIENT_ID, false, DEFAULT_LOCK_TIMEOUT, "inst0")?;
        
        self.client.do_call()?;

        let error:i32         = self.client.unpacker.unpack_i32()?;
        let link_id:i32       = self.client.unpacker.unpack_i32()?;
        let abort_port:u32    = self.client.unpacker.unpack_u32()?;
        let max_recv_size:u32 = self.client.unpacker.unpack_u32()?;

        self.opt_link = Some(Link{ link_id, abort_port, max_recv_size });

        match error {
            0  => Ok(()),
            1  => Err(err("Syntax error")),
            3  => Err(err("Device not accessible")),
            9  => Err(err("Out of resources")),
            11 => Err(err("Device locked by another link")),
            21 => Err(err("Invalid address")),
            _  => Err(err("Unknown error"))
        }
    }

    // TODO: consider moving this to the device level
    pub fn ask(&mut self, data:&[u8]) -> io::Result<Vec<u8>> {
        self.write(data)?;
        self.read()
    }

    pub fn write(&mut self, data:&[u8]) -> io::Result<()> {
        self.client.lastxid += 1;
        self.client.packer.reset();

        let link_id:i32 = self.get_link()?;
        pack_callheader_no_auth(&mut self.client.packer, self.client.lastxid, DEVICE_CORE_PROG, DEVICE_CORE_VERS, DEVICE_WRITE)?;
        xdr_pack::pack_device_write_parms(&mut self.client.packer, link_id, DEFAULT_LOCK_TIMEOUT, DEFAULT_LOCK_TIMEOUT, OPERATION_FLAGS_END_ONLY, data)?;
        
        self.client.do_call()?;
    
        let error:i32 = self.client.unpacker.unpack_i32()?;
        let size:u32  = self.client.unpacker.unpack_u32()?;

        if size as usize != data.len() {
            return Err(Error::new(ErrorKind::Other, "Number of bytes in confirmation doesn't match number of bytes sent"));
        }

        match error {
            0  => Ok(()),
            4  => Err(err("Invalid link identifier")),
            5  => Err(err("Parameter error")),
            11 => Err(err("Device locked by another link")),
            15 => Err(err("I/O timeout")),
            17 => Err(err("I/O error")),
            23 => Err(err("Abort")),
            _  => Err(err("Unknown error")),
        }

    }

    pub fn read(&mut self) -> io::Result<Vec<u8>> {
        self.client.lastxid += 1;
        self.client.packer.reset();
        
        let link_id:i32 = self.get_link()?;
        pack_callheader_no_auth(&mut self.client.packer, self.client.lastxid, DEVICE_CORE_PROG, DEVICE_CORE_VERS, DEVICE_READ)?;
        xdr_pack::pack_device_read_parms(&mut self.client.packer, link_id, std::u32::MAX, DEFAULT_LOCK_TIMEOUT, DEFAULT_LOCK_TIMEOUT, 0, 0)?;
        self.client.do_call()?;

        let error:i32    = self.client.unpacker.unpack_i32()?;
        let reason:i32   = self.client.unpacker.unpack_i32()?;
        let data:Vec<u8> = self.client.unpacker.unpack_variable_len_opaque()?;
        
        match error {
            0  => {
                match reason {
                    0 => Err(err("Expected one of three reason bits to be set")),
                    1 => Err(err("End of read due to requested byte count reached")),
                    2 => Err(err("End of read due to termination character")),
                    3 => Err(err("Unexpected combination of reason bits")),
                    4 => Ok(data),
                    _ => Err(err("Bit in reason code that should be zero aren't zero")),
                }
            },
            4  => Err(err("Invalid link identifier")),
            11 => Err(err("Device locked by another link")),
            15 => Err(err("I/O timeout")),
            17 => Err(err("I/O error")),
            23 => Err(err("Abort")),
            _  => Err(err("Unknown error")),
        }

    }

    pub fn destroy_link(&mut self) -> io::Result<()> {
        if self.opt_link.is_none() {
            return Err(err("No link to destroy"));
        }

        self.client.lastxid += 1;
        self.client.packer.reset();
        
        let link_id:i32 = self.get_link()?;
        pack_callheader_no_auth(&mut self.client.packer, self.client.lastxid, DEVICE_CORE_PROG, DEVICE_CORE_VERS, DESTROY_LINK)?;
        self.client.packer.pack_i32(link_id)?;
        self.client.do_call()?;

        let device_error:i32 = self.client.unpacker.unpack_i32()?;
        match device_error {
            0 => Ok(()),
            4 => Err(err("Invalid link identifier")),
            _ => Err(err("Unknown error code")),
        }

    }

}