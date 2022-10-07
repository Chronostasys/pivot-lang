use crate::ast::node::Value;
use colored::Colorize;
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::debug_info::*;
use inkwell::module::Module;
use inkwell::targets::TargetMachine;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::FunctionType;
use inkwell::types::StructType;
use inkwell::types::VoidType;
use inkwell::values::BasicValueEnum;
use inkwell::values::FunctionValue;
use inkwell::values::PointerValue;
use lsp_types::Diagnostic;
use lsp_types::DiagnosticSeverity;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;

use super::compiler::get_target_machine;
use super::error::ErrorCode;
use super::error::ERR_MSG;
use super::node::types::TypeNameNode;
use super::range::Pos;
use super::range::Range;
// TODO: match all case
// const DW_ATE_UTF: u32 = 0x10;
const DW_ATE_BOOLEAN: u32 = 0x02;
const DW_ATE_FLOAT: u32 = 0x04;
const DW_ATE_SIGNED: u32 = 0x05;
// const DW_ATE_UNSIGNED: u32 = 0x07;
fn get_dw_ate_encoding(basetype: &BasicTypeEnum) -> u32 {
    match basetype {
        BasicTypeEnum::FloatType(_) => DW_ATE_FLOAT,
        BasicTypeEnum::IntType(i) => match i.get_bit_width() {
            1 => DW_ATE_BOOLEAN,
            64 => DW_ATE_SIGNED,
            _ => todo!(),
        },
        _ => todo!(),
    }
}
#[derive(Debug, Clone)]
pub struct Ctx<'a, 'ctx> {
    pub table: HashMap<String, (PointerValue<'ctx>, String)>,
    pub types: HashMap<String, (PLType<'a, 'ctx>, Option<DIType<'ctx>>)>,
    pub father: Option<&'a Ctx<'a, 'ctx>>,
    pub context: &'ctx Context,
    pub builder: &'a Builder<'ctx>,
    pub module: &'a Module<'ctx>,
    pub dibuilder: &'a DebugInfoBuilder<'ctx>,
    pub diunit: &'a DICompileUnit<'ctx>,
    pub function: Option<FunctionValue<'ctx>>,
    pub block: Option<BasicBlock<'ctx>>,
    pub continue_block: Option<BasicBlock<'ctx>>,
    pub break_block: Option<BasicBlock<'ctx>>,
    pub targetmachine: &'a TargetMachine,
    pub discope: DIScope<'ctx>,
    pub nodebug_builder: &'a Builder<'ctx>,
    pub src_file_path: &'a str,
    pub errs: &'a RefCell<Vec<PLDiag>>,
}

#[derive(Debug, Clone)]
pub enum PLDiag {
    Error(Err),
}

#[derive(Debug, Clone)]
pub struct Err {
    pub msg: String,
    pub diag: Diagnostic,
}

const PL_DIAG_SOURCE: &str = "plsp";

impl PLDiag {
    pub fn print(&self) {
        match self {
            PLDiag::Error(s) => {
                let err = format!(
                    "error at {}\n\t{}",
                    format!(
                        "{}:{}:{}",
                        s.msg,
                        s.diag.range.start.line + 1,
                        s.diag.range.start.character + 1
                    )
                    .yellow(),
                    format!("{}", s.diag.message.blue().bold()),
                );
                println!("{}", err);
            }
        }
    }
    pub fn get_diagnostic(&self) -> Diagnostic {
        match self {
            PLDiag::Error(s) => s.diag.clone(),
        }
    }
    pub fn new_error(file: String, range: Range, code: ErrorCode) -> Self {
        let diag = Diagnostic::new_with_code_number(
            range.to_diag_range(),
            DiagnosticSeverity::ERROR,
            code as i32,
            Some(PL_DIAG_SOURCE.to_string()),
            ERR_MSG[&code].to_string(),
        );
        PLDiag::Error(Err { msg: file, diag })
    }
}

#[derive(Debug, Clone)]
pub enum PLType<'a, 'ctx> {
    FN(FNType<'ctx>),
    STRUCT(STType<'a, 'ctx>),
    PRIMITIVE(PriType<'ctx>),
    VOID(VoidType<'ctx>),
}
#[derive(Debug, Clone)]
pub struct PriType<'ctx> {
    basetype: BasicTypeEnum<'ctx>,
    id: String,
}
impl<'a, 'ctx> PLType<'a, 'ctx> {
    pub fn get_basic_type(&self) -> BasicTypeEnum<'ctx> {
        self.get_basic_type_op().unwrap()
    }
    pub fn get_basic_type_op(&self) -> Option<BasicTypeEnum<'ctx>> {
        match self {
            PLType::FN(f) => Some(
                f.fntype
                    .get_type()
                    .ptr_type(inkwell::AddressSpace::Global)
                    .as_basic_type_enum(),
            ),
            PLType::STRUCT(s) => Some(s.struct_type.as_basic_type_enum()),
            PLType::PRIMITIVE(t) => Some(t.basetype),
            PLType::VOID(_) => None,
        }
    }

    pub fn get_ret_type(&self) -> RetTypeEnum<'ctx> {
        match self {
            PLType::VOID(x) => RetTypeEnum::VOID(*x),
            _ => RetTypeEnum::BASIC(self.get_basic_type()),
        }
    }

    pub fn get_ditype(&self, ctx: &mut Ctx<'a, 'ctx>) -> Option<DIType<'ctx>> {
        let td = ctx.targetmachine.get_target_data();
        match self {
            PLType::FN(_) => None,
            PLType::STRUCT(x) => {
                let mut offset = 0;
                let m = x
                    .ordered_fields
                    .iter()
                    .map(|v| {
                        let (tp, off) = v.get_di_type(ctx, offset);
                        offset = off;
                        tp
                    })
                    .collect::<Vec<_>>();
                return Some(
                    ctx.dibuilder
                        .create_struct_type(
                            ctx.discope,
                            &x.name,
                            ctx.diunit.get_file(),
                            x.line_no,
                            td.get_bit_size(&x.struct_type),
                            td.get_abi_alignment(&x.struct_type),
                            DIFlags::PUBLIC,
                            None,
                            &m,
                            0,
                            None,
                            &x.name,
                        )
                        .as_type(),
                );
            }
            PLType::PRIMITIVE(pt) => {
                return Some(
                    ctx.dibuilder
                        .create_basic_type(
                            &pt.id,
                            td.get_bit_size(&self.get_basic_type()),
                            get_dw_ate_encoding(&self.get_basic_type()),
                            DIFlags::PUBLIC,
                        )
                        .unwrap()
                        .as_type(),
                );
            }
            PLType::VOID(_) => None,
        }
    }
}

