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

    fn lattice_ops(&self, _type_id: &str) -> Option<LatticeOps> {
        None
    }

    fn capture_snapshot(&self) -> Vec<(String, Value)> {
        Vec::new()
    }

    fn current_semiring(&self) -> Option<SemiringOps> {
        None
    }
}
