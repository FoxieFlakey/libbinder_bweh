// This is a utility library for using bytemuck
// easing few stuffs such as cnoverting slice of
// bytes with unknown alignment

use std::{slice, mem::MaybeUninit, ops::Deref};

use bytemuck::{Pod, PodCastError};

#[derive(Clone)]
pub enum PodData<'data, T: Pod> {
    Borrow(&'data T),
    Owned(T),
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum FromBytesError {
    #[error("Size mismatch between T and number of bytes")]
    SizeMismatch,
}

impl<'data, T: Pod> PodData<'data, T> {
    pub fn as_bytes(this: &Self) -> &[u8] {
        match this {
            PodData::Borrow(x) => bytemuck::bytes_of(*x),
            PodData::Owned(x) => bytemuck::bytes_of(x),
        }
    }

    fn check_invariance() {
        assert!(
            size_of::<T>() > 0,
            "Zero sized type are specifically unsupported!"
        );
    }

    pub fn try_from_bytes(bytes: &'data [u8]) -> Result<Self, FromBytesError> {
        Self::check_invariance();
        match bytemuck::try_from_bytes(bytes) {
            Ok(x) => Ok(Self::Borrow(x)),
            Err(PodCastError::SizeMismatch) => Err(FromBytesError::SizeMismatch),
            Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                if bytes.len() != size_of::<T>() {
                    return Err(FromBytesError::SizeMismatch);
                }

                // Size is matches and only alignment wrong, the Pod trait literally
                // means do whatever tf i want :3
                let mut converted: MaybeUninit<T> = MaybeUninit::uninit();
                let ptr = converted.as_mut_ptr().cast::<u8>();

                // SAFETY: Checked that the length if correct
                unsafe { slice::from_raw_parts_mut(ptr, size_of::<T>()) }.copy_from_slice(bytes);

                // SAFETY: Copied data into it and initialized
                Ok(Self::Owned(unsafe { converted.assume_init() }))
            }
            Err(x) => panic!("unexpected '{x}'"),
        }
    }

    pub fn to_owned(this: Self) -> PodData<'static, T> {
        PodData::Owned(match this {
            PodData::Owned(x) => x,
            PodData::Borrow(x) => x.clone(),
        })
    }

    pub fn unwrap(this: Self) -> T {
        match Self::to_owned(this) {
            PodData::Borrow(_) => unreachable!(),
            PodData::Owned(x) => x,
        }
    }
}

impl<T: Pod> From<T> for PodData<'static, T> {
    fn from(value: T) -> Self {
        Self::check_invariance();
        Self::Owned(value)
    }
}

impl<T: Pod> Deref for PodData<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            PodData::Borrow(x) => x,
            PodData::Owned(x) => &x,
        }
    }
}
