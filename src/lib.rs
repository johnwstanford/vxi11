
// External data representation, a protocol for serializing data to be sent over the network
pub mod xdr;

// Remote procedure call, a protocol build on top of XDR to provide something like C-style function calls over the network
pub mod rpc;

// A protocol using RPC that's meant to communicate with instruments like oscilloscopes, power supplies, waveform generators, etc
pub mod vxi11;

// Module for devices that implement the VXI11 protocol
pub mod devices;
