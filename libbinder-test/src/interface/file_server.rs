use std::os::fd::OwnedFd;

use crate::interface::service::{self, IService};

pub trait IFileServer: IService {
    // Demonstrates the BINDER_TYPE_FD usage
    fn open_file(&self, path: &str) -> anyhow::Result<OwnedFd>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn write_file(&self, path: &str, buf: &[u8]) -> anyhow::Result<()>;
}

pub const ID: &str = "foxie.ifileserver";
pub const SERVICE_ID: &str = "foxie.fileserver";

pub const OPEN_FILE_CODE: u32 = service::NEXT_TRANSACTION_CODE + 0;
pub const READ_FILE_CODE: u32 = service::NEXT_TRANSACTION_CODE + 1;
pub const WRITE_FILE_CODE: u32 = service::NEXT_TRANSACTION_CODE + 2;

#[expect(unused)]
pub const NEXT_TRANSACTION_CODE: u32 = service::NEXT_TRANSACTION_CODE + 3;
