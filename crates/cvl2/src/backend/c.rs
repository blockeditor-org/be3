use std::collections::{HashMap, HashSet};

use crate::compiler::{
    AnalysisBlock, AnalysisLine, AnalyzedFn, ComptimeValue, ComptimeValueFn, Env, PositionedError,
    Region, RuntimeValue, Symbol, analyze_function, compiler_pos, throw_err,
};
use crate::comptime::{ComptimeValueKind, get_comptime};
use crate::ct::Type;
use crate::parser::TokenPosition;
use crate::printers::printers::RUNTIME_VALUE;
use crate::printers::{UNLIMITED_DEPTH, analysis_line_name, analysis_line_pos};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CValidatedIdentifierName(String);

impl CValidatedIdentifierName {
    pub fn new(name: String) -> Option<Self> {
        validate_c_name(&name).then_some(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn validate_c_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl CBinaryOp {
    pub fn from_token(token: &str) -> Option<Self> {
        Some(match token {
            "==" => CBinaryOp::Eq,
            "!=" => CBinaryOp::Ne,
            "<" => CBinaryOp::Lt,
            "<=" => CBinaryOp::Le,
            ">" => CBinaryOp::Gt,
            ">=" => CBinaryOp::Ge,
            "+" => CBinaryOp::Add,
            "-" => CBinaryOp::Sub,
            "*" => CBinaryOp::Mul,
            "/" => CBinaryOp::Div,
            "%" => CBinaryOp::Rem,
            _ => return None,
        })
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            CBinaryOp::Eq
                | CBinaryOp::Ne
                | CBinaryOp::Lt
                | CBinaryOp::Le
                | CBinaryOp::Gt
                | CBinaryOp::Ge
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            CBinaryOp::Eq => "==",
            CBinaryOp::Ne => "!=",
            CBinaryOp::Lt => "<",
            CBinaryOp::Le => "<=",
            CBinaryOp::Gt => ">",
            CBinaryOp::Ge => ">=",
            CBinaryOp::Add => "+",
            CBinaryOp::Sub => "-",
            CBinaryOp::Mul => "*",
            CBinaryOp::Div => "/",
            CBinaryOp::Rem => "%",
        }
    }
}

#[derive(Debug, Default)]
pub struct CCodegenCtx {
    fns: HashMap<ComptimeValueFn, CFnName>,
    fn_order: Vec<ComptimeValueFn>,
    used_names: HashSet<String>,
    gid: u64,
}

#[derive(Debug, Clone)]
struct CFnName {
    name: String,
    exported: bool,
}

impl CCodegenCtx {
    pub fn export(&mut self, fn_value: ComptimeValueFn, name: &str) -> bool {
        if self.fns.contains_key(&fn_value) || !self.used_names.insert(name.to_string()) {
            return false;
        }
        self.fns.insert(
            fn_value.clone(),
            CFnName {
                name: name.to_string(),
                exported: true,
            },
        );
        self.fn_order.push(fn_value);
        true
    }

    fn fn_name(&mut self, fn_value: &ComptimeValueFn) -> String {
        if let Some(existing) = self.fns.get(fn_value) {
            return existing.name.clone();
        }
        let name = loop {
            let candidate = format!("cvl2_fn_{}", self.gid);
            self.gid += 1;
            if self.used_names.insert(candidate.clone()) {
                break candidate;
            }
        };
        self.fns.insert(
            fn_value.clone(),
            CFnName {
                name: name.clone(),
                exported: false,
            },
        );
        self.fn_order.push(fn_value.clone());
        name
    }
}

pub fn codegen_c(env: &mut Env, ctx: &mut CCodegenCtx) -> Result<String, PositionedError> {
    let mut prototypes = Vec::new();
    let mut definitions = Vec::new();
    let mut idx = 0;
    while idx < ctx.fn_order.len() {
        let fn_value = ctx.fn_order[idx].clone();
        idx += 1;
        let name = ctx.fns[&fn_value].clone();
        let analyzed = analyze_function(env, &fn_value)?;
        let signature = c_signature(env, &name, &fn_value, &analyzed.ty)?;
        let body = codegen_c_body(env, ctx, &analyzed)?;
        prototypes.push(format!("{signature};\n"));
        definitions.push(format!("{signature} {{\n{body}}}\n"));
    }
    Ok(format!(
        "{}\n{}",
        prototypes.concat(),
        definitions.join("\n")
    ))
}

fn c_type(env: &mut Env, ty: &Type, pos: &TokenPosition) -> Result<&'static str, PositionedError> {
    match ty {
        Type::CInt(_) => Ok("int"),
        Type::User(t) => {
            let repr = t.repr(env, pos)?;
            c_type(env, &repr, pos)
        }
        Type::Void(_) => Ok("void"),
        other => Err(throw_err(
            env,
            Some(pos.clone()),
            format!("{} cannot be used in C", other.dump()),
            None,
            None,
        )),
    }
}

fn c_signature(
    env: &mut Env,
    name: &CFnName,
    fn_value: &ComptimeValueFn,
    ret: &Type,
) -> Result<String, PositionedError> {
    let pos = fn_value.pos();
    let Type::Tuple(args) = &fn_value.args().ty else {
        return Err(throw_err(
            env,
            Some(pos.clone()),
            "a C function's arguments must be a list",
            None,
            None,
        ));
    };
    let mut params = Vec::new();
    for (i, arg) in args.children.iter().enumerate() {
        params.push(format!("{} _a{i}", c_type(env, arg, pos)?));
    }
    let params = if params.is_empty() {
        "void".to_string()
    } else {
        params.join(", ")
    };
    let storage = if name.exported { "" } else { "static " };
    Ok(format!(
        "{storage}{} {}({params})",
        c_type(env, ret, pos)?,
        name.name
    ))
}

struct CLabel {
    label: Symbol,
    result: Option<String>,
    target: String,
    used: bool,
}

struct CBody<'a> {
    block: &'a AnalysisBlock,
    values: Vec<Option<String>>,
    out: String,
    indent: usize,
    temps: usize,
    labels: usize,
}

