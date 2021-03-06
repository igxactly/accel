//! Memory management
//!
//! Unified address
//! ---------------
//!
//! - All memories are mapped into a single 64bit memory space
//! - We can get where the pointed memory exists from its value.
//!
//! Memory Types
//! ------------
//!
//! |name                      | where exists | From Host | From Device | As slice | Description                                                            |
//! |:-------------------------|:------------:|:---------:|:-----------:|:--------:|:-----------------------------------------------------------------------|
//! | (usual) Host memory      | Host         | ✓         |  -          |  ✓       | allocated by usual manner, e.g. `vec![0; n]`                           |
//! | [Registered Host memory] | Host         | ✓         |  ✓          |  ✓       | A host memory registered into CUDA memory management system            |
//! | [Page-locked Host memory]| Host         | ✓         |  ✓          |  ✓       | OS memory paging is disabled for accelerating memory transfer          |
//! | [Device memory]          | Device       | ✓         |  ✓          |  ✓       | allocated on device as a single span                                   |
//! | [Array]                  | Device       | ✓         |  ✓          |  -       | properly aligned memory on device for using Texture and Surface memory |
//!
//! [Registered Host memory]:  ./struct.RegisteredMemory.html
//! [Page-locked Host memory]: ./struct.PageLockedMemory.html
//! [Device memory]:           ./struct.DeviceMemory.html
//! [Array]:                   ./struct.Array.html
//!

mod array;
mod device;
mod dimension;
mod info;
mod page_locked;
mod registered;
mod scalar;
mod slice;

pub use array::*;
pub use device::*;
pub use dimension::*;
pub use info::*;
pub use page_locked::*;
pub use registered::*;
pub use scalar::*;

use crate::*;
use cuda::*;
use num_traits::Zero;
use std::{mem::MaybeUninit, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MemoryType {
    Host,
    PageLocked,
    Device,
    Array,
}

/// Typed wrapper of cuPointerGetAttribute
fn get_attr<T, Attr>(ptr: *const T, attr: CUpointer_attribute) -> error::Result<Attr> {
    let data = MaybeUninit::uninit();
    unsafe {
        ffi_call!(
            cuPointerGetAttribute,
            data.as_ptr() as *mut _,
            attr,
            ptr as CUdeviceptr
        )?;
        data.assume_init()
    }
}

/// Has unique head address and allocated size.
pub trait Memory {
    /// Scalar type of each element
    type Elem: Scalar;

    /// Get head address of the memory as a const pointer
    fn head_addr(&self) -> *const Self::Elem;

    /// Get head address of the memory as a mutable pointer
    fn head_addr_mut(&mut self) -> *mut Self::Elem;

    /// Number of elements
    fn num_elem(&self) -> usize;

    /// Get memory type
    fn memory_type(&self) -> MemoryType;
}

/// Copy data from one to another
///
/// Examples
/// ---------
///
/// - memcpy from page-locked host memory to device memory
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// let src = PageLockedMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src);
/// ```
///
/// - memcpy from device memory to page-locked host memory
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = PageLockedMemory::<i32>::zeros(ctx.clone(), 12);
/// let src = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src);
/// ```
///
/// - memcpy from device to device
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// let src = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src);
/// ```
///
/// - memcpy from Rust slice to device memory
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// let src = vec![0_i32; 12];
/// dest.copy_from(src.as_slice()); // requires explicit cast to slice
/// ```
///
/// - memcpy from device memory to Rust slice
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = vec![0_i32; 12];
/// let src = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src);
/// ```
///
/// Requirements
/// -------------
///
/// - Cannot copy between different types
///
/// ```compile_fail
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = DeviceMemory::<i64>::zeros(ctx.clone(), 12);
/// let src = PageLockedMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src); // compile fail
/// ```
///
/// - Panics if sizes are different
///
/// ```should_panic
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut dest = DeviceMemory::<i32>::zeros(ctx.clone(), 24);
/// let src = PageLockedMemory::<i32>::zeros(ctx.clone(), 12);
/// dest.copy_from(&src); // will panic
/// ```
///
/// Panic
/// -----
/// - `self` and `src` are identical
/// - if `self` nad `src` belong to different context
/// - if the size memory size mismathes
pub trait Memcpy<Target: ?Sized>: Memory
where
    Target: Memory<Elem = Self::Elem> + Memcpy<Self>,
{
    fn copy_from(&mut self, source: &Target);
    fn copy_to(&self, destination: &mut Target) {
        destination.copy_from(self);
    }
}

/// Set all elements by `value`
///
/// Examples
/// ---------
///
/// - Set `i32`
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut mem = DeviceMemory::<i32>::zeros(ctx.clone(), 12);
/// mem.set(1234);
/// for &val in mem.as_slice() {
///   assert_eq!(val, 1234);
/// }
/// ```
///
/// - Set `f32`
///   - Be sure that `f64` is not supported yet because CUDA does not support 64-bit memset.
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut mem = DeviceMemory::<f32>::zeros(ctx.clone(), 12);
/// mem.set(1.0);
/// for &val in mem.as_slice() {
///   assert_eq!(val, 1.0);
/// }
/// ```
///
/// - Set for host memory equals to `mem.iter_mut().for_each(|v| *v = value)`
///
/// ```
/// # use accel::*;
/// # let device = Device::nth(0).unwrap();
/// # let ctx = device.create_context();
/// let mut mem = PageLockedMemory::<i32>::zeros(ctx.clone(), 12);
/// mem.set(1234);
/// for &val in mem.as_slice() {
///   assert_eq!(val, 1234);
/// }
/// ```
pub trait Memset: Memory {
    fn set(&mut self, value: Self::Elem);
}

/// Allocatable memories with CUDA context
pub trait Allocatable: Contexted + Memset + Sized {
    /// Shape for initialization
    type Shape: Zero;

    /// Allocate a memory without initialization
    ///
    /// Safety
    /// ------
    /// - Cause undefined behavior when read before write
    ///
    /// Panic
    /// ------
    /// - if shape is zero
    unsafe fn uninitialized(ctx: Arc<Context>, shape: Self::Shape) -> Self;

    /// uniformly initialized
    ///
    /// Panic
    /// ------
    /// - if shape is zero
    fn from_elem(ctx: Arc<Context>, shape: Self::Shape, elem: Self::Elem) -> Self {
        let mut mem = unsafe { Self::uninitialized(ctx, shape) };
        mem.set(elem);
        mem
    }

    /// uniformly initialized by zero
    ///
    /// Panic
    /// ------
    /// - if shape is zero
    fn zeros(ctx: Arc<Context>, shape: Self::Shape) -> Self {
        Self::from_elem(ctx, shape, <Self::Elem as Zero>::zero())
    }
}

/// Memory which has continuous 1D index, i.e. can be treated as a Rust slice
pub trait Continuous: Memory {
    fn as_slice(&self) -> &[Self::Elem];
    fn as_mut_slice(&mut self) -> &mut [Self::Elem];
}

/// Memory which is managed under the CUDA unified memory management systems
pub trait Managed: Memory {
    fn buffer_id(&self) -> u64 {
        get_attr(
            self.head_addr(),
            CUpointer_attribute::CU_POINTER_ATTRIBUTE_BUFFER_ID,
        )
        .expect("Not managed by CUDA")
    }
}
