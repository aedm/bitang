use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub mod controls;
pub mod spline;

#[allow(dead_code)]
pub struct ArcHashRef<T>(Arc<T>);

impl<T> std::hash::Hash for ArcHashRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.0.as_ref(), state);
    }
}

impl<T> PartialEq for ArcHashRef<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0.as_ref(), other.0.as_ref())
    }
}

impl<T> Eq for ArcHashRef<T> {}

impl<T> Deref for ArcHashRef<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum ControlIdPartType {
    Chart,
    ChartValues,
    Camera,
    Object,
    Scene,
    Compute,
    Value,
    ChartStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ControlIdPart {
    pub part_type: ControlIdPartType,
    pub name: String,
}

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq, Debug, Clone, Default, PartialOrd, Ord)]
pub struct ControlId {
    pub parts: Vec<ControlIdPart>,
}

impl ControlId {
    pub fn add(&self, part_type: ControlIdPartType, part_name: &str) -> Self {
        let mut parts = self.parts.clone();
        parts.push(ControlIdPart {
            part_type,
            name: part_name.to_string(),
        });
        Self { parts }
    }

    pub fn prefix(&self, length: usize) -> Self {
        assert!(length <= self.parts.len());
        Self {
            parts: self.parts[..length].to_vec(),
        }
    }
}

impl Display for ControlId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                write!(f, ".")?;
            }
            write!(f, "{:?}:{}", part.part_type, part.name)?;
        }
        Ok(())
    }
}
