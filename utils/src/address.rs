use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Addr<T, A>(*mut T, PhantomData<A>);
// it is guaranteed that the size of this is the same as pointer type.

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum PhysicalAddr {}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum VirtualAddr {}

pub type PAddr<T> = Addr<T, PhysicalAddr>;
pub type VAddr<T> = Addr<T, PhysicalAddr>;

impl<T, A> Addr<T, A> {
    #[inline]
    pub fn from(ptr: *mut T) -> Self {
        debug_assert_eq!(
            (ptr as usize) % core::mem::align_of::<T>(),
            0,
            "address must be aligned properly"
        );
        Self(ptr, PhantomData)
    }
    #[inline]
    pub fn from_raw(raw_addr: usize) -> Self {
        debug_assert_eq!(
            raw_addr % core::mem::align_of::<T>(),
            0,
            "address must be aligned properly"
        );
        Self(raw_addr as *mut T, PhantomData)
    }
    #[inline]
    pub fn cast<U>(self) -> Addr<U, A> {
        debug_assert_eq!(
            (self.0 as usize) % core::mem::align_of::<U>(),
            0,
            "address must be aligned properly"
        );
        Addr::from(self.0 as *mut U)
    }
    #[inline]
    pub fn ptr(self) -> *const T {
        self.0
    }
    #[inline]
    pub fn mut_ptr(self) -> *mut T {
        self.0
    }
    #[inline]
    pub fn raw(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn round_up(self, align: usize) -> Self {
        debug_assert!(
            align.is_power_of_two(),
            "align (= {}) is not pow of 2",
            align
        );
        let raw = self.raw();
        let tmp = align.wrapping_sub(1);
        Self::from_raw(raw.wrapping_add(tmp) & !tmp)
    }
    #[inline]
    pub fn round_down(self, align: usize) -> Self {
        debug_assert!(
            align.is_power_of_two(),
            "align (= {}) is not pow of 2",
            align
        );
        let raw = self.raw();
        Self::from_raw(raw & !align.wrapping_sub(1))
    }
}

impl<T, A> Add<usize> for Addr<T, A> {
    type Output = Self;
    fn add(self, offset: usize) -> Self::Output {
        Self::from(unsafe { self.0.add(offset) })
    }
}
impl<T, A> AddAssign<usize> for Addr<T, A> {
    fn add_assign(&mut self, offset: usize) {
        *self = *self + offset;
    }
}
impl<T, A> Sub<usize> for Addr<T, A> {
    type Output = Self;
    fn sub(self, offset: usize) -> Self::Output {
        Self::from(unsafe { self.0.sub(offset) })
    }
}
impl<T, A> SubAssign<usize> for Addr<T, A> {
    fn sub_assign(&mut self, offset: usize) {
        *self = *self - offset;
    }
}

impl<T, A> Clone for Addr<T, A> {
    fn clone(&self) -> Self {
        Self::from(self.0)
    }
}
impl<T, A> Copy for Addr<T, A> {}
