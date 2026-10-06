use std::sync::Arc;

use libbinder::{
    object::{B, ObjectTrait, TransactionError},
    proxy::Proxy,
};

// This library exposes useful types and helpers for using libbinder
pub mod packetable;
pub mod reader;
pub mod writer;

pub static REPLY_SUCCESS: u32 = 0;
pub static REPLY_FAILURE: u32 = 1;

pub trait TryFromProxy: ObjectTrait {
    type ProxyType: ObjectTrait;

    fn try_from_proxy(proxy: Proxy) -> Result<Arc<B<Self>>, TransactionError>;
    fn into_base(reference: Arc<B<Self>>) -> Arc<B<dyn ObjectTrait>>;
}

impl TryFromProxy for dyn ObjectTrait {
    type ProxyType = Proxy;

    fn try_from_proxy(proxy: Proxy) -> Result<Arc<B<Self>>, TransactionError> {
        Ok(Arc::new(B::new(proxy)))
    }

    fn into_base(reference: Arc<B<Self>>) -> Arc<B<dyn ObjectTrait>> {
        reference
    }
}

#[macro_export]
macro_rules! object_trait_forwarder {
    (remote $impl_name:ident, $($target_field:tt)*) => {
        impl ::libbinder::object::ObjectTrait for $impl_name {
            fn get_remote<'a>(&'a self) -> Option<&'a ::libbinder::proxy::Proxy> {
                (self.$($target_field)*).get_remote()
            }

            fn get_runtime(&self) -> std::sync::Arc<::libbinder::Runtime> {
                (self.$($target_field)*).get_runtime()
            }

            fn on_transaction(
                &self,
                code: ::std::primitive::u32,
                flags: ::enumflags2::BitFlags<::libbinder::object::Flag>,
                message: &mut ::libbinder::packet::Packet,
            ) -> Result<
                Option<(u32, ::libbinder::packet::Packet)>,
                ::libbinder::object::TransactionError,
            > {
                (self.$($target_field)*).on_transaction(code, flags, message)
            }
        }
    };

    (local $impl_name:ident, $($target_field:tt)*) => {
        impl ::libbinder::object::ObjectTrait for $impl_name {
            fn get_remote<'a>(&'a self) -> Option<&'a ::libbinder::proxy::Proxy> {
                None
            }

            fn get_runtime(&self) -> std::sync::Arc<::libbinder::Runtime> {
                (self.$($target_field)*).get_runtime()
            }

            fn on_transaction(
                &self,
                code: ::std::primitive::u32,
                flags: ::enumflags2::BitFlags<::libbinder::object::Flag>,
                message: &mut ::libbinder::packet::Packet,
            ) -> Result<
                Option<(u32, ::libbinder::packet::Packet)>,
                ::libbinder::object::TransactionError,
            > {
                (self.$($target_field)*).on_transaction(code, flags, message)
            }
        }
    };
}
