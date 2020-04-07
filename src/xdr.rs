
extern crate byteorder;

use std::io::{self, Write, Error, ErrorKind, Cursor};

use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

pub struct Packer{
	pub buff:Vec<u8>
}

pub struct Unpacker {
	pub buff:Vec<u8>
}

impl Packer {
	
	pub fn new() -> Self { Packer{buff: Vec::new()} }

	pub fn reset(&mut self) { self.buff.clear(); }

	pub fn get_buf(&mut self) -> io::Result<Vec<u8>> {
		Ok(self.buff.clone())
	}

	// Packing methods that can only add multiples of four bytes, so if we started off with the correct
	// padding, we'll end up with the correct padding
	pub fn pack_u32(&mut self, x:u32) -> io::Result<()> { self.buff.write_u32::<BigEndian>(x) }
	pub fn pack_i32(&mut self, x:i32) -> io::Result<()> { self.buff.write_i32::<BigEndian>(x) }
	
	pub fn pack_bool(&mut self, b:bool) -> io::Result<()> {
		if b { self.pack_i32(1) }
		else { self.pack_i32(0) }
	}

	pub fn pack_enum(&mut self, x:i32) -> io::Result<()> { self.pack_i32(x) }

	// Packing methods that require padding checks at the end
	pub fn pack_variable_len_opaque(&mut self, data:&[u8]) -> io::Result<()> {
		self.pack_u32(data.len() as u32)?;
		self.buff.write(data)?;

		// Ensure alignment
		while self.buff.len() % 4 != 0 { self.buff.push(0); }
		Ok(())
	}

}

impl Unpacker {
	
	pub fn new() -> Self { Unpacker{buff: Vec::new()} }

	pub fn reset(&mut self, data:&[u8]) { 
		self.buff.clear();
		self.buff.extend_from_slice(data);
	}

	pub fn all_data_consumed(&self) -> bool { self.buff.len() == 0 }
	pub fn drop(&mut self, n:usize) -> io::Result<()> {
		if n%4 != 0 {
			return Err(Error::new(ErrorKind::Other, "Only drop multiples of four bytes in order to maintain alignment"));
		}
		for _ in 0..n { 
			if self.buff.len() == 0 { return Err(Error::new(ErrorKind::Other, "Tried to drop past the end of the buffer")) }
			self.buff.remove(0); 
		}
		Ok(())
	}
	pub fn peek(&self, idx:usize) -> io::Result<u8> {
		self.buff.get(idx).map(|b| *b).ok_or(Error::new(ErrorKind::Other, "Tried to peek past the end of the buffer"))
	}
	pub fn get_remaining_bytes(&mut self) -> io::Result<Vec<u8>> {
		Ok(self.buff.clone())
	}

	pub fn unpack_u32(&mut self) -> io::Result<u32> { 
		let mut rdr = Cursor::new(&self.buff);
		let ans:u32 = rdr.read_u32::<BigEndian>()?;
		self.drop(4)?;
		Ok(ans)
	}
	pub fn unpack_i32(&mut self) -> io::Result<i32> { 
		let mut rdr = Cursor::new(&self.buff);
		let ans:i32 = rdr.read_i32::<BigEndian>()?;
		self.drop(4)?;
		Ok(ans)
	}

	// An enum is just an i32 with a restricted set of values.  We can't check that this value is in the restricted set at this
	// level because it depends on the application, so for our purposes here, an enum is the same as an i32
	pub fn unpack_enum(&mut self) -> io::Result<i32> { self.unpack_i32() }

	pub fn unpack_bool(&mut self) -> io::Result<bool> {
		match self.unpack_i32() {
			Ok(0) => Ok(false),
			Ok(1) => Ok(true),
			Ok(x) => panic!("Expected 0 or 1 in unpack_bool but got {}", x),
			Err(e) => Err(e),
		}
	}

	pub fn unpack_variable_len_opaque(&mut self) -> io::Result<Vec<u8>> {
		let n:u32 = self.unpack_u32()?;
		let ans:Vec<u8> = self.buff.drain(..(n as usize)).collect();

		// Drop several bytes if necessary to maintain alignment
		while self.buff.len() % 4 != 0 { self.buff.remove(0); }
		Ok(ans)
	}

}