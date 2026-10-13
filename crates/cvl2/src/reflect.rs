use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::{
    AnalysisBlock, AnalysisLine, AnalysisResult, AnalyzedFn, ComptimeValue, ComptimeValueKwInt,
    ComptimeValueKwList, ComptimeValueKwString, ComptimeValueTarget, ComptimeValueType, Env,
    PositionedError, Region, RuntimeValue, Symbol, TargetEnv, analyze, analyze_base,
    analyze_function, throw_err, with_target_env,
};
use crate::comptime::{ComptimeValueKind, get_comptime};
use crate::ct::{CallArg, KwString, Type, TypeTarget, TypeTuple, TypeUnknown, TypeVoid};
use crate::parser::{BlockToken, SyntaxNode, TokenPosition};
use crate::printers::{analysis_line_name, analysis_line_pos};
use crate::user_type::{LazyPrelude, UserType};

pub struct ReflectedFn {
    pub analyzed: AnalyzedFn,
    pub types: Vec<Type>,
}

#[derive(Clone)]
pub struct ReflectValue {
    pub func: Rc<ReflectedFn>,
    pub index: usize,
}

impl std::fmt::Debug for ReflectValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value({})", self.index)
    }
}

impl ReflectValue {
    pub fn ty(&self) -> Type {
        self.func.types[self.index].clone()
    }

    pub fn pos(&self) -> &TokenPosition {
        analysis_line_pos(&self.func.analyzed.block.lines[self.index])
    }
}

pub fn target_literal(
    env: &mut Env,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let name = analyze_base(
        env,
        Type::KwString(KwString),
        &SyntaxNode::Block(Box::new(ast.clone())),
        block,
    )?;
    let ComptimeValue::KwString(name) = get_comptime(
        env,
        Some(ComptimeValueKind::KwString),
        name.value,
        ast.pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::Target(TypeTarget),
        value: RuntimeValue::Comptime(ComptimeValue::Target(ComptimeValueTarget {
            env: TargetEnv::User(Symbol::new()),
            name: name.to_owned_string(),
        })),
    })
}

pub fn constant_type(value: &ComptimeValue) -> Type {
    match value {
        ComptimeValue::CInt(_) => Type::CInt(crate::ct::CInt),
        ComptimeValue::KwInt(_) => Type::KwInt(crate::ct::KwInt),
        ComptimeValue::KwBool(_) => Type::KwBool(crate::ct::KwBool),
        ComptimeValue::KwString(_) => Type::KwString(KwString),
        ComptimeValue::Void(_) => Type::Void(TypeVoid),
        ComptimeValue::Type(_) => Type::CtType(crate::ct::CtType),
        ComptimeValue::Target(_) => Type::Target(TypeTarget),
        _ => Type::Unknown(TypeUnknown),
    }
}

pub fn constant_int(value: &ComptimeValue) -> Option<i64> {
    match value {
        ComptimeValue::CInt(int) => Some(int.value.into()),
        ComptimeValue::KwInt(int) => Some(int.value),
        _ => None,
    }
}

pub fn base_type(env: &mut Env, ty: &Type, pos: &TokenPosition) -> Result<Type, PositionedError> {
    let mut ty = ty.clone();
    while let Type::User(user) = &ty {
        if user
            .static_symbol(env, crate::std_keys::std_key(crate::std_keys::StdKey::Repr))?
            .is_none()
        {
            break;
        }
        ty = user.repr(env, pos)?;
    }
    Ok(ty)
}

