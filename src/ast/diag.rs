use ariadne::{Cache, ColorGenerator, Fmt, Label, Report, ReportKind, Source};
use dyn_fmt::AsStrFormatExt;
use internal_macro::range;
use lazy_static::lazy_static;
use rustc_hash::FxHashMap;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};
macro_rules! define_error {
    ($(
        $ident:ident = $string_keyword:expr
    ),*) => {
        #[derive(Debug, PartialEq, Clone, Copy, Eq, Hash, Default)]
        #[allow(non_camel_case_types, dead_code)]
        pub enum ErrorCode {
            #[default] UNKNOWN = 114513,
            $($ident),*
        }
        $(pub const $ident: &'static str = $string_keyword;)*
        lazy_static! {
            pub static ref ERR_MSG: HashMap<ErrorCode, &'static str> = {
                let mut mp = HashMap::new();
                $(mp.insert(ErrorCode::$ident, $ident);)*
                mp
            };
        }
    };
    ($(
        $ident:ident = $string_keyword:expr
    ),*,) => {
        define_error!($($ident = $string_keyword),*);
    };
}
define_error!(
    SYNTAX_ERROR_STATEMENT = "failed to parse statement",
    SYNTAX_ERROR_TOP_STATEMENT = "failed to parse top level statement",
    WHILE_CONDITION_MUST_BE_BOOL = "while condition must be bool",
    IF_CONDITION_MUST_BE_BOOL = "if condition must be bool",
    BREAK_MUST_BE_IN_LOOP = "break statement must be in loop",
    CONTINUE_MUST_BE_IN_LOOP = "continue statement must be in loop",
    VOID_TYPE_CANNOT_BE_PARAMETER = "void type cannot be function parameter",
    PARAMETER_TYPE_NOT_MATCH = "parameter type not match",
    PARAMETER_LENGTH_NOT_MATCH = "parameter length not match",
    FUNCTION_NOT_FOUND = "function not found",
    NOT_A_FUNCTION = "not a function",
    VALUE_NOT_COMPARABLE = "value not comparable",
    LOGIC_OP_NOT_BOOL = "logic operation parameters must be bool",
    UNRECOGNIZED_BIN_OPERATOR = "unrecognized binary operator",
    UNRECOGNIZED_UNARY_OPERATOR = "unrecognized unary operator",
    INVALID_UNARY_EXPRESSION = "invalid unary expression",
    STRUCT_FIELD_NOT_FOUND = "struct field not found",
    INVALID_GET_FIELD = "cannot get field from non struct type",
    VAR_NOT_FOUND = "variable not found",
    REDECLARATION = "redeclaration of variable",
    FOR_CONDITION_MUST_BE_BOOL = "for condition must be bool",
    COMPLETION = "exp not complete",
    NOT_ASSIGNABLE = "left value is not assignable",
    ASSIGN_CONST = "try assigning to a const value",
    REF_CONST = "try referencing to a const value",
    INVALID_STRUCT_DEF = "invalid struct definition",
    UNDEFINED_TYPE = "undefined type",
    RETURN_VALUE_IN_VOID_FUNCTION = "return value in void function",
    RETURN_TYPE_MISMATCH = "return type mismatch",
    NO_RETURN_VALUE_IN_NON_VOID_FUNCTION = "non void function must have a return value",
    FUNCTION_MUST_HAVE_RETURN = "function must have a return value",
    REDEFINE_TYPE = "redefine type",
    STRUCT_FIELD_TYPE_NOT_MATCH = "struct field type not match",
    MISSING_SEMI = "missing semicolon",
    EXPECT_TYPE = "expect type",
    EXPECT_VALUE = "expect value",
    REDEFINE_SYMBOL = "redefine symbol",
    SYMBOL_NOT_FOUND = "symbol not found",
    UNRESOLVED_MODULE = "unresolved module",
    ARRAY_TYPE_NOT_MATCH = "array type not match",
    ARRAY_INIT_EMPTY = "array init cannot be empty",
    CANNOT_INDEX_NON_ARRAY = "cannot index non array type",
    ARRAY_INDEX_MUST_BE_INT = "array index must be int",
    ARRAY_INDEX_OUT_OF_BOUNDS = "array index out of bounds",
    NEEDED_INDEX_FOR_ARRAY_ELEMENT_ACCESS = "needed index for array element access",
    SIZE_MUST_BE_INT = "size must be int",
    TYPE_MISMATCH = "type mismatch",
    ILLEGAL_GET_FIELD_OPERATION = "illegal get field operation",
    NOT_A_POINTER = "not a pointer",
    CAN_NOT_REF_CONSTANT = "can not ref constant",
    ILLEGAL_SELF_RECURSION = "illegal self recursion, please use pointer",
    GENERIC_CANNOT_BE_INFER = "generic can not be infer",
    RECEIVER_CANNOT_BE_INFER = "receiver can not be infer",
    DUPLICATE_METHOD = "duplicate method",
    GENERIC_PARAM_LEN_MISMATCH = "generic param len mismatch",
    GENERIC_NOT_FOUND = "generic not found",
    NOT_GENERIC_TYPE = "not generic type",
    EXPECT_TRAIT_TYPE = "expect trait type",
    EXPECT_STRUCT_TYPE = "expect struct type",
    METHOD_NOT_IN_TRAIT = "method not in trait def",
    METHOD_NOT_IN_IMPL = "method required in trait not found in impl block",
    EXPECT_PUBLIC_FUNCTION = "expect public function",
    EXPECT_PUBLIC_STRUCT = "expect public struct",
    EXPECT_PUBLIC_TRAIT = "expect public trait",
    EXPECT_PUBLIC_FIELD = "expect public field",
    TRAIT_METHOD_SHALL_NOT_HAVE_MODIFIER = "trait method shall not have modifier",
    MACRO_NOT_FOUND = "macro not found",
    EXPECT_IDENTIFIER = "expect identifier",
    UNEXPECTED_TOKEN = "unexpected token",
    EXPECT_STRING = "expect string",
    EXPECT_EXPRESSION = "expect expression",
    EXPECT_STATEMENT = "expect statement",
    EXPECT_STATEMENTS = "expect statements",
    NO_MACRO_LOOP_VAR = "no macro loop var used in macro loop block",
    MACRO_LOOP_VAR_USED_OUT_OF_LOOP = "macro loop var used out of loop",
    MACRO_VAR_NOT_FOUND = "macro var not found",
);
macro_rules! define_warn {
    ($(
        $ident:ident = $string_keyword:expr
    ),*) => {
        #[derive(Debug, PartialEq, Clone, Copy, Eq, Hash, Default)]
        #[allow(non_camel_case_types, dead_code)]
        pub enum WarnCode {
            #[default] UNKNOWN = 1919809,
            $($ident),*
        }
        $(pub const $ident: &'static str = $string_keyword;)*
        lazy_static! {
            pub static ref WARN_MSG: HashMap<WarnCode, &'static str> = {
                let mut mp = HashMap::new();
                $(mp.insert(WarnCode::$ident, $ident);)*
                mp
            };
        }
    };
    ($(
        $ident:ident = $string_keyword:expr
    ),*,) => {
        define_warn!($($ident = $string_keyword),*);
    };
}
define_warn! {
    UNREACHABLE_STATEMENT= "unreachable statement",
    UNUSED_VARIABLE = "unused variable",
    UNUSED_FUNCTION = "unused function",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagCode {
    Err(ErrorCode),
    Warn(WarnCode),
}

impl Default for DiagCode {
    fn default() -> Self {
        DiagCode::Err(ErrorCode::UNKNOWN)
    }
}
impl Display for DiagCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagCode::Err(e) => write!(f, "E{}", *e as u32),
            DiagCode::Warn(w) => write!(f, "W{}", *w as u32),
        }
    }
}

