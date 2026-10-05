use std::os::fd::OwnedFd;

use libbinder::object::ObjectTrait;
use libbinder_basic_macros::binder_ipc_object;

#[binder_ipc_object(root = true)]
pub trait IObject: ObjectTrait {
    // this can be used to determine if derived proxy can be made
    // but lets put that out of scope, assume this is normal method
    fn has_interface(&self, name: &str) -> anyhow::Result<bool>;
    fn get_an_file(&self, path: &str) -> anyhow::Result<OwnedFd>;
}

fn a() {
    //
}
