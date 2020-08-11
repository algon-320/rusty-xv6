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
    pub const fn from(ptr: *mut T) -> Self {
        Self(ptr, PhantomData)
    }
    #[inline]
    pub fn cast<U>(self) -> Addr<U, A> {
        assert_eq!((self.0 as usize) % core::mem::align_of::<U>(), 0);
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