fn line_types(
    env: &mut Env,
    fn_value: &crate::compiler::ComptimeValueFn,
    analyzed: &AnalyzedFn,
) -> Result<Vec<Type>, PositionedError> {
    let block = &analyzed.block;
    let mut types: Vec<Type> = Vec::with_capacity(block.lines.len());
    let mut labels: HashMap<Symbol, Type> = HashMap::new();
    let type_of = |types: &[Type], value: &RuntimeValue| match value {
        RuntimeValue::Runtime(idx) if idx.1 == block.validate => types[idx.0].clone(),
        RuntimeValue::Runtime(_) => Type::Unknown(TypeUnknown),
        RuntimeValue::Comptime(value) => constant_type(value),
    };
    for line in &block.lines {
        let ty = match line {
            AnalysisLine::Args { .. } => fn_value.args().ty.clone(),
            AnalysisLine::Tuple { items, .. } => Type::Tuple(TypeTuple {
                children: items.iter().map(|item| type_of(&types, item)).collect(),
            }),
            AnalysisLine::TupleGet { tuple, index, .. } => match type_of(&types, tuple) {
                Type::Tuple(tuple) => tuple
                    .children
                    .get(*index)
                    .cloned()
                    .unwrap_or(Type::Unknown(TypeUnknown)),
                _ => Type::Unknown(TypeUnknown),
            },
            AnalysisLine::Call {
                method: RuntimeValue::Comptime(ComptimeValue::Fn(callee)),
                ..
            } => analyze_function(env, callee)?.ty,
            AnalysisLine::CBinary { .. } => Type::CInt(crate::ct::CInt),
            AnalysisLine::Emit { ty, .. } => ty.clone(),
            AnalysisLine::LabelBegin { label, ty, .. } => {
                labels.insert(*label, ty.clone());
                Type::Void(TypeVoid)
            }
            AnalysisLine::LabelEnd { label, .. } => labels
                .get(label)
                .cloned()
                .unwrap_or(Type::Unknown(TypeUnknown)),
            AnalysisLine::Break { .. }
            | AnalysisLine::RegionBegin { .. }
            | AnalysisLine::RegionEnd { .. } => Type::Void(TypeVoid),
            _ => Type::Unknown(TypeUnknown),
        };
        types.push(ty);
    }
    Ok(types)
}

struct Builder {
    func: Rc<ReflectedFn>,
    operand: UserType,
    line: UserType,
}

fn enum_value(
    env: &mut Env,
    ty: &UserType,
    case: &str,
    payload: Option<ComptimeValue>,
) -> Result<ComptimeValue, PositionedError> {
    let Some((index, _)) = ty.case(env, case)? else {
        return Err(throw_err(
            env,
            None,
            format!("{} has no case \"{case}\"", ty.name()),
            None,
            None,
        ));
    };
    Ok(ComptimeValue::Enum(crate::compiler::ComptimeValueEnum {
        case: index,
        payload: payload.map(Box::new),
    }))
}

fn struct_value(
    env: &mut Env,
    ty: &Type,
    mut fields: Vec<(&str, ComptimeValue)>,
) -> Result<ComptimeValue, PositionedError> {
    let Type::User(user) = ty else {
        return Err(throw_err(
            env,
            None,
            format!("expected a std.Struct, got {}", ty.dump()),
            None,
            None,
        ));
    };
    let mut values = Vec::new();
    for name in user.field_names(env)? {
        let Some(found) = fields.iter().position(|(field, _)| *field == name) else {
            return Err(throw_err(
                env,
                None,
                format!(
                    "std.reflect doesn't fill {}'s field \"{name}\"",
                    user.name()
                ),
                None,
                None,
            ));
        };
        values.push(fields.swap_remove(found).1);
    }
    if let Some((name, _)) = fields.first() {
        return Err(throw_err(
            env,
            None,
            format!("{} has no field \"{name}\"", user.name()),
            None,
            None,
        ));
    }
    Ok(ComptimeValue::Struct(values))
}

fn string(value: &str) -> ComptimeValue {
    ComptimeValue::KwString(ComptimeValueKwString::new(value.to_string()))
}

fn type_value(ty: Type) -> ComptimeValue {
    ComptimeValue::Type(ComptimeValueType { ty })
}

impl Builder {
    fn value(&self, index: usize) -> ComptimeValue {
        ComptimeValue::ReflectValue(ReflectValue {
            func: self.func.clone(),
            index,
        })
    }

    fn operand(
        &self,
        env: &mut Env,
        value: &RuntimeValue,
    ) -> Result<ComptimeValue, PositionedError> {
        match value {
            RuntimeValue::Runtime(idx) => {
                enum_value(env, &self.operand, "value", Some(self.value(idx.0)))
            }
            RuntimeValue::Comptime(value) => enum_value(
                env,
                &self.operand,
                "constant",
                Some(ComptimeValue::ReflectConstant(Rc::new(value.clone()))),
            ),
        }
    }

