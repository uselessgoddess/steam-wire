use std::fmt::Debug;
use std::io::{Read, Write};

use prost::Message;
use steam_wire_proto_common::{ProtoError, RpcMessage, RpcMethod};

pub trait ServiceMethodRequest: Debug + Message {
    const REQ_NAME: &'static str;
    type Response: RpcMessage;

    fn parse(_reader: &mut dyn Read) -> Result<Self, ProtoError>
    where
        Self: Sized;
    fn write(&self, _writer: &mut dyn Write) -> Result<(), ProtoError>;
    fn encode_size(&self) -> usize;
}

impl<T: RpcMethod> ServiceMethodRequest for T {
    const REQ_NAME: &'static str = T::METHOD_NAME;
    type Response = T::Response;

    fn parse(reader: &mut dyn Read) -> Result<Self, ProtoError> {
        <Self as RpcMessage>::parse(reader)
    }

    fn write(&self, writer: &mut dyn Write) -> Result<(), ProtoError> {
        <Self as RpcMessage>::write(self, writer)
    }

    fn encode_size(&self) -> usize {
        <Self as RpcMessage>::encode_size(self)
    }
}
