
pub const RPCVERSION:u32 = 2;

pub const CALL:i32  = 0;
pub const REPLY:i32 = 1;

pub const MSG_ACCEPTED:i32 = 0;
pub const MSG_DENIED:i32 = 1;

pub const RPC_MISMATCH:i32 = 0;
pub const AUTH_ERROR:i32 = 1;

pub const SUCCESS:i32 = 0;            // RPC executed successfully
pub const PROG_UNAVAIL:i32  = 1;      // remote hasn't exported program
pub const PROG_MISMATCH:i32 = 2;      // remote can't support version #
pub const PROC_UNAVAIL:i32  = 3;      // program can't support procedure
pub const GARBAGE_ARGS:i32  = 4;      // procedure can't decode params

pub const IPPROTO_TCP:u32 = 6;
pub const IPPROTO_UDP:u32 = 17;

pub mod xdr_unpack;
pub mod xdr_pack;

pub mod port_mapping;

// TODO: implement server functionality

pub mod tcp_clients;
pub mod udp_clients;