impl CBody<'_> {
    fn temp(&mut self) -> String {
        self.temps += 1;
        format!("_{}", self.temps - 1)
    }

    fn label(&mut self) -> String {
        self.labels += 1;
        format!("_l{}", self.labels - 1)
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn value(
        &self,
        env: &Env,
        value: &RuntimeValue,
        pos: &TokenPosition,
    ) -> Result<String, PositionedError> {
        match value {
            RuntimeValue::Comptime(ComptimeValue::CInt(int)) => Ok(int.value.to_string()),
            RuntimeValue::Runtime(idx) if idx.1 == self.block.validate => {
                self.values[idx.0].clone().ok_or_else(|| {
                    throw_err(
                        env,
                        Some(pos.clone()),
                        "this value is not available in C",
                        Some(vec![(
                            Some(analysis_line_pos(&self.block.lines[idx.0]).clone()),
                            "produced here".to_string(),
                        )]),
                        None,
                    )
                })
            }
            other => Err(throw_err(
                env,
                Some(pos.clone()),
                format!(
                    "TODO C value: {}",
                    RUNTIME_VALUE.dump(other, UNLIMITED_DEPTH)
                ),
                None,
                None,
            )),
        }
    }

    fn call_args(
        &self,
        env: &Env,
        arg: &RuntimeValue,
        pos: &TokenPosition,
    ) -> Result<Vec<String>, PositionedError> {
        match arg {
            RuntimeValue::Comptime(ComptimeValue::Void(_)) => Ok(Vec::new()),
            RuntimeValue::Runtime(idx) if idx.1 == self.block.validate => {
                match &self.block.lines[idx.0] {
                    AnalysisLine::Tuple { items, .. } => items
                        .iter()
                        .map(|item| self.value(env, item, pos))
                        .collect(),
                    _ => Ok(vec![self.value(env, arg, pos)?]),
                }
            }
            _ => Ok(vec![self.value(env, arg, pos)?]),
        }
    }
}

