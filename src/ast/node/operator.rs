use super::primary::VarNode;
use super::*;
use crate::ast::ctx::Ctx;
use crate::ast::ctx::PLType;
use crate::ast::tokens::TokenType;

use crate::handle_calc;
use crate::lsp::diagnostics::send_completions;
use inkwell::IntPredicate;
use internal_macro::range;
use lsp_types::CompletionItem;
use lsp_types::CompletionItemKind;
use lsp_types::InsertTextFormat;
use paste::item;

#[range]
pub struct UnaryOpNode {
    pub op: TokenType,
    pub exp: Box<dyn Node>,
}
impl Node for UnaryOpNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("UnaryOpNode");
        tab(tabs + 1, line.clone(), end);
        println!("{:?}", self.op);
        self.exp.print(tabs + 1, true, line.clone());
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> NodeResult<'ctx> {
        let (exp, pltype) = self.exp.emit(ctx)?;
        let exp = ctx.try_load(exp);
        return Ok(match (exp, self.op) {
            (Value::IntValue(exp), TokenType::MINUS) => (
                Value::IntValue(ctx.builder.build_int_neg(exp, "negtmp")),
                pltype,
            ),
            (Value::FloatValue(exp), TokenType::MINUS) => (
                Value::FloatValue(ctx.builder.build_float_neg(exp, "negtmp")),
                pltype,
            ),
            (Value::BoolValue(exp), TokenType::NOT) => (
                Value::BoolValue(ctx.builder.build_int_compare(
                    IntPredicate::EQ,
                    exp,
                    ctx.context.bool_type().const_int(false as u64, true),
                    "nottmp",
                )),
                pltype,
            ),
            (_exp, _op) => {
                return Err(ctx.add_err(
                    self.range,
                    crate::ast::error::ErrorCode::INVALID_UNARY_EXPRESSION,
                ));
            }
        });
    }
}

#[range]
pub struct BinOpNode {
    pub left: Box<dyn Node>,
    pub op: TokenType,
    pub right: Box<dyn Node>,
}
impl Node for BinOpNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("BinOpNode");
        self.left.print(tabs + 1, false, line.clone());
        tab(tabs + 1, line.clone(), false);
        println!("{:?}", self.op);
        self.right.print(tabs + 1, true, line.clone());
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> NodeResult<'ctx> {
        let (lv, lpltype) = self.left.emit(ctx)?;
        let left = ctx.try_load(lv);
        let (rv, rpltype) = self.right.emit(ctx)?;
        let right = ctx.try_load(rv);
        if lpltype != rpltype {
            return Err(ctx.add_err(
                self.range,
                crate::ast::error::ErrorCode::BIN_OP_TYPE_MISMATCH,
            ));
        }
        Ok(match self.op {
            TokenType::PLUS => handle_calc!(ctx, add, float_add, left, right, self.range),
            TokenType::MINUS => handle_calc!(ctx, sub, float_sub, left, right, self.range),
            TokenType::MUL => handle_calc!(ctx, mul, float_mul, left, right, self.range),
            TokenType::DIV => handle_calc!(ctx, signed_div, float_div, left, right, self.range),
            TokenType::EQ
            | TokenType::NE
            | TokenType::LEQ
            | TokenType::GEQ
            | TokenType::GREATER
            | TokenType::LESS => match (left, right) {
                (Value::IntValue(lhs), Value::IntValue(rhs)) => (
                    Value::BoolValue(ctx.builder.build_int_compare(
                        self.op.get_op(),
                        lhs,
                        rhs,
                        "cmptmp",
                    )),
                    Some("bool".to_string()),
                ),
                (Value::FloatValue(lhs), Value::FloatValue(rhs)) => (
                    Value::BoolValue(ctx.builder.build_float_compare(
                        self.op.get_fop(),
                        lhs,
                        rhs,
                        "cmptmp",
                    )),
                    Some("bool".to_string()),
                ),
                _ => {
                    return Err(ctx.add_err(
                        self.range,
                        crate::ast::error::ErrorCode::VALUE_NOT_COMPARABLE,
                    ))
                }
            },
            TokenType::AND => match (left, right) {
                (Value::BoolValue(lhs), Value::BoolValue(rhs)) => (
                    Value::BoolValue(ctx.builder.build_and(lhs, rhs, "andtmp")),
                    Some("bool".to_string()),
                ),
                _ => {
                    return Err(
                        ctx.add_err(self.range, crate::ast::error::ErrorCode::LOGIC_OP_NOT_BOOL)
                    )
                }
            },
            TokenType::OR => match (left, right) {
                (Value::BoolValue(lhs), Value::BoolValue(rhs)) => (
                    Value::BoolValue(ctx.builder.build_or(lhs, rhs, "ortmp")),
                    Some("bool".to_string()),
                ),
                _ => {
                    return Err(
                        ctx.add_err(self.range, crate::ast::error::ErrorCode::LOGIC_OP_NOT_BOOL)
                    )
                }
            },
            _ => {
                return Err(ctx.add_err(
                    self.range,
                    crate::ast::error::ErrorCode::UNRECOGNIZED_BIN_OPERATOR,
                ))
            }
        })
    }
}

