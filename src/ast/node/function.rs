use super::statement::StatementsNode;
use super::*;
use super::{alloc, types::TypedIdentifierNode, Node, TypeNode};
use crate::ast::diag::ErrorCode;
use crate::ast::node::{deal_line, tab};
use crate::ast::pltype::{eq, FNType, PLType};
use crate::utils::read_config::enter;
use indexmap::IndexMap;
use inkwell::debug_info::*;
use internal_macro::{comments, format, range};
use lsp_types::SemanticTokenType;
use std::cell::RefCell;
use std::fmt::format;
use std::rc::Rc;
use std::vec;
#[range]
#[format]
#[comments]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FuncCallNode {
    pub generic_params: Option<Box<GenericParamNode>>,
    pub id: Box<NodeEnum>,
    pub paralist: Vec<Box<NodeEnum>>,
}

impl Node for FuncCallNode {
    fn format(&self, builder: &mut FmtBuilder) {
        self.formatBuild(builder);
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
        // let currscope = ctx.discope;
        let mp = ctx.move_generic_types();
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
        fntype.add_generic_type(ctx)?;
        fntype.clear_generic();
        if let Some(generic_params) = &self.generic_params {
            let generic_params_range = generic_params.range.clone();
            generic_params.emit_highlight(ctx);
            if generic_params.generics.len() != fntype.generic_map.len() {
                return Err(
                    ctx.add_err(generic_params_range, ErrorCode::GENERIC_PARAM_LEN_MISMATCH)
                );
            }
            let generic_types = generic_params.get_generic_types(ctx)?;
            let mut i = 0;
            for (_, pltype) in fntype.generic_map.iter() {
                if generic_types[i].is_some() {
                    eq(pltype.clone(), generic_types[i].as_ref().unwrap().clone());
                }
                i = i + 1;
            }
        }
        let mut skip = 0;
        if plvalue.is_some() {
            if let Some(receiver) = plvalue.unwrap().receiver {
                para_values.push(receiver.into());
                skip = 1;
            }
        }
        // funcvalue must use fntype to get a new one,can not use the return  plvalue of id node emit
        if fntype.param_pltypes.len() - skip as usize != self.paralist.len() {
            return Err(ctx.add_err(self.range, ErrorCode::PARAMETER_LENGTH_NOT_MATCH));
        }
        // set sig and param hint
        let mut prevpos = id_range.end;
        for (i, para) in self.paralist.iter_mut().enumerate() {
            let sigrange = prevpos.to(para.range().end);
            prevpos = para.range().end;
            let pararange = para.range();
            ctx.push_param_hint(
                pararange.clone(),
                fntype.param_names[i + skip as usize].clone(),
            );
            ctx.set_if_sig(
                sigrange,
                fntype.name.clone().split("::").last().unwrap().to_string()
                    + "("
                    + fntype
                        .param_names
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            s.clone()
                                + ": "
                                + FmtBuilder::generate_node(&fntype.param_pltypes[i]).as_str()
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str()
                    + ")",
                &fntype.param_names,
                i as u32 + skip,
            );
        }
        // value check and generic infer
        for (i, para) in self.paralist.iter_mut().enumerate() {
            let pararange = para.range();
            let (value, value_pltype, _) = para.emit(ctx)?;
            if value.is_none() || value_pltype.is_none() {
                return Ok((None, None, TerminatorEnum::NONE));
            }
            let load = ctx.try_load2var(pararange, value.unwrap())?;
            let value_pltype = value_pltype.unwrap();
            if !fntype.param_pltypes[i + skip as usize]
                .clone()
                .eq_or_infer(ctx, value_pltype.clone())?
            {
                return Err(ctx.add_err(pararange, ErrorCode::PARAMETER_TYPE_NOT_MATCH));
            }
            para_values.push(load.as_basic_value_enum().into());
        }
        if fntype.need_gen_code() {
            let block = ctx.block;
            let f = ctx.function;
            ctx.need_highlight = false;
            let (_, pltype, _) = fntype.node.gen_fntype(ctx, false)?;
            ctx.need_highlight = true;
            ctx.function = f;
            ctx.position_at_end(block.unwrap());
            let pltype = pltype.unwrap();
            match &*pltype.borrow() {
                PLType::FN(f) => {
                    fntype = f.clone();
                }
                _ => unreachable!(),
            };
        }
        let function = fntype.get_or_insert_fn(ctx);
        if let Some(f) = ctx.function {
            if f.get_subprogram().is_some() {
                ctx.discope = f.get_subprogram().unwrap().as_debug_info_scope();
                let pos = self.range().start;
                // ctx.discope = currscope;
                ctx.build_dbg_location(pos)
            }
        };
        let ret = ctx.builder.build_call(
            function,
            &para_values,
            format(format_args!("call_{}", RefCell::borrow(&pltype).get_name())).as_str(),
        );
        ctx.save_if_comment_doc_hover(id_range, Some(fntype.doc.clone()));
        let res = match ret.try_as_basic_value().left() {
            Some(v) => Ok((
                {
                    ctx.nodebug_builder.unset_current_debug_location();
                    let ptr = alloc(ctx, v.get_type(), "ret_alloc_tmp");
                    ctx.nodebug_builder.build_store(ptr, v);
                    Some(ptr.into())
                },
                Some({
                    match &*fntype.ret_pltype.get_type(ctx)?.borrow() {
                        PLType::GENERIC(g) => g.curpltype.as_ref().unwrap().clone(),
                        _ => fntype.ret_pltype.get_type(ctx)?,
                    }
                }),
                TerminatorEnum::NONE,
            )),
            None => Ok((
                None,
                Some(fntype.ret_pltype.get_type(ctx)?),
                TerminatorEnum::NONE,
            )),
        };
        fntype.clear_generic();
        ctx.send_if_go_to_def(id_range, fntype.range, ctx.plmod.path.clone());
        ctx.set_if_refs_tp(pltype.clone(), id_range);
        ctx.reset_generic_types(mp);
        ctx.emit_comment_highlight(&self.comments[0]);
        return res;
    }
}
#[range]
#[format]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FuncDefNode {
    pub id: Box<VarNode>,
    pub paralist: Vec<Box<TypedIdentifierNode>>,
    pub ret: Box<TypeNodeEnum>,
    pub doc: Vec<Box<NodeEnum>>,
    pub precom: Vec<Box<NodeEnum>>,
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
        let mut generic_map = IndexMap::default();
        if let Some(generics) = &mut self.generics {
            generic_map = generics.gen_generic_type(ctx);
        }
        let mp = ctx.move_generic_types();
        for (name, pltype) in generic_map.iter() {
            ctx.add_generic_type(
                name.clone(),
                pltype.clone(),
                pltype.clone().borrow().get_range().unwrap(),
            );
        }
        for para in self.paralist.iter() {
            let paramtype = para.typenode.get_type(ctx)?;
            ctx.set_if_refs_tp(paramtype.clone(), para.typenode.range());
            if first && para.id.name == "self" {
                method = true;
            }
            first = false;
            param_pltypes.push(para.typenode.clone());
            param_name.push(para.id.name.clone());
        }
        let refs = vec![];
        let mut ftp = FNType {
            name: self.id.name.clone(),
            ret_pltype: self.ret.clone(),
            param_pltypes,
            param_names: param_name,
            range: self.id.range,
            refs: Rc::new(RefCell::new(refs)),
            doc: self.doc.clone(),
            llvmname: if self.declare {
                self.id.name.clone()
            } else {
                ctx.plmod.get_full_name(&self.id.name)
            },
            method,
            generic_map,
            generic: self.generics.is_some(),
            node: Box::new(self.clone()),
        };
        if self.generics.is_none() {
            ftp.get_or_insert_fn(ctx);
        }
        let pltype = Rc::new(RefCell::new(PLType::FN(ftp.clone())));
        ctx.set_if_refs_tp(pltype.clone(), self.id.range);
        ctx.add_doc_symbols(pltype.clone());
        if method {
            let a = self
                .paralist
                .first()
                .unwrap()
                .typenode
                .get_type(ctx)
                .unwrap();
            let mut b = a.borrow_mut();
            if let PLType::POINTER(s) = &mut *b {
                if let PLType::STRUCT(s) = &mut *s.borrow_mut() {
                    ftp.param_pltypes = ftp.param_pltypes[1..].to_vec();
                    ctx.add_method(
                        s,
                        self.id.name.split("::").last().unwrap(),
                        ftp.clone(),
                        self.id.range,
                    );
                }
            }
        }
        ctx.add_type(self.id.name.clone(), pltype, self.id.range)?;
        ctx.reset_generic_types(mp);
        Ok(())
    }
}
impl Node for FuncDefNode {
    fn format(&self, builder: &mut FmtBuilder) {
        self.formatBuild(builder);
    }
    fn print(&self, tabs: usize, end: bool, mut line: Vec<bool>) {
        deal_line(tabs, &mut line, end);
        tab(tabs, line.clone(), end);
        println!("FuncDefNode");
        tab(tabs + 1, line.clone(), false);
        println!("id: {}", self.id.name);
        for c in self.precom.iter() {
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
        self.gen_fntype(ctx, true)
    }
}
impl FuncDefNode {
    fn gen_fntype<'a, 'ctx>(&mut self, ctx: &mut Ctx<'a, 'ctx>, first: bool) -> NodeResult<'ctx> {
        ctx.save_if_comment_doc_hover(self.id.range, Some(self.doc.clone()));
        ctx.emit_comment_highlight(&self.precom);
        ctx.push_semantic_token(self.id.range, SemanticTokenType::FUNCTION, 0);
        if let Some(generics) = &mut self.generics {
            generics.emit(ctx)?;
        }
        for para in self.paralist.iter() {
            ctx.push_semantic_token(para.id.range, SemanticTokenType::PARAMETER, 0);
            ctx.push_semantic_token(para.typenode.range(), SemanticTokenType::TYPE, 0);
        }
        ctx.push_semantic_token(self.ret.range(), SemanticTokenType::TYPE, 0);
        let pltype = ctx.get_type(&self.id.name, self.range)?;
        if let Some(body) = self.body.as_mut() {
            // add function
            let child = &mut ctx.new_child(self.range.start);
            let mp = child.move_generic_types();
            let mut fntype = match &*pltype.borrow() {
                PLType::FN(fntype) => fntype.clone(),
                _ => return Ok((None, None, TerminatorEnum::NONE)),
            };
            let funcvalue = {
                fntype.add_generic_type(child)?;
                if first {
                    fntype.generic_map.iter_mut().for_each(|(_, pltype)| {
                        match &mut *pltype.borrow_mut() {
                            PLType::GENERIC(g) => {
                                g.set_place_holder(child);
                            }
                            _ => unreachable!(),
                        }
                    })
                }
                fntype.get_or_insert_fn(child)
            };
            let mut param_ditypes = vec![];
            for para in self.paralist.iter() {
                let pltype = para.typenode.get_type(child)?;
                match &*pltype.borrow() {
                    PLType::VOID => {
                        return Err(
                            child.add_err(para.range, ErrorCode::VOID_TYPE_CANNOT_BE_PARAMETER)
                        )
                    }
                    pltype => {
                        param_ditypes.push(pltype.get_ditype(child).unwrap());
                    }
                };
            }
            // debug info
            let subroutine_type = child.dibuilder.create_subroutine_type(
                child.diunit.get_file(),
                self.ret.get_type(child)?.borrow().get_ditype(child),
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
                let pltype = self.ret.get_type(child)?;
                let ret_type = {
                    let op = pltype.borrow().get_basic_type_op(child);
                    if op.is_none() {
                        return Ok((None, None, TerminatorEnum::NONE));
                    }
                    op.unwrap()
                };
                ctx.nodebug_builder.unset_current_debug_location();
                let retv = alloc(child, ret_type, "retvalue");
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
                let basetype = para.get_type(child)?.borrow().get_basic_type(child);
                let alloca = alloc(child, basetype, &fntype.param_names[i]);
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
                let parapltype = para.get_type(child)?.clone();
                child
                    .add_symbol(
                        fntype.param_names[i].clone(),
                        alloca,
                        parapltype,
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
            let (_, _, terminator) = body.emit(child)?;
            if !terminator.is_return() {
                return Err(child.add_err(self.range, ErrorCode::FUNCTION_MUST_HAVE_RETURN));
            }
            child.nodebug_builder.position_at_end(allocab);
            child.nodebug_builder.build_unconditional_branch(entry);
            child.discope = discope;
            child.reset_generic_types(mp);
            return Ok((None, Some(pltype.clone()), TerminatorEnum::NONE));
        }
        Ok((None, Some(pltype.clone()), TerminatorEnum::NONE))
    }
}
