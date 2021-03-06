use super::*;
use std::ffi::c_void;

/// Determine actual memory type dynamically
///
/// Because `Continuous` memories can be treated as a slice,
/// input slice may represents any type of memory.
fn memory_type<T>(ptr: *const T) -> MemoryType {
    match get_attr(ptr, CUpointer_attribute::CU_POINTER_ATTRIBUTE_MEMORY_TYPE) {
        Ok(CUmemorytype_enum::CU_MEMORYTYPE_HOST) => MemoryType::PageLocked,
        Ok(CUmemorytype_enum::CU_MEMORYTYPE_DEVICE) => MemoryType::Device,
        Ok(CUmemorytype_enum::CU_MEMORYTYPE_ARRAY) => MemoryType::Array,
        _ => MemoryType::Host,
    }
}

impl<T: Scalar> Memory for [T] {
    type Elem = T;
    fn head_addr(&self) -> *const T {
        self.as_ptr()
    }

    fn head_addr_mut(&mut self) -> *mut T {
        self.as_mut_ptr()
    }

    fn num_elem(&self) -> usize {
        self.len()
    }

    fn memory_type(&self) -> MemoryType {
        memory_type(self.as_ptr())
    }
}

impl<T: Scalar> Memcpy<PageLockedMemory<T>> for [T] {
    fn copy_from(&mut self, src: &PageLockedMemory<T>) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match self.memory_type() {
            // H -> H
            MemoryType::Host | MemoryType::PageLocked => self.copy_from(src),
            // H -> D
            MemoryType::Device => unsafe {
                contexted_call!(
                    &src.get_context(),
                    cuMemcpyHtoD_v2,
                    self.head_addr_mut() as CUdeviceptr,
                    src.as_ptr() as *mut c_void,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Page-locked memory to Device failed"),
            // H -> A
            MemoryType::Array => {
                unimplemented!("Dynamical cast from slice to CUDA Array is not supported yet")
            }
        }
    }
}

impl<T: Scalar> Memcpy<[T]> for PageLockedMemory<T> {
    fn copy_from(&mut self, src: &[T]) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match src.memory_type() {
            // H -> H
            MemoryType::Host | MemoryType::PageLocked => self.copy_from_slice(src),
            // D -> H
            MemoryType::Device => unsafe {
                contexted_call!(
                    &self.get_context(),
                    cuMemcpyDtoH_v2,
                    self.head_addr_mut() as *mut c_void,
                    src.as_ptr() as CUdeviceptr,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Device to Page-locked memory failed"),
            // A -> H
            MemoryType::Array => unreachable!("Array cannot be casted to a slice"),
        }
    }
}

impl<T: Scalar> Memcpy<RegisteredMemory<'_, T>> for [T] {
    fn copy_from(&mut self, src: &RegisteredMemory<'_, T>) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match self.memory_type() {
            // H -> H
            MemoryType::Host | MemoryType::PageLocked => self.copy_from(src),
            // H -> D
            MemoryType::Device => unsafe {
                contexted_call!(
                    &src.get_context(),
                    cuMemcpyHtoD_v2,
                    self.head_addr_mut() as CUdeviceptr,
                    src.as_ptr() as *mut c_void,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from registered host memory to Device failed"),
            // H -> A
            MemoryType::Array => {
                unimplemented!("Dynamical cast from slice to CUDA Array is not supported yet")
            }
        }
    }
}

impl<T: Scalar> Memcpy<[T]> for RegisteredMemory<'_, T> {
    fn copy_from(&mut self, src: &[T]) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match src.memory_type() {
            // H -> H
            MemoryType::Host | MemoryType::PageLocked => self.copy_from_slice(src),
            // D -> H
            MemoryType::Device => unsafe {
                contexted_call!(
                    &self.get_context(),
                    cuMemcpyDtoH_v2,
                    self.head_addr_mut() as *mut c_void,
                    src.as_ptr() as CUdeviceptr,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Device to registered host memory failed"),
            // A -> H
            MemoryType::Array => unreachable!("Array cannot be casted to a slice"),
        }
    }
}

impl<T: Scalar> Memcpy<DeviceMemory<T>> for [T] {
    fn copy_from(&mut self, src: &DeviceMemory<T>) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match self.memory_type() {
            // D -> H
            MemoryType::Host | MemoryType::PageLocked => unsafe {
                contexted_call!(
                    &src.get_context(),
                    cuMemcpyDtoH_v2,
                    self.head_addr_mut() as *mut c_void,
                    src.as_ptr() as CUdeviceptr,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Device to Host memory failed"),
            // D -> D
            MemoryType::Device => unsafe {
                contexted_call!(
                    &src.get_context(),
                    cuMemcpyDtoD_v2,
                    self.head_addr_mut() as CUdeviceptr,
                    src.as_ptr() as CUdeviceptr,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Page-locked memory to Device failed"),
            // D -> A
            MemoryType::Array => {
                unimplemented!("Dynamical cast from slice to CUDA Array is not supported yet")
            }
        }
    }
}

impl<T: Scalar> Memcpy<[T]> for DeviceMemory<T> {
    fn copy_from(&mut self, src: &[T]) {
        assert_ne!(self.head_addr(), src.head_addr());
        assert_eq!(self.num_elem(), src.num_elem());
        match src.memory_type() {
            // H -> D
            MemoryType::Host | MemoryType::PageLocked => unsafe {
                contexted_call!(
                    &self.get_context(),
                    cuMemcpyHtoD_v2,
                    self.head_addr_mut() as CUdeviceptr,
                    src.as_ptr() as *mut c_void,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Page-locked memory to Device failed"),
            // D -> D
            MemoryType::Device => unsafe {
                contexted_call!(
                    &self.get_context(),
                    cuMemcpyDtoD_v2,
                    self.head_addr_mut() as CUdeviceptr,
                    src.as_ptr() as CUdeviceptr,
                    self.num_elem() * T::size_of()
                )
            }
            .expect("memcpy from Device to Page-locked memory failed"),
            // A -> D
            MemoryType::Array => unreachable!("Array cannot be casted to a slice"),
        }
    }
}

impl<T: Scalar> Continuous for [T] {
    fn as_slice(&self) -> &[Self::Elem] {
        self
    }

    fn as_mut_slice(&mut self) -> &mut [Self::Elem] {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_for_slice() -> error::Result<()> {
        let a = vec![0_u32; 12];
        assert!(matches!(a.as_slice().memory_type(), MemoryType::Host));
        assert_eq!(a.as_slice().num_elem(), 12);
        Ok(())
    }
}
