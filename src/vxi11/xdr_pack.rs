use std::io;

use crate::xdr::Packer;

pub fn pack_device_link(packer:&mut Packer, link:i32) -> io::Result<()> {
	packer.pack_i32(link)
}

pub fn pack_create_link_parms(packer:&mut Packer, id:i32, lock_device:bool, lock_timeout:u32, device:&str) -> io::Result<()> {
	assert!(device.chars().all(|c| c.is_ascii()));
	packer.pack_i32(id)?;
	packer.pack_bool(lock_device)?;
	packer.pack_u32(lock_timeout)?;
	packer.pack_variable_len_opaque(device.as_bytes())
}

pub fn pack_device_write_parms(packer:&mut Packer, link:i32, timeout:u32, lock_timeout:u32, flags:i32, data:&[u8]) -> io::Result<()> {
	packer.pack_i32(link)?;
	packer.pack_u32(timeout)?;
	packer.pack_u32(lock_timeout)?;
	packer.pack_i32(flags)?;
	packer.pack_variable_len_opaque(data)
}

pub fn pack_device_read_parms(packer:&mut Packer, link:i32, request_size:u32, timeout:u32, lock_timeout:u32, flags:i32, term_char:i32) -> io::Result<()> {
    packer.pack_i32(link)?;
    packer.pack_u32(request_size)?;
    packer.pack_u32(timeout)?;
    packer.pack_u32(lock_timeout)?;
    packer.pack_i32(flags)?;
    packer.pack_i32(term_char)
}

pub fn pack_device_generic_parms(packer:&mut Packer, link:i32, flags:i32, lock_timeout:u32, timeout:u32) -> io::Result<()> {
    packer.pack_i32(link)?;
    packer.pack_i32(flags)?;
    packer.pack_u32(lock_timeout)?;
    packer.pack_u32(timeout)
}

pub fn pack_device_remote_func_parms(packer:&mut Packer, host_addr:u32, host_port:u32, prog_num:u32, prog_vers:u32, prog_family:i32) -> io::Result<()> {
    packer.pack_u32(host_addr)?;
    packer.pack_u32(host_port)?;
    packer.pack_u32(prog_num)?;
    packer.pack_u32(prog_vers)?;
    packer.pack_i32(prog_family)
}

pub fn pack_device_enable_srq_parms(packer:&mut Packer, link:i32, enable:bool, handle:&[u8]) -> io::Result<()> {
	assert!(handle.len() < 40);
	packer.pack_i32(link)?;
	packer.pack_bool(enable)?;
	packer.pack_variable_len_opaque(handle)
}

pub fn pack_device_lock_parms(packer:&mut Packer, link:i32, flags:i32, lock_timeout:u32) -> io::Result<()> {
    packer.pack_i32(link)?;
    packer.pack_i32(flags)?;
    packer.pack_u32(lock_timeout)
}

pub fn pack_device_docmd_parms(packer:&mut Packer, link:i32, flags:i32, timeout:u32, lock_timeout:u32, cmd:i32, network_order:bool, datasize:i32, data_in:&[u8]) -> io::Result<()> {
    packer.pack_i32(link)?;
    packer.pack_i32(flags)?;
    packer.pack_u32(timeout)?;
    packer.pack_u32(lock_timeout)?;
    packer.pack_i32(cmd)?;
    packer.pack_bool(network_order)?;
    packer.pack_i32(datasize)?;
    packer.pack_variable_len_opaque(data_in)
}

pub fn pack_device_error(packer:&mut Packer, error:i32) -> io::Result<()> {
	packer.pack_i32(error)
}

pub fn pack_device_srq_parms(packer:&mut Packer, handle:&[u8]) -> io::Result<()> {
    packer.pack_variable_len_opaque(handle)
}

pub fn pack_create_link_resp(packer:&mut Packer, error:i32, link:i32, abort_port:u32, max_recv_size:u32) -> io::Result<()> {
    packer.pack_i32(error)?;
    packer.pack_i32(link)?;
    packer.pack_u32(abort_port)?;
    packer.pack_u32(max_recv_size)
}

pub fn pack_device_write_resp(packer:&mut Packer, error:i32, size:u32) -> io::Result<()> {
    packer.pack_i32(error)?;
    packer.pack_u32(size)
}

pub fn pack_device_read_resp(packer:&mut Packer, error:i32, reason:i32, data:&[u8]) -> io::Result<()> {
    packer.pack_i32(error)?;
    packer.pack_i32(reason)?;
    packer.pack_variable_len_opaque(data)
}

pub fn pack_device_read_stb_resp(packer:&mut Packer, error:i32, stb:u32) -> io::Result<()> {
    packer.pack_i32(error)?;
    packer.pack_u32(stb)
}

pub fn pack_device_docmd_resp(packer:&mut Packer, error:i32, data_out:&[u8]) -> io::Result<()> {
    packer.pack_i32(error)?;
    packer.pack_variable_len_opaque(data_out)
}
