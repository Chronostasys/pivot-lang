use super::*;
use crate::ast::ctx::Ctx;

use internal_macro::range;

#[range]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct BoolConstNode {
    pub value: bool,
}

impl Node for BoolConstNode {
    fn print(&self) {
        println!("BoolConstNode:");
        println!("{:?}", self.value)
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> Value<'ctx> {
        Value::BoolValue(ctx.context.bool_type().const_int(self.value as u64, true))
    }
}

#[range]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct NumNode {
    pub value: Num,
}
impl Node for NumNode {
    fn print(&self) {
        println!("NumNode:");
        println!("{:?}", self.value)
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> Value<'ctx> {
        if let Num::INT(x) = self.value {
            let b = ctx.context.i64_type().const_int(x, true);
            return Value::IntValue(b);
        } else if let Num::FLOAT(x) = self.value {
            let b = ctx.context.f64_type().const_float(x);
            return Value::FloatValue(b);
        }
        panic!("not implemented")
    }
}

#[range]
#[derive(Debug, PartialEq, Clone)]
pub struct VarNode {
    pub name: String,
}
impl Node for VarNode {
    fn print(&self) {
        println!("VarNode:");
        println!("{}", self.name)
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> Value<'ctx> {
        let v = ctx.get_symbol(&self.name);
        if let Some(v) = v {
            return Value::VarValue(*v);
        }
        todo!(); // TODO: 未定义的变量
    }
}