use lsp_types::{Diagnostic, DiagnosticSeverity, DiagnosticTag, Url};

use crate::Db;

use super::{
    ctx::Ctx,
    range::{Pos, Range},
};

/// # PLDiag
/// Diagnostic for pivot-lang
#[range]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PLDiag {
    code: DiagCode,
    help: Option<Box<String>>,
    labels: Vec<(Range, String, Option<(String, Vec<String>)>)>,
    pub source: Option<String>,
}

const PL_DIAG_SOURCE: &str = "plsp";

impl Pos {
    pub fn utf8_offset(&self, doc: &Source) -> usize {
        doc.line(self.line - 1).unwrap().offset() + self.column - 1
    }
}
use std::fmt::Debug;
impl PLDiag {
    pub fn print(&self, path: &str, f: impl Fn(&dyn Db, &str) -> Source + 'static, db: &dyn Db) {
        let mut colors = ColorGenerator::new();
        let mut rb = Report::build(
            self.get_report_kind(),
            path,
            self.range.start.utf8_offset(&f(db, path)),
        )
        .with_code(self.code)
        .with_message(self.get_msg());
        let mut labels = vec![];
        self.labels
            .iter()
            .find(|(r, p, _)| r.start == self.range.start && self.range.end == r.end && p == path)
            .or_else(|| {
                labels.push((
                    self.range,
                    path.to_string(),
                    Some(("here".to_string(), vec![])),
                ));
                None
            });

        for (range, path, txt) in labels.iter().chain(self.labels.iter()) {
            let color = colors.next();
            let mut lab;
            if let Some((tpl, args)) = txt {
                lab = Label::new((
                    path.as_str(),
                    range.start.utf8_offset(&f(db, path))..range.end.utf8_offset(&f(db, path)),
                ));
                let mut msg = tpl.clone();
                msg = msg.format(
                    &args
                        .iter()
                        .map(|s| s.fg(color).to_string())
                        .collect::<Vec<_>>(),
                );
                lab = lab.with_message(msg);
            } else {
                lab = Label::new((
                    path,
                    range.start.utf8_offset(&f(db, path))..range.end.utf8_offset(&f(db, path)),
                ));
            }
            rb = rb.with_label(lab.with_color(color));
        }
        if let Some(help) = &self.help {
            rb = rb.with_help(help);
        }
        let r = rb.finish();
        r.eprint(PLFileCache::new(db, Box::new(f))).unwrap();
    }
    fn get_report_kind(&self) -> ReportKind {
        match self.code {
            DiagCode::Err(_) => ReportKind::Error,
            DiagCode::Warn(_) => ReportKind::Warning,
        }
    }
    pub fn is_err(&self) -> bool {
        match self.code {
            DiagCode::Err(_) => true,
            DiagCode::Warn(_) => false,
        }
    }
    pub fn get_msg(&self) -> String {
        match self.code {
            DiagCode::Err(code) => ERR_MSG[&code].to_string(),
            DiagCode::Warn(code) => WARN_MSG[&code].to_string(),
        }
    }
    pub fn get_diagnostic(&self, p: &str, diags: &mut FxHashMap<String, Vec<Diagnostic>>) {
        let mut d = match self.code {
            DiagCode::Err(code) => Diagnostic::new_with_code_number(
                self.range.to_diag_range(),
                DiagnosticSeverity::ERROR,
                code as i32,
                Some(PL_DIAG_SOURCE.to_string()),
                ERR_MSG[&code].to_string(),
            ),
            DiagCode::Warn(code) => {
                let mut warn = Diagnostic::new_with_code_number(
                    self.range.to_diag_range(),
                    DiagnosticSeverity::WARNING,
                    code as i32,
                    Some(PL_DIAG_SOURCE.to_string()),
                    WARN_MSG[&code].to_string(),
                );
                if code == WarnCode::UNUSED_FUNCTION
                    || code == WarnCode::UNUSED_VARIABLE
                    || code == WarnCode::UNREACHABLE_STATEMENT
                {
                    warn.tags = Some(vec![DiagnosticTag::UNNECESSARY]);
                }
                warn
            }
        };
        let mut labels = vec![];
        self.labels.iter().for_each(|(range, file, txt)| {
            let mut lab = lsp_types::DiagnosticRelatedInformation {
                location: lsp_types::Location {
                    uri: Url::from_file_path(file).unwrap(),
                    range: range.to_diag_range(),
                },
                message: "related source here".to_string(),
            };
            if let Some((tpl, args)) = txt {
                lab.message = tpl.clone();
                lab.message = lab.message.format(args);
            }
            labels.push(lab);
        });
        d.related_information = Some(labels);
        let p = if let Some(source) = &self.source {
            source.clone()
        } else {
            p.to_string()
        };
        diags.entry(p.to_string()).or_default().push(d);
    }
    pub fn new_error(range: Range, code: ErrorCode) -> Self {
        PLDiag {
            range,
            code: DiagCode::Err(code),
            ..Default::default()
        }
    }
    pub fn set_source(&mut self, source: &str) -> &mut Self {
        self.source = Some(source.to_string());
        self
    }
    pub fn add_help(&mut self, help: &str) -> &mut Self {
        self.help = Some(Box::new(help.to_string()));
        self
    }
    pub fn add_to_ctx(&self, ctx: &Ctx) -> PLDiag {
        ctx.add_diag(self.clone())
    }

