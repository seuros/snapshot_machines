//! Private initialization boundary.
use magnus::{Error, Ruby, function, method, prelude::*};

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("SnapshotMachines")?;
    let pending = module.define_class("Pending", ruby.class_object())?;
    pending.undef_default_alloc_func();
    pending.define_method("id", method!(crate::Attempt::id, 0))?;
    let class = module.define_class("Catalog", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(crate::Wrapper::new, 0))?;
    class.define_method("capacity", method!(crate::Wrapper::capacity, 0))?;
    class.define_method("begin", method!(crate::Wrapper::begin, 1))?;
    class.define_method("commit", method!(crate::Wrapper::commit, 2))?;
    class.define_method("abort", method!(crate::Wrapper::abort, 1))?;
    class.define_method("latest", method!(crate::Wrapper::latest, 0))?;
    class.define_method("restore_plan", method!(crate::Wrapper::restore_plan, 1))
}
