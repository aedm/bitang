use std::ops::Deref;
use std::rc::Rc;

pub mod controls;
pub mod spline;

pub struct RcHashRef<T>(Rc<T>);

impl<T> std::hash::Hash for RcHashRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.0.as_ref(), state);
    }
}

impl<T> PartialEq for RcHashRef<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ref(), other.0.as_ref())
    }
}

impl<T> Eq for RcHashRef<T> {}

impl<T> Deref for RcHashRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
