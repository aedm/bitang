use std::rc::Rc;

use anyhow::Result;

use super::{ComputeCall, ComputePassContext, DoubleBuffer, GpuContext, Shader};

// TODO: rename to "stage" or similar
pub enum Run {
    Init(Rc<DoubleBuffer>),
    Simulate(Rc<DoubleBuffer>),
}

/// Represents a compute step in the chart sequence.
pub struct Compute {
    pub id: String,
    pub run: Run,
    compute_call: ComputeCall,
}

impl Compute {
    pub fn new(context: &GpuContext, id: &str, shader: Shader, run: Run) -> Result<Compute> {
        let invocation_count = match &run {
            Run::Init(buffer) => buffer.item_count,
            Run::Simulate(buffer) => buffer.item_count,
        };
        let compute_call = ComputeCall::new(context, id, shader, invocation_count)?;
        Ok(Compute {
            id: id.to_string(),
            run,
            compute_call,
        })
    }

    pub fn execute(&self, context: &mut ComputePassContext<'_>) -> Result<()> {
        if let Run::Simulate(buffer) = &self.run {
            buffer.step();
        }
        self.compute_call.execute(context)
    }
}
