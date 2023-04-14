use crate::render::chart::Chart;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Project {
    pub charts_by_name: HashMap<String, Rc<Chart>>,
    pub cuts: Vec<Cut>,
}

pub struct Cut {
    pub chart: Rc<Chart>,
    pub start_time: f32,
    pub end_time: f32,
    pub offset: f32,
}
