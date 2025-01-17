use super::*;

use crate::ast::builder::BuilderEnum;
use crate::ast::builder::IRBuilder;
use crate::ast::ctx::Ctx;
use crate::ast::diag::{ErrorCode, WarnCode};
use crate::format_label;

use internal_macro::node;
use lsp_types::SemanticTokenType;
#[node(comment)]
pub struct DefNode {
    pub var: VarNode,
    pub tp: Option<Box<TypeNodeEnum>>,
    pub exp: Option<Box<NodeEnum>>,
}

impl PrintTrait for DefNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("DefNode");
        self.var.print(tabs + 1, false, line.clone());
        if let Some(tp) = &self.tp {
            tp.print(tabs + 1, true, line.clone());
        } else {
            self.exp
                .as_ref()
                .unwrap()
                .print(tabs + 1, true, line.clone());
        }
    }
}

impl Node for DefNode {
    fn emit<'a, 'ctx, 'b>(
        &mut self,
        ctx: &'b mut Ctx<'a>,
        builder: &'b BuilderEnum<'a, 'ctx>,
    ) -> NodeResult {
        let range = self.range();
        ctx.push_semantic_token(self.var.range, SemanticTokenType::VARIABLE, 0);
        if self.exp.is_none() && self.tp.is_none() {
            return Err(ctx.add_diag(self.range.new_err(ErrorCode::UNDEFINED_TYPE)));
        }
        let mut pltype = None;
        let mut expv = None;
        if let Some(tp) = &self.tp {
            tp.emit_highlight(ctx);
            pltype = Some(tp.get_type(ctx, builder)?);
        }
        if let Some(exp) = &mut self.exp {
            let (value, pltype_opt, _) =
                ctx.emit_with_expectation(exp, pltype.clone(), self.var.range(), builder)?;
            // for err tolerate
            if pltype_opt.is_none() {
                return Err(ctx.add_diag(self.range.new_err(ErrorCode::UNDEFINED_TYPE)));
            }
            if value.is_none() {
                return Err(ctx.add_diag(self.range.new_err(ErrorCode::EXPECT_VALUE)));
            }
            let tp = pltype_opt.clone().unwrap();
            if pltype.is_none() {
                ctx.push_type_hints(self.var.range, tp.clone());
                pltype = Some(tp);
            }
            expv = value;
        }
        let pltype = pltype.unwrap();
        let ptr2value = builder.alloc(
            &self.var.name,
            &pltype.borrow(),
            ctx,
            Some(self.var.range.start),
        );
        ctx.add_symbol(
            self.var.name.clone(),
            ptr2value,
            pltype,
            self.var.range,
            false,
        )?;
        if let Some(exp) = expv {
            builder.build_dbg_location(self.var.range.start);
            builder.build_store(ptr2value, ctx.try_load2var(range, exp, builder)?);
        }
        Ok((None, None, TerminatorEnum::NONE))
    }
}
#[node]
pub struct AssignNode {
    pub var: Box<NodeEnum>,
    pub exp: Box<NodeEnum>,
}

impl PrintTrait for AssignNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("AssignNode");
        self.var.print(tabs + 1, false, line.clone());
        self.exp.print(tabs + 1, true, line.clone());
    }
}

impl Node for AssignNode {
    fn emit<'a, 'ctx, 'b>(
        &mut self,
        ctx: &'b mut Ctx<'a>,
        builder: &'b BuilderEnum<'a, 'ctx>,
    ) -> NodeResult {
        let exp_range = self.exp.range();
        let (ptr, lpltype, _) = self.var.emit(ctx, builder)?;
        if lpltype.is_none() {
            return Err(ctx.add_diag(self.var.range().new_err(ErrorCode::NOT_ASSIGNABLE)));
        }
        let (value, _, _) =
            ctx.emit_with_expectation(&mut self.exp, lpltype, self.var.range(), builder)?;
        if ptr.as_ref().unwrap().is_const {
            return Err(ctx.add_diag(self.range.new_err(ErrorCode::ASSIGN_CONST)));
        }
        let load = ctx.try_load2var(exp_range, value.unwrap(), builder)?;
        builder.build_store(ptr.unwrap().value, load);
        Ok((None, None, TerminatorEnum::NONE))
    }
}

#[node(comment)]
pub struct EmptyNode {}

impl PrintTrait for EmptyNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("EmptyNode");
    }
}

impl Node for EmptyNode {
    fn emit<'a, 'ctx, 'b>(
        &mut self,
        ctx: &'b mut Ctx<'a>,
        _builder: &'b BuilderEnum<'a, 'ctx>,
    ) -> NodeResult {
        ctx.emit_comment_highlight(&self.comments[0]);
        Ok((None, None, TerminatorEnum::NONE))
    }
}

#[node]
pub struct StatementsNode {
    pub statements: Vec<Box<NodeEnum>>,
}

impl PrintTrait for StatementsNode {
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
}

impl Node for StatementsNode {
    fn emit<'a, 'ctx, 'b>(
        &mut self,
        ctx: &'b mut Ctx<'a>,
        builder: &'b BuilderEnum<'a, 'ctx>,
    ) -> NodeResult {
        let mut terminator = TerminatorEnum::NONE;
        for m in self.statements.iter_mut() {
            if let NodeEnum::Empty(_) = **m {
                continue;
            }
            if !terminator.is_none() {
                if let NodeEnum::Comment(c) = &**m {
                    ctx.push_semantic_token(c.range, SemanticTokenType::COMMENT, 0);
                    continue;
                }
                ctx.add_diag(
                    m.range()
                        .new_warn(WarnCode::UNREACHABLE_STATEMENT)
                        .add_help(
                            "This statement will never be executed, because the previous \
                            statements contains a terminator. Try to remove it.",
                        )
                        .clone(),
                );
                continue;
            }
            let pos = m.range().start;
            builder.build_dbg_location(pos);
            let re = m.emit(ctx, builder);
            if re.is_err() {
                continue;
            }
            let (_, _, terminator_res) = re.unwrap();
            terminator = terminator_res;
        }
        for (v, (_, _, range, refs)) in &ctx.table {
            if refs.borrow().len() <= 1 && v != "self" {
                range
                    .new_warn(WarnCode::UNUSED_VARIABLE)
                    .add_label(
                        *range,
                        ctx.get_file(),
                        format_label!("Unused variable `{}`", v),
                    )
                    .add_to_ctx(ctx);
            }
        }
        Ok((None, None, terminator))
    }
}

impl StatementsNode {
    pub fn emit_child<'a, 'ctx, 'b>(
        &mut self,
        ctx: &'b mut Ctx<'a>,
        builder: &'b BuilderEnum<'a, 'ctx>,
    ) -> NodeResult {
        let child = &mut ctx.new_child(self.range.start, builder);
        self.emit(child, builder)
    }
}