#[range]
pub struct TakeOpNode {
    pub head: Box<dyn Node>,
    pub ids: Vec<Box<VarNode>>,
    pub complete: bool,
}

impl Node for TakeOpNode {
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("TakeOpNode");
        let mut i = self.ids.len();
        self.head.print(tabs + 1, i == 0, line.clone());
        for id in &self.ids {
            i -= 1;
            id.print(tabs + 1, i == 0, line.clone());
        }
    }
    fn emit<'a, 'ctx>(&'a mut self, ctx: &mut Ctx<'a, 'ctx>) -> NodeResult<'ctx> {
        let head = self.head.emit(ctx)?;
        // let head = ctx.try_load(head);
        let (mut res, mut pltype) = head;
        for id in &self.ids {
            res = match res.as_basic_value_enum() {
                BasicValueEnum::PointerValue(s) => {
                    let etype = s.get_type().get_element_type();
                    let index;
                    if etype.is_struct_type() {
                        let st = etype.into_struct_type();
                        let tpname = st.get_name().unwrap().to_str().unwrap();
                        let (tp, _) = ctx.get_type(tpname, self.range).unwrap();
                        if let PLType::STRUCT(s) = tp {
                            let field = s.fields.get(&id.name);
                            if let Some(field) = field {
                                index = field.index;
                                pltype = Some(field.typename.id.clone());
                            } else {
                                return Err(ctx.add_err(
                                    id.range,
                                    crate::ast::error::ErrorCode::STRUCT_FIELD_NOT_FOUND,
                                ));
                            }
                        } else {
                            panic!("not implemented");
                        }
                    } else {
                        return Err(
                            ctx.add_err(id.range, crate::ast::error::ErrorCode::INVALID_GET_FIELD)
                        );
                    }
                    Value::VarValue(ctx.builder.build_struct_gep(s, index, "structgep").unwrap())
                }
                _ => panic!("not implemented {:?}", res),
            }
        }
        if !self.complete {
            // end with ".", gen completions
            eprintln!("completions");
            if let Some(sender) = ctx.sender {
                if let Some((pos, id)) = &ctx.completion {
                    if pos.line == self.range.end.line {
                        let (tp, _) = ctx.get_type(&pltype.unwrap(), self.range).unwrap();
                        if let PLType::STRUCT(s) = tp {
                            let mut completions = Vec::new();
                            for (name, _) in &s.fields {
                                completions.push(CompletionItem {
                                    kind: Some(CompletionItemKind::FIELD),
                                    label: name.clone(),
                                    detail: Some("field".to_string()),
                                    insert_text: Some(name.clone()),
                                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                                    ..Default::default()
                                });
                            }
                            eprintln!("completions: {:?}", completions);
                            send_completions(sender, id.clone(), completions);
                            eprintln!("sent completions");
                        }
                    }
                }
            }

            return Err(ctx.add_err(self.range, crate::ast::error::ErrorCode::COMPLETION));
        }
        Ok((res, pltype))
    }
}