    fn line(
        &self,
        env: &mut Env,
        index: usize,
        labels: &mut HashMap<Symbol, usize>,
    ) -> Result<ComptimeValue, PositionedError> {
        let line = &self.func.analyzed.block.lines[index];
        let at = ("at", self.value(index));
        let (case, mut fields) = match line {
            AnalysisLine::Args { .. } => ("args", vec![]),
            AnalysisLine::Tuple { items, .. } => {
                let mut operands = Vec::new();
                for item in items {
                    operands.push(self.operand(env, item)?);
                }
                (
                    "tuple",
                    vec![(
                        "items",
                        ComptimeValue::KwList(ComptimeValueKwList::new(operands)),
                    )],
                )
            }
            AnalysisLine::TupleGet { tuple, index, .. } => (
                "tuple_get",
                vec![
                    ("tuple", self.operand(env, tuple)?),
                    (
                        "index",
                        ComptimeValue::KwInt(ComptimeValueKwInt {
                            value: *index as i64,
                        }),
                    ),
                ],
            ),
            AnalysisLine::Call { method, arg, .. } => (
                "call",
                vec![
                    ("callee", self.operand(env, method)?),
                    ("arg", self.operand(env, arg)?),
                ],
            ),
            AnalysisLine::CBinary { op, lhs, rhs, .. } => (
                "c_binary",
                vec![
                    ("op", string(op.as_str())),
                    ("lhs", self.operand(env, lhs)?),
                    ("rhs", self.operand(env, rhs)?),
                ],
            ),
            AnalysisLine::LabelBegin { label, ty, .. } => {
                labels.insert(*label, index);
                ("label_begin", vec![("ty", type_value(ty.clone()))])
            }
            AnalysisLine::LabelEnd { label, value, .. }
            | AnalysisLine::Break { label, value, .. } => {
                let Some(begin) = labels.get(label).copied() else {
                    return Err(throw_err(
                        env,
                        Some(analysis_line_pos(line).clone()),
                        "std.reflect found a break to a label that isn't open here",
                        None,
                        None,
                    ));
                };
                let case = match line {
                    AnalysisLine::LabelEnd { .. } => "label_end",
                    _ => "break",
                };
                (
                    case,
                    vec![
                        ("label", self.value(begin)),
                        ("value", self.operand(env, value)?),
                    ],
                )
            }
            AnalysisLine::RegionBegin {
                region: Region::CIf { cond },
                ..
            } => ("c_if", vec![("cond", self.operand(env, cond)?)]),
            AnalysisLine::RegionEnd { .. } => ("end", vec![]),
            AnalysisLine::Emit {
                data,
                data_ty,
                operands,
                ..
            } => {
                let mut items = Vec::new();
                for operand in operands {
                    items.push(self.operand(env, operand)?);
                }
                (
                    "emit",
                    vec![
                        (
                            "data",
                            ComptimeValue::ReflectData(Rc::new((data.clone(), data_ty.clone()))),
                        ),
                        (
                            "operands",
                            ComptimeValue::KwList(ComptimeValueKwList::new(items)),
                        ),
                    ],
                )
            }
            other => ("other", vec![("name", string(&analysis_line_name(other)))]),
        };
        fields.push(at);
        let Some((case_index, _)) = self.line.case(env, case)? else {
            return Err(throw_err(
                env,
                None,
                format!("std.reflect.Line has no case \"{case}\""),
                None,
                None,
            ));
        };
        let payload_ty = self.line.case_payload(env, case_index)?;
        let payload = struct_value(env, &payload_ty, fields)?;
        enum_value(env, &self.line, case, Some(payload))
    }
}

pub fn builtin_reflect_function_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = crate::compiler::builtin_list_args(env, &pos, &arg_ast, "std.reflect.function", 2)?;
    let mut args = Vec::new();
    for (item, ty) in items.iter().zip([
        Type::Target(TypeTarget),
        Type::ReflectFn(crate::ct::TypeReflectFn),
    ]) {
        let value = analyze(env, ty.clone(), item.pos.clone(), &item.items, block)?;
        args.push(ty.cast_into(env, block, value, item.pos.clone())?.value);
    }
    let function_ty = LazyPrelude::reflect().user_type(env, "Function")?;
    crate::kw::emit(
        env,
        block,
        pos,
        crate::kw::KwBuiltinOp::ReflectFunction,
        args,
        Type::User(function_ty),
    )
}