fn codegen_c_body(
    env: &mut Env,
    ctx: &mut CCodegenCtx,
    analyzed: &AnalyzedFn,
) -> Result<String, PositionedError> {
    let block = &analyzed.block;
    let mut body = CBody {
        block,
        values: vec![None; block.lines.len()],
        out: String::new(),
        indent: 1,
        temps: 0,
        labels: 0,
    };
    let mut labels: Vec<CLabel> = Vec::new();

    for (i, line) in block.lines.iter().enumerate() {
        match line {
            AnalysisLine::Args { .. } | AnalysisLine::Tuple { .. } => {}
            AnalysisLine::TupleGet { pos, tuple, index } => {
                let RuntimeValue::Runtime(tuple_idx) = tuple else {
                    return Err(throw_err(
                        env,
                        Some(pos.clone()),
                        "TODO C tuple_get on a comptime tuple",
                        None,
                        None,
                    ));
                };
                body.values[i] = Some(match &block.lines[tuple_idx.0] {
                    AnalysisLine::Args { .. } => format!("_a{index}"),
                    AnalysisLine::Tuple { items, .. } => body.value(env, &items[*index], pos)?,
                    _ => {
                        return Err(throw_err(
                            env,
                            Some(pos.clone()),
                            "TODO C tuple_get on this value",
                            None,
                            None,
                        ));
                    }
                });
            }
            AnalysisLine::CBinary { pos, op, lhs, rhs } => {
                let lhs = body.value(env, lhs, pos)?;
                let rhs = body.value(env, rhs, pos)?;
                let temp = body.temp();
                body.line(&format!("int {temp} = {lhs} {} {rhs};", op.as_str()));
                body.values[i] = Some(temp);
            }
            AnalysisLine::Call { pos, method, arg } => {
                let method = get_comptime(
                    env,
                    Some(ComptimeValueKind::Fn),
                    method.clone(),
                    pos.clone(),
                )?;
                let ComptimeValue::Fn(method) = method else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let name = ctx.fn_name(&method);
                let args = body.call_args(env, arg, pos)?.join(", ");
                let ret = analyze_function(env, &method)?.ty;
                if let Type::Void(_) = ret {
                    body.line(&format!("{name}({args});"));
                } else {
                    let ty = c_type(env, &ret, pos)?;
                    let temp = body.temp();
                    body.line(&format!("{ty} {temp} = {name}({args});"));
                    body.values[i] = Some(temp);
                }
            }
            AnalysisLine::LabelBegin { pos, label, ty } => {
                let result = match ty {
                    Type::Void(_) => None,
                    ty => {
                        let ty = c_type(env, ty, pos)?;
                        let temp = body.temp();
                        body.line(&format!("{ty} {temp};"));
                        Some(temp)
                    }
                };
                body.line("{");
                body.indent += 1;
                labels.push(CLabel {
                    label: *label,
                    result,
                    target: body.label(),
                    used: false,
                });
            }
            AnalysisLine::Break { pos, label, value } => {
                let Some(target) = labels.iter_mut().rev().find(|l| l.label == *label) else {
                    return Err(throw_err(
                        env,
                        Some(pos.clone()),
                        "cannot break to a label outside this function",
                        None,
                        None,
                    ));
                };
                target.used = true;
                let result = target.result.clone();
                let goto = format!("goto {};", target.target);
                if let Some(result) = result {
                    let value = body.value(env, value, pos)?;
                    body.line(&format!("{result} = {value};"));
                }
                body.line(&goto);
            }
            AnalysisLine::LabelEnd { pos, label, value } => {
                let Some(target) = labels.pop().filter(|l| l.label == *label) else {
                    unreachable!("label_end always matches the innermost label_begin")
                };
                let unreachable = matches!(value, RuntimeValue::Comptime(ComptimeValue::Void(_)));
                if let (Some(result), false) = (&target.result, unreachable) {
                    let value = body.value(env, value, pos)?;
                    body.line(&format!("{result} = {value};"));
                }
                body.indent -= 1;
                body.line("}");
                if target.used {
                    body.line(&format!("{}:;", target.target));
                }
                body.values[i] = target.result;
            }
            AnalysisLine::RegionBegin {
                pos,
                region: Region::CIf { cond },
            } => {
                let cond = body.value(env, cond, pos)?;
                body.line(&format!("if ({cond}) {{"));
                body.indent += 1;
            }
            AnalysisLine::RegionEnd { .. } => {
                body.indent -= 1;
                body.line("}");
            }
            AnalysisLine::RegionBegin { pos, .. }
            | AnalysisLine::KwBinary { pos, .. }
            | AnalysisLine::MutNew { pos, .. }
            | AnalysisLine::KwBuiltin { pos, .. }
            | AnalysisLine::MutGet { pos, .. }
            | AnalysisLine::MutSet { pos, .. } => {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!("{} is not supported in C", analysis_line_name(line)),
                    None,
                    None,
                ));
            }
            AnalysisLine::ComptimeKvListInit { pos }
            | AnalysisLine::ComptimeKvListAppend { pos, .. }
            | AnalysisLine::ComptimeFileCreate { pos, .. }
            | AnalysisLine::Emit { pos, .. } => {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!("{} is not supported in C", analysis_line_name(line)),
                    None,
                    None,
                ));
            }
        }
    }

    if !matches!(analyzed.ty, Type::Void(_)) {
        let value = body.value(env, &analyzed.value, &compiler_pos())?;
        body.line(&format!("return {value};"));
    }
    Ok(body.out)
}
