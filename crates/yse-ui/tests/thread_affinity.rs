use static_assertions::assert_not_impl_any;
use yse_model::{Owner, Var};
use yse_ui::{LineEdit, Window};

assert_not_impl_any!(Owner: Send, Sync);
assert_not_impl_any!(Var<String>: Send, Sync);
assert_not_impl_any!(Window: Send, Sync);
assert_not_impl_any!(LineEdit: Send, Sync);
