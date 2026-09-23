use libbinder_sys::transaction::TransactionKernelManaged;

pub struct KernelManaged {
    pub inner: TransactionKernelManaged,
}
