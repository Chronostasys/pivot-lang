use super::statement::StatementsNode;
use super::*;
use super::{alloc, types::TypedIdentifierNode, Node, TypeNode};
use crate::ast::diag::ErrorCode;
use crate::ast::node::{deal_line, tab};
use crate::ast::pltype::{FNType, PLType};
use crate::utils::read_config::enter;
use inkwell::debug_info::*;
use internal_macro::range;
use lsp_types::SemanticTokenType;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::fmt::format;
use std::rc::Rc;
use std::vec;
#[range]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FuncCallNode {
    pub id: Box<NodeEnum>,
    pub paralist: Vec<Box<NodeEnum>>,
}

impl Node for FuncCallNode {
    fn format(&self, tabs: usize, prefix: &str) -> String {
        let mut format_res = String::new();
        let mut param_str = String::new();
        match Some(&self.paralist) {
            Some(paralist) => {
                let mut len = 0;
                for param in paralist {
                    len += 1;
                    param_str.push_str(&param.format(tabs, prefix));
                    if len < paralist.len() {
                        param_str.push_str(", ");
                    }
                }
            }
            None => (),
        }
        format_res.push_str(&self.id.format(tabs, prefix));
        format_res.push_str("(");
        format_res.push_str(&param_str);
        format_res.push_str(")");
        format_res
    }
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("FuncCallNode");
        let mut i = self.paralist.len();
        self.id.print(tabs + 1, false, line.clone());
        for para in &self.paralist {
            i -= 1;
            para.print(tabs + 1, i == 0, line.clone());
        }
    }
    fn emit<'a, 'ctx>(&mut self, ctx: &mut Ctx<'a, 'ctx>) -> NodeResult<'ctx> {
        let id_range = self.id.range();
        let mut para_values = Vec::new();
        let (plvalue, pltype, _) = self.id.emit(ctx)?;
        if pltype.is_none() {
            return Err(ctx.add_err(self.range, ErrorCode::FUNCTION_NOT_FOUND));
        }
        let pltype = pltype.unwrap().clone();
        let mut fntype = match &*pltype.borrow() {
            PLType::FN(f) => f.clone(),
            _ => return Err(ctx.add_err(self.range, ErrorCode::FUNCTION_NOT_FOUND)),
        };
        fntype.clear_generic();
        let mut skip = 0;
        if plvalue.is_some() {
            if let Some(receiver) = plvalue.unwrap().receiver {
                para_values.push(receiver.into());
                skip = 1;
            }
            // funcvalue must use fntype to get a new one,can not use the return  plvalue of id node emit
            let funcvalue = fntype.get_or_insert_fn(ctx);
            if funcvalue.count_params() - skip != self.paralist.len() as u32 {
                return Err(ctx.add_err(self.range, ErrorCode::PARAMETER_LENGTH_NOT_MATCH));
            }
            let mut prevpos = id_range.end;
            for (i, para) in self.paralist.iter_mut().enumerate() {
                let sigrange = prevpos.to(para.range().end);
                prevpos = para.range().end;
                let pararange = para.range();
                ctx.push_param_hint(pararange.clone(), fntype.param_names[i].clone());
                ctx.set_if_sig(
                    sigrange,
                    fntype.name.clone().split("::").last().unwrap().to_string()
                        + "("
                        + fntype
                            .param_names
                            .iter()
                            .enumerate()
                            .map(|(i, s)| {
                                s.clone() + ": " + &fntype.param_pltypes[i].borrow().get_name()
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                            .as_str()
                        + ")",
                    &fntype.param_names,
                    i as u32 + skip,
                );
                let re = para.emit(ctx);
                if re.is_err() {
                    continue;
                }
                let (value, _, _) = re.unwrap();
                if value.is_none() {
                    continue;
                }
                let load = ctx.try_load2var(pararange, value.unwrap())?;
                let param = funcvalue.get_nth_param(i as u32 + skip).unwrap();
                if load.get_type() != param.get_type() {
                    _ = ctx.add_err(pararange, ErrorCode::PARAMETER_TYPE_NOT_MATCH);
                }
                para_values.push(load.as_basic_value_enum().into());
            }
            let ret = ctx.builder.build_call(
                funcvalue,
                &para_values,
                format(format_args!("call_{}", RefCell::borrow(&pltype).get_name())).as_str(),
            );
            ctx.save_if_comment_doc_hover(id_range, Some(fntype.doc.clone()));
            let res = match ret.try_as_basic_value().left() {
                Some(v) => Ok((
                    {
                        let ptr = alloc(ctx, v.get_type(), "ret_alloc_tmp");
                        ctx.nodebug_builder.build_store(ptr, v);
                        Some(ptr.into())
                    },
                    Some(fntype.ret_pltype),
                    TerminatorEnum::NONE,
                )),
                None => Ok((None, Some(fntype.ret_pltype), TerminatorEnum::NONE)),
            };
            ctx.set_if_refs_tp(pltype.clone(), id_range);
            ctx.send_if_go_to_def(id_range, fntype.range, ctx.plmod.path.clone());
            return res;
        }
        // funcvalue must use fntype to get a new one,can not use the return  plvalue of id node emit
        if fntype.param_pltypes.len() - skip as usize != self.paralist.len() {
            return Err(ctx.add_err(self.range, ErrorCode::PARAMETER_LENGTH_NOT_MATCH));
        }
        for (i, para) in self.paralist.iter_mut().enumerate() {
            if i < skip as usize {
                continue;
            }
            let pararange = para.range();
            let (value, value_pltype, _) = para.emit(ctx)?;
            if value.is_none() || value_pltype.is_none() {
                return Ok((None, None, TerminatorEnum::NONE));
            }
            let load = ctx.try_load2var(pararange, value.unwrap())?;
            let value_pltype = value_pltype.unwrap();
            if value_pltype != fntype.param_pltypes[i] {
                match &mut *fntype.param_pltypes[i].clone().borrow_mut() {
                    PLType::GENERIC(g) => {
                        g.set_type(value_pltype.clone());
                    }
                    _ => return Err(ctx.add_err(pararange, ErrorCode::PARAMETER_TYPE_NOT_MATCH)),
                }
            }
            para_values.push(load.as_basic_value_enum().into());
            ctx.push_param_hint(pararange.clone(), fntype.param_names[i].clone());
        }
        if fntype.need_gen_code() {
            let block = ctx.block;
            let (_, pltype, _) = fntype.node.emit(ctx)?;
            ctx.position_at_end(block.unwrap());
            let pltype = pltype.unwrap();
            match &*pltype.borrow() {
                PLType::FN(f) => {
                    fntype = f.clone();
                }
                _ => unreachable!(),
            };
        }
        let ret = ctx.builder.build_call(
            fntype.get_or_insert_fn(ctx),
            &para_values,
            format(format_args!("call_{}", RefCell::borrow(&pltype).get_name())).as_str(),
        );
        ctx.save_if_comment_doc_hover(id_range, Some(fntype.doc.clone()));
        let res = match ret.try_as_basic_value().left() {
            Some(v) => Ok((
                {
                    let ptr = alloc(ctx, v.get_type(), "ret_alloc_tmp");
                    ctx.nodebug_builder.build_store(ptr, v);
                    Some(ptr.into())
                },
                Some({
                    match &*fntype.ret_pltype.clone().borrow() {
                        PLType::GENERIC(g) => g.curpltype.as_ref().unwrap().clone(),
                        _ => fntype.ret_pltype.clone(),
                    }
                }),
                TerminatorEnum::NONE,
            )),
            None => Ok((None, Some(fntype.ret_pltype.clone()), TerminatorEnum::NONE)),
        };
        fntype.clear_generic();
        ctx.set_if_refs_tp(pltype.clone(), id_range);
        ctx.send_if_go_to_def(id_range, fntype.range, ctx.plmod.path.clone());
        return res;
    }
}
#[range]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FuncDefNode {
    pub id: Box<VarNode>,
    pub paralist: Vec<Box<TypedIdentifierNode>>,
    pub ret: Box<TypeNodeEnum>,
    pub doc: Vec<Box<NodeEnum>>,
    pub declare: bool,
    pub generics: Option<Box<GenericDefNode>>,
    pub body: Option<StatementsNode>,
}
impl FuncDefNode {
    pub fn emit_func_def<'a, 'ctx>(&mut self, ctx: &mut Ctx<'a, 'ctx>) -> Result<(), PLDiag> {
        if let Ok(_) = ctx.get_type(&self.id.name.as_str(), self.id.range) {
            return Err(ctx.add_err(self.range, ErrorCode::REDEFINE_SYMBOL));
        }
        let mut param_pltypes = Vec::new();
        let mut param_name = Vec::new();
        let mut method = false;
        let mut first = true;
        let mut generic_map = FxHashMap::default();
        if self.generics.is_some() {
            generic_map = self.generics.as_mut().unwrap().gen_generic_type(ctx)?;
        }
        let child = &mut ctx.tmp_child_ctx();
        for (name, pltype) in generic_map.iter() {
            child.add_type(
                name.clone(),
                pltype.clone(),
                pltype.clone().borrow().get_range().unwrap(),
            )?;
        }
        for para in self.paralist.iter() {
            let paramtype = para.tp.get_type(&child)?;
            if first && para.id.name == "self" {
                method = true;
            }
            first = false;
            param_pltypes.push(paramtype.clone());
            param_name.push(para.id.name.clone());
        }
        let refs = vec![];
        let ftp = FNType {
            name: self.id.name.clone(),
            ret_pltype: self.ret.get_type(&child)?,
            param_pltypes,
            param_names: param_name,
            range: self.id.range,
            refs: Rc::new(RefCell::new(refs)),
            doc: self.doc.clone(),
            llvmname: if self.declare {
                self.id.name.clone()
            } else {
                child.plmod.get_full_name(&self.id.name)
            },
            method,
            generic_map,
            generic: self.generics.is_some(),
            node: Box::new(self.clone()),
        };
        if method {
            let a = self.paralist.first().unwrap().tp.get_type(&child).unwrap();
            let mut b = a.borrow_mut();
            if let PLType::POINTER(s) = &mut *b {
                if let PLType::STRUCT(s) = &mut *s.borrow_mut() {
                    let mut ftp = ftp.clone();
                    ftp.param_pltypes = ftp.param_pltypes[1..].to_vec();
                    s.methods
                        .insert(self.id.name.split("::").last().unwrap().to_string(), ftp);
                }
            }
        }
        ftp.get_or_insert_fn(&child);
        let pltype = Rc::new(RefCell::new(PLType::FN(ftp)));
        child.set_if_refs_tp(pltype.clone(), self.id.range);
        child.add_doc_symbols(pltype.clone());
        ctx.add_type(self.id.name.clone(), pltype, self.id.range)?;
        Ok(())
    }
}
impl Node for FuncDefNode {
    fn format(&self, tabs: usize, prefix: &str) -> String {
        let paralist = &self.paralist;
        let params_print = print_params(&paralist);
        let mut doc_str = String::new();
        for c in self.doc.iter() {
            doc_str.push_str(&prefix.repeat(tabs));
            doc_str.push_str(&c.format(tabs, prefix));
        }
        let mut format_res = String::new();
        format_res.push_str(enter());
        let mut ret_type = String::new();
        ret_type.push_str(&self.ret.format(tabs, prefix));
        format_res.push_str(&doc_str);
        format_res.push_str(&prefix.repeat(tabs));
        format_res.push_str("fn ");
        format_res.push_str(&self.id.name.split("::").last().unwrap());
        format_res.push_str("(");
        format_res.push_str(&params_print);
        format_res.push_str(") ");
        format_res.push_str(&ret_type);
        match &self.body {
            Some(body) => {
                format_res.push_str(" {");
                format_res.push_str(&body.format(tabs + 1, prefix));
                format_res.push_str(&prefix.repeat(tabs));
                format_res.push_str("}");
            }
            None => {
                format_res.push_str(";");
            }
        }
        format_res.push_str(enter());
        format_res
    }

    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("FuncDefNode");
        tab(tabs + 1, line.clone(), false);
        println!("id: {}", self.id.name);
        for c in self.doc.iter() {
            c.print(tabs + 1, false, line.clone());
        }
        for p in self.paralist.iter() {
            p.print(tabs + 1, false, line.clone());
        }
        // tab(tabs + 1, line.clone(), false);
        self.ret.print(tabs + 1, false, line.clone());
        if let Some(body) = &self.body {
            body.print(tabs + 1, true, line.clone());
        }
    }
    fn emit<'a, 'ctx>(&mut self, ctx: &mut Ctx<'a, 'ctx>) -> NodeResult<'ctx> {
        ctx.save_if_comment_doc_hover(self.id.range, Some(self.doc.clone()));
        for c in self.doc.iter_mut() {
            c.emit(ctx)?;
        }
        ctx.push_semantic_token(self.id.range, SemanticTokenType::FUNCTION, 0);
        for para in self.paralist.iter() {
            ctx.push_semantic_token(para.id.range, SemanticTokenType::PARAMETER, 0);
            ctx.push_semantic_token(para.tp.range(), SemanticTokenType::TYPE, 0);
        }
        ctx.push_semantic_token(self.ret.range(), SemanticTokenType::TYPE, 0);
        let pltype = ctx.get_type(&self.id.name, self.range)?;
        if let Some(body) = self.body.as_mut() {
            // add function
            let mut child = ctx.new_child(self.range.start);
            let fntype = match &*pltype.borrow() {
                PLType::FN(fntype) => fntype.clone(),
                _ => return Ok((None, None, TerminatorEnum::NONE)),
            };
            let funcvalue = {
                fntype.add_generic_type(&mut child)?;
                fntype.get_or_insert_fn(&child)
            };
            let mut param_ditypes = vec![];
            for para in self.paralist.iter() {
                let pltype = para.tp.get_type(&child)?;
                match &*pltype.borrow() {
                    PLType::VOID => {
                        return Err(child.add_err(
                            para.range,
                            crate::ast::diag::ErrorCode::VOID_TYPE_CANNOT_BE_PARAMETER,
                        ))
                    }
                    pltype => {
                        param_ditypes.push(pltype.get_ditype(&child).unwrap());
                    }
                };
            }
            // debug info
            let subroutine_type = child.dibuilder.create_subroutine_type(
                child.diunit.get_file(),
                self.ret.get_type(&child)?.borrow().get_ditype(&child),
                &param_ditypes,
                DIFlags::PUBLIC,
            );
            let subprogram = child.dibuilder.create_function(
                child.diunit.get_file().as_debug_info_scope(),
                &fntype.append_name_with_generic(fntype.name.clone()),
                None,
                child.diunit.get_file(),
                self.range.start.line as u32,
                subroutine_type,
                true,
                true,
                self.range.start.line as u32,
                DIFlags::PUBLIC,
                false,
            );
            funcvalue.set_subprogram(subprogram);
            child.function = Some(funcvalue);
            let discope = child.discope;
            child.discope = subprogram.as_debug_info_scope().clone();
            // add block
            let allocab = child.context.append_basic_block(funcvalue, "alloc");
            let entry = child.context.append_basic_block(funcvalue, "entry");
            let return_block = child.context.append_basic_block(funcvalue, "return");
            child.position_at_end(return_block);
            let ret_value_ptr = if funcvalue.get_type().get_return_type().is_some() {
                let pltype = self.ret.get_type(&mut child)?;
                let ret_type = {
                    let op = pltype.borrow().get_basic_type_op(&child);
                    if op.is_none() {
                        return Ok((None, None, TerminatorEnum::NONE));
                    }
                    op.unwrap()
                };
                let retv = alloc(&mut child, ret_type, "retvalue");
                // 返回值不能在函数结束时从root表移除
                child.roots.borrow_mut().pop();
                Some(retv)
            } else {
                None
            };

            child.return_block = Some((return_block, ret_value_ptr));
            if let Some(ptr) = ret_value_ptr {
                let value = child.nodebug_builder.build_load(ptr, "load_ret_tmp");
                child.builder.position_at_end(return_block);
                child.gc_collect();
                child.gc_rm_root_current(ptr.as_basic_value_enum());
                child.nodebug_builder.build_return(Some(&value));
            } else {
                child.gc_collect();
                child.nodebug_builder.build_return(None);
            };
            child.position_at_end(entry);
            // alloc para
            for (i, para) in fntype.param_pltypes.iter().enumerate() {
                let alloca = alloc(
                    &mut child,
                    para.clone().borrow().get_basic_type(ctx),
                    &fntype.param_names[i],
                );
                // add alloc var debug info
                let divar = child.dibuilder.create_parameter_variable(
                    child.discope,
                    &fntype.param_names[i],
                    i as u32,
                    child.diunit.get_file(),
                    self.range.start.line as u32,
                    param_ditypes[i],
                    false,
                    DIFlags::PUBLIC,
                );
                child.build_dbg_location(self.paralist[i].range.start);
                child.dibuilder.insert_declare_at_end(
                    alloca,
                    Some(divar),
                    None,
                    child.builder.get_current_debug_location().unwrap(),
                    allocab,
                );
                child
                    .builder
                    .build_store(alloca, funcvalue.get_nth_param(i as u32).unwrap());
                child
                    .add_symbol(
                        fntype.param_names[i].clone(),
                        alloca,
                        para.clone(),
                        self.paralist[i].id.range,
                        false,
                    )
                    .unwrap();
            }
            // emit body
            child.builder.unset_current_debug_location();
            if self.id.name == "main" {
                if let Some(inst) = allocab.get_first_instruction() {
                    child.builder.position_at(allocab, &inst);
                    child.nodebug_builder.position_at(allocab, &inst);
                } else {
                    child.position_at_end(allocab);
                }
                child.init_global();
                child.builder.position_at_end(entry);
                child.nodebug_builder.position_at_end(entry);
            }
            let (_, _, terminator) = body.emit(&mut child)?;
            if !terminator.is_return() {
                return Err(child.add_err(self.range, ErrorCode::FUNCTION_MUST_HAVE_RETURN));
            }
            child.nodebug_builder.position_at_end(allocab);
            child.nodebug_builder.build_unconditional_branch(entry);
            child.discope = discope;
            return Ok((None, Some(pltype.clone()), TerminatorEnum::NONE));
        }
        Ok((None, Some(pltype.clone()), TerminatorEnum::NONE))
    }
}
