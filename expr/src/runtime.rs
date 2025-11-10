use crate::error::TypeTag;
use crate::value::{EnumInfo, LatticeOps, OrderInfo, SemiringOps, Value};

pub trait Env {
    fn get_ident(&self, name: &str) -> Option<Value>;
    fn get_field(&self, base: &Value, field: &str) -> Option<Value>;

    fn enum_info(&self, type_id: &str) -> Option<EnumInfo>;
    fn default_order(&self, type_id: &str) -> Option<OrderInfo>;
    fn named_order(&self, order_id: &str) -> Option<OrderInfo>;

    fn semiring(&self, _name: &str) -> Option<SemiringOps> {
        None
    }

    fn lattice_for(&self, _ty: &TypeTag) -> Option<LatticeOps> {
        None
    }

    fn finite_set(&self, _name: &str) -> Option<Vec<Value>> {
        None
    }

    fn current_semiring(&self) -> Option<SemiringOps> {
        None
    }
}
