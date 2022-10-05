use super::primary::*;
use super::*;
use crate::ast::ctx::Ctx;
use inkwell::debug_info::*;
use internal_macro::range;

#[range]
pub struct DefNode {
    pub var: VarNode,
    pub exp: Box<dyn Node>,
}
impl Node for DefNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("DefNode");
        self.var.print(tabs + 1, false, line.clone());
        self.exp.print(tabs + 1, true, line.clone());
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> (Value<'ctx>, Option<String>) {
        let (v, pltype) = self.exp.emit(ctx);
        let e = ctx.try_load(v).as_basic_value_enum();
        let tp = e.get_type();
        let p = alloc(ctx, tp, &self.var.name);
        ctx.builder.build_store(p, e);
        let pltype = pltype.unwrap();
        let (_, ditype) = ctx.get_type(pltype.as_str()).unwrap();
        ctx.dibuilder.create_auto_variable(
            ctx.discope,
            &self.var.name,
            ctx.diunit.get_file(),
            self.var.range().start.line as u32,
            *ditype,
            true,
            DIFlags::PUBLIC,
            ditype.get_align_in_bits(),
        );
        ctx.add_symbol(self.var.name.clone(), p, pltype.clone());
        (Value::None, Some(pltype))
    }
}
#[range]
pub struct AssignNode {
    pub var: Box<dyn Node>,
    pub exp: Box<dyn Node>,
}
impl Node for AssignNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("AssignNode");
        self.var.print(tabs + 1, false, line.clone());
        self.exp.print(tabs + 1, true, line.clone());
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> (Value<'ctx>, Option<String>) {
        let (pt, _) = self.var.emit(ctx);
        let (value, _) = self.exp.emit(ctx);
        if let Value::VarValue(ptr) = pt {
            let load = ctx.try_load(value);
            ctx.builder.build_store(ptr, load.as_basic_value_enum());
            return (Value::None, None);
        }
        todo!()
    }
}

#[range]
pub struct NLNode {}

impl Node for NLNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("NLNode");
    }
    fn emit<'a, 'ctx>(&'a mut self, _: &mut Ctx<'a, 'ctx>) -> (Value<'ctx>, Option<String>) {
        (Value::None, None)
    }
}

#[range]
pub struct StatementsNode {
    pub statements: Vec<Box<dyn Node>>,
}
impl Node for StatementsNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("StatementsNode");
        let mut i = self.statements.len();
        for statement in &self.statements {
            i -= 1;
            statement.print(tabs + 1, i == 0, line.clone());
        }
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> (Value<'ctx>, Option<String>) {
        let child = &mut ctx.new_child(self.range.start);
        for m in self.statements.iter_mut() {
            let pos = m.range().start;
            child.build_dbg_location(pos);
            m.emit(child);
        }
        (Value::None, None)
    }
}

impl StatementsNode {
    pub fn emit_child<'a, 'ctx>(
        &'a mut self,
        ctx: &mut Ctx<'a, 'ctx>,
    ) -> (Value<'ctx>, Option<String>) {
        let child = ctx;
        for m in self.statements.iter_mut() {
            let pos = m.range().start;
            child.build_dbg_location(pos);
            m.emit(child);
        }
        (Value::None, None)
    }
}