pub enum RetTypeEnum<'ctx> {
    VOID(VoidType<'ctx>),
    BASIC(BasicTypeEnum<'ctx>),
}

impl<'ctx> RetTypeEnum<'ctx> {
    pub fn fn_type(
        &self,
        param_types: &[BasicMetadataTypeEnum<'ctx>],
        is_var_args: bool,
    ) -> FunctionType<'ctx> {
        match self {
            RetTypeEnum::VOID(t) => t.fn_type(param_types, is_var_args),
            RetTypeEnum::BASIC(t) => t.fn_type(param_types, is_var_args),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field<'a, 'ctx> {
    pub index: u32,
    pub tp: PLType<'a, 'ctx>,
    pub typename: &'a TypeNameNode,
    pub name: String,
}

impl<'a, 'ctx> Field<'a, 'ctx> {
    pub fn get_di_type(&self, ctx: &mut Ctx<'a, 'ctx>, offset: u64) -> (DIType<'ctx>, u64) {
        let tp = self.typename.get_debug_type(ctx).unwrap();
        (
            ctx.dibuilder
                .create_member_type(
                    ctx.discope,
                    &self.name,
                    ctx.diunit.get_file(),
                    self.typename.range.start.line as u32,
                    tp.get_size_in_bits(),
                    tp.get_align_in_bits(),
                    offset + tp.get_offset_in_bits(),
                    DIFlags::PUBLIC,
                    tp,
                )
                .as_type(),
            offset + tp.get_size_in_bits(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct FNType<'ctx> {
    pub name: String,
    pub fntype: FunctionValue<'ctx>,
    pub ret_pltype: Option<String>,
}
#[derive(Debug, Clone)]
pub struct STType<'a, 'ctx> {
    pub name: String,
    pub fields: BTreeMap<String, Field<'a, 'ctx>>,
    pub struct_type: StructType<'ctx>,
    pub ordered_fields: Vec<Field<'a, 'ctx>>,
    pub line_no: u32,
}

fn add_primitive_types<'a, 'ctx>(ctx: &mut Ctx<'a, 'ctx>) {
    let pltype_i64 = PLType::PRIMITIVE(PriType {
        basetype: ctx.context.i64_type().as_basic_type_enum(),
        id: "i64".to_string(),
    });
    let ditype_i64 = pltype_i64.get_ditype(ctx);
    ctx.types
        .insert("i64".to_string(), (pltype_i64.clone(), ditype_i64));

    let pltype_f64 = PLType::PRIMITIVE(PriType {
        basetype: ctx.context.f64_type().as_basic_type_enum(),
        id: "f64".to_string(),
    });
    let ditype_f64 = pltype_f64.get_ditype(ctx);
    ctx.types
        .insert("f64".to_string(), (pltype_f64.clone(), ditype_f64));

    let pltype_bool = PLType::PRIMITIVE(PriType {
        basetype: ctx.context.bool_type().as_basic_type_enum(),
        id: "bool".to_string(),
    });
    let ditype_bool = pltype_bool.get_ditype(ctx);
    ctx.types
        .insert("bool".to_string(), (pltype_bool.clone(), ditype_bool));

    let pltype_void = PLType::VOID(ctx.context.void_type());
    let ditype_void = pltype_void.get_ditype(ctx);
    ctx.types
        .insert("void".to_string(), (pltype_void.clone(), ditype_void));
}

pub fn create_ctx_info<'ctx>(
    context: &'ctx Context,
    dir: &str,
    file: &str,
) -> (
    Module<'ctx>,
    Builder<'ctx>,
    DebugInfoBuilder<'ctx>,
    DICompileUnit<'ctx>,
    TargetMachine,
    Builder<'ctx>,
) {
    let builder = context.create_builder();
    let module = context.create_module("main");
    let (dibuilder, compile_unit) = module.create_debug_info_builder(
        true,
        DWARFSourceLanguage::C,
        file,
        dir,
        "plc frontend",
        false,
        "",
        0,
        "",
        DWARFEmissionKind::Full,
        0,
        false,
        false,
        "",
        "",
    );
    let tm = get_target_machine(inkwell::OptimizationLevel::None);
    (
        module,
        builder,
        dibuilder,
        compile_unit,
        tm,
        context.create_builder(),
    )
}

impl<'a, 'ctx> Ctx<'a, 'ctx> {
    pub fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        builder: &'a Builder<'ctx>,
        dibuilder: &'a DebugInfoBuilder<'ctx>,
        diunit: &'a DICompileUnit<'ctx>,
        tm: &'a TargetMachine,
        nodbg_builder: &'a Builder<'ctx>,
        src_file_path: &'a str,
        errs: &'a RefCell<Vec<PLDiag>>,
    ) -> Ctx<'a, 'ctx> {
        let mut ctx = Ctx {
            table: HashMap::new(),
            types: HashMap::new(),
            father: None,
            context,
            module,
            builder,
            function: None,
            block: None,
            continue_block: None,
            break_block: None,
            dibuilder,
            diunit,
            targetmachine: tm,
            discope: diunit.as_debug_info_scope(),
            nodebug_builder: nodbg_builder,
            src_file_path,
            errs,
        };
        add_primitive_types(&mut ctx);
        ctx
    }
    pub fn new_child(&'a self, start: Pos) -> Ctx<'a, 'ctx> {
        let mut ctx = Ctx {
            table: HashMap::new(),
            types: HashMap::new(),
            father: Some(self),
            context: self.context,
            builder: self.builder,
            module: self.module,
            function: self.function,
            block: self.block,
            continue_block: self.continue_block,
            break_block: self.break_block,
            dibuilder: self.dibuilder,
            diunit: self.diunit,
            targetmachine: self.targetmachine,
            discope: self
                .dibuilder
                .create_lexical_block(
                    self.discope,
                    self.diunit.get_file(),
                    start.line as u32,
                    start.column as u32,
                )
                .as_debug_info_scope(),
            nodebug_builder: self.nodebug_builder,
            src_file_path: self.src_file_path,
            errs: self.errs,
        };
        add_primitive_types(&mut ctx);
        ctx
    }

    /// # get_symbol
    /// search in current and all father symbol tables
    pub fn get_symbol(&self, name: &str) -> Option<(&PointerValue<'ctx>, String)> {
        let v = self.table.get(name);
        if let Some((pv, pltype)) = v {
            return Some((pv, pltype.to_string()));
        }
        if let Some(father) = self.father {
            return father.get_symbol(name);
        }
        None
    }

    pub fn add_symbol(
        &mut self,
        name: String,
        pv: PointerValue<'ctx>,
        tp: String,
        range: Range,
    ) -> Result<(), PLDiag> {
        if self.table.contains_key(&name) {
            return Err(self.add_err(range, ErrorCode::REDECLARATION));
        }
        self.table.insert(name, (pv, tp));
        Ok(())
    }

    pub fn get_type(&self, name: &str) -> Option<&(PLType<'a, 'ctx>, Option<DIType<'ctx>>)> {
        let v = self.types.get(name);
        if let Some(pv) = v {
            return Some(pv);
        }
        if let Some(father) = self.father {
            return father.get_type(name);
        }
        None
    }

    pub fn add_type(&mut self, name: String, tp: PLType<'a, 'ctx>) {
        if self.types.contains_key(&name) {
            todo!() // TODO 报错
        }
        let ditype = tp.get_ditype(self);
        self.types.insert(name, (tp.clone(), ditype));
    }

    pub fn add_err(&mut self, range: Range, code: ErrorCode) -> PLDiag {
        let dia = PLDiag::new_error(self.src_file_path.to_string(), range, code);
        self.errs.borrow_mut().push(dia.clone());
        dia
    }

    pub fn try_load(&mut self, v: Value<'ctx>) -> Value<'ctx> {
        match v.as_basic_value_enum() {
            BasicValueEnum::PointerValue(v) => {
                let v = self.builder.build_load(v, "loadtmp");
                match v {
                    BasicValueEnum::IntValue(v) => match v.get_type().get_bit_width() {
                        1 => Value::BoolValue(v),
                        64 => Value::IntValue(v),
                        _ => todo!(),
                    },
                    BasicValueEnum::FloatValue(v) => Value::FloatValue(v),
                    _ => Value::LoadValue(v),
                }
            }
            _ => v,
        }
    }
}
