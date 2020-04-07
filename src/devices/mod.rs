
// Currently all devices supported here are Siglent.  If multiple manufacturers are ever supported, I'll probably
// organize them into modules by manufacturer

pub mod sds1202x {

	use std::io::{self, Error, ErrorKind};
	use std::ops::Drop;
	use std::str;

	use crate::vxi11::CoreClient;
	
	pub struct SDS1202X {
		core: CoreClient,
	}

	impl SDS1202X {
		
		pub fn new(host:&str) -> io::Result<Self> {
			let mut core = CoreClient::new(host)?;

			core.create_link()?;

			match str::from_utf8(&(core.ask(b"*IDN?")?)) {
				Ok(idn_resp) => {
					if idn_resp.contains("SDS1202X") { /* Do nothing because this is what we expected */ }
					else { return Err(Error::new(ErrorKind::Other, "Successfully connected to a device but it doesn't appear to be the right model")); }
				},
				Err(_) => return Err(Error::new(ErrorKind::Other, "Received a response to *IDN? but unable to interpret as UTF-8")),
			}

			Ok(Self{ core })
		}

		pub fn ask(&mut self, data:&[u8]) -> io::Result<Vec<u8>> { self.core.ask(data) }

	}

	impl Drop for SDS1202X {

		fn drop(&mut self) { self.core.destroy_link().expect("Unable to destroy link for SDS1202X"); }

	}

}