pub fn reflect_function(
    env: &mut Env,
    target: &ComptimeValueTarget,
    fn_value: &crate::compiler::ComptimeValueFn,
) -> Result<ComptimeValue, PositionedError> {
    let prelude = LazyPrelude::reflect();
    let function_ty = prelude.user_type(env, "Function")?;
    let operand = prelude.user_type(env, "Operand")?;
    let line = prelude.user_type(env, "Line")?;

    let (analyzed, types) = with_target_env(env, target.env, |env| {
        let analyzed = analyze_function(env, fn_value)?;
        let types = line_types(env, fn_value, &analyzed)?;
        Ok((analyzed, types))
    })?;
    let params = match &fn_value.args().ty {
        Type::Tuple(tuple) => tuple.children.clone(),
        Type::Void(_) => Vec::new(),
        other => vec![other.clone()],
    };
    let ret = analyzed.ty.clone();
    let result = analyzed.value.clone();
    let builder = Builder {
        func: Rc::new(ReflectedFn { analyzed, types }),
        operand,
        line,
    };
    let mut labels = HashMap::new();
    let mut lines = Vec::new();
    for index in 0..builder.func.analyzed.block.lines.len() {
        lines.push(builder.line(env, index, &mut labels)?);
    }
    let result = builder.operand(env, &result)?;
    struct_value(
        env,
        &Type::User(function_ty),
        vec![
            (
                "lines",
                ComptimeValue::KwList(ComptimeValueKwList::new(lines)),
            ),
            (
                "params",
                ComptimeValue::KwList(ComptimeValueKwList::new(
                    params.into_iter().map(type_value).collect(),
                )),
            ),
            ("ret", type_value(ret)),
            ("result", result),
        ],
    )
}

pub fn builtin_reflect_fail_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = crate::compiler::builtin_list_args(env, &pos, &arg_ast, "std.reflect.fail", 2)?;
    let mut args = Vec::new();
    for (item, ty) in items.iter().zip([
        Type::ReflectValue(crate::ct::TypeReflectValue),
        Type::KwString(KwString),
    ]) {
        let value = analyze(env, ty.clone(), item.pos.clone(), &item.items, block)?;
        args.push(ty.cast_into(env, block, value, item.pos.clone())?.value);
    }
    crate::kw::emit(
        env,
        block,
        pos,
        crate::kw::KwBuiltinOp::ReflectFail,
        args,
        Type::Never(crate::ct::TypeNever),
    )
}

pub fn builtin_emit_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = crate::compiler::builtin_list_args(env, &pos, &arg_ast, "std.emit", 3)?;
    let ty = analyze(
        env,
        Type::CtType(crate::ct::CtType),
        items[0].pos.clone(),
        &items[0].items,
        block,
    )?;
    let ComptimeValue::Type(ty) = get_comptime(
        env,
        Some(ComptimeValueKind::Type),
        ty.value,
        items[0].pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    let data = analyze(
        env,
        Type::Unknown(TypeUnknown),
        items[1].pos.clone(),
        &items[1].items,
        block,
    )?;
    let RuntimeValue::Comptime(data_value) = data.value else {
        return Err(throw_err(
            env,
            Some(items[1].pos.clone()),
            "std.emit's data must be known at compile time",
            None,
            None,
        ));
    };
    let operand_items = match crate::compiler::trim_ws(&items[2].items).as_slice() {
        [SyntaxNode::Block(list)] if list.tag == crate::parser::BracketTag::List => {
            crate::ct::list_items(env, list)?
        }
        _ => {
            return Err(throw_err(
                env,
                Some(items[2].pos.clone()),
                "std.emit's operands are a list in parentheses, as in (a, b)",
                None,
                None,
            ));
        }
    };
    let mut operands = Vec::new();
    for item in operand_items {
        let value = analyze(
            env,
            Type::Unknown(TypeUnknown),
            item.pos.clone(),
            &item.items,
            block,
        )?;
        operands.push(value.value);
    }
    let idx = crate::compiler::block_append(
        block,
        AnalysisLine::Emit {
            pos,
            data: data_value,
            data_ty: data.ty,
            operands,
            ty: ty.ty.clone(),
        },
    );
    Ok(AnalysisResult {
        ty: ty.ty,
        value: RuntimeValue::Runtime(idx),
    })
}
