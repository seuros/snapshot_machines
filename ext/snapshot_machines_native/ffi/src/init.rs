//! Private initialization boundary.
use crate::{Catalog, classes, values};
use magnus::{Error, Ruby, function, method, prelude::*};
use matryona_machines::ruby::Class;

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("SnapshotMachines")?;
    classes::define(ruby, module)?;
    // Validating hooks behind every value class; private
    // once `lib/snapshot_machines.rb` has loaded this extension.
    module.define_singleton_method("__snapshot_id", function!(values::checked_id, 1))?;
    module.define_singleton_method("__scope_of", function!(values::id_scope, 1))?;
    module.define_singleton_method("__pending", function!(values::checked_pending, 2))?;
    module.define_singleton_method("__manifest", function!(values::checked_manifest, 3))?;
    module.define_singleton_method(
        "__holds_manifest",
        function!(values::checked_holds_manifest, 1),
    )?;
    let class = module.define_class("Catalog", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Catalog::new, 0))?;
    class.define_singleton_method("with_scope", function!(Catalog::with_scope, 1))?;
    class.define_singleton_method("restored", function!(Catalog::restored, 3))?;
    class.define_method("scope", method!(Catalog::scope, 0))?;
    class.define_method("issued", method!(Catalog::issued, 0))?;
    class.define_method("capacity", method!(Catalog::capacity, 0))?;
    class.define_method("begin", method!(Catalog::begin, 1))?;
    class.define_method("commit", method!(Catalog::commit, 2))?;
    class.define_method("abort", method!(Catalog::abort, 1))?;
    class.define_method("pending", method!(Catalog::pending, 0))?;
    class.define_method("latest", method!(Catalog::latest, 0))?;
    class.define_method("restore_plan", method!(Catalog::restore_plan, 1))?;
    // `new`, `with_scope` and `restored` answer a Catalog: its class is fixed
    // now, before any of them runs.
    Class::pin_wrapped::<Catalog>(ruby, class).map(drop)
}