    /// Add a label to the diagnostic
    ///
    /// # Arguments
    /// * `range` - The src range of the label
    /// * `label` - The label text and arguments, you may use `format_label` macro to build it
    pub fn add_label(
        &mut self,
        range: Range,
        file: String,
        label: Option<(String, Vec<String>)>,
    ) -> &mut Self {
        self.labels.push((range, file, label));
        self
    }

    pub fn set_range(&mut self, range: Range) -> &mut Self {
        self.range = range;
        self
    }

    pub fn new_warn(range: Range, code: WarnCode) -> Self {
        PLDiag {
            range,
            code: DiagCode::Warn(code),
            ..Default::default()
        }
    }
}

pub struct PLFileCache<'a> {
    db: &'a dyn Db,
    f: Box<dyn Fn(&'a dyn Db, &str) -> Source>,
    cache: FxHashMap<String, Source>,
}

impl<'a> PLFileCache<'a> {
    pub fn new(db: &'a dyn Db, f: Box<dyn Fn(&'a dyn Db, &str) -> Source>) -> Self {
        Self {
            db,
            f,
            cache: Default::default(),
        }
    }
}

impl<'a> Cache<&str> for PLFileCache<'a> {
    fn fetch(&mut self, id: &&str) -> Result<&Source, Box<dyn std::fmt::Debug + '_>> {
        if !self.cache.contains_key(*id) {
            self.cache.insert((*id).to_string(), (self.f)(self.db, *id));
        }
        Ok(self.cache.get(*id).unwrap())
    }

    fn display<'b>(&self, id: &'b &str) -> Option<Box<dyn std::fmt::Display + 'b>> {
        Some(Box::new(id))
    }
}
