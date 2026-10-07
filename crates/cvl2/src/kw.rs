use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::compiler::{
    AnalysisBlock, AnalysisLine, AnalysisResult, ComptimeNamespace, ComptimeValue,
    ComptimeValueEnum, ComptimeValueKwBool, ComptimeValueKwInt, ComptimeValueKwList,
    ComptimeValueKwString, ComptimeValueOptional, ComptimeValueVoid, Env, PositionedError,
    RuntimeValue, Symbol, analyze, block_append, compiler_pos, throw_err,
};
use crate::ct::{
    CallArg, CtNamespace, KwInt, KwList, KwString, KwText, Type, TypeKwField, list_items,
};
use crate::parser::{BlockToken, BracketTag, RawTag, SyntaxNode, TokenPosition, unescape_string};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KwBuiltinOp {
    StringFromInt,
    StringLen,
    ListNew,
    ListLen,
    ListGet,
    ListPush,
    ListJoin,
    OptionalSome,
    OptionalIsSome,
    OptionalUnwrap,
    StructNew,
    StructGet,
    EnumNew,
    EnumGet,
    EnumEq,
    EnumNe,
    BoolIs,
    StringConcat,
    TextParts,
    TextFromString,
    TextFromInt,
    TextFresh,
    TextRender,
}

#[derive(Debug)]
pub enum TextNode {
    Str(ComptimeValueKwString),
    Parts(Vec<Rc<TextNode>>),
    Fresh { id: u64, hint: String },
}

pub fn render_text(text: &TextNode) -> String {
    fn walk(
        text: &TextNode,
        out: &mut String,
        names: &mut HashMap<u64, String>,
        counts: &mut HashMap<String, usize>,
    ) {
        match text {
            TextNode::Str(s) => s.with_str(|s| out.push_str(s)),
            TextNode::Parts(parts) => {
                for part in parts {
                    walk(part, out, names, counts);
                }
            }
            TextNode::Fresh { id, hint } => {
                let name = names.entry(*id).or_insert_with(|| {
                    let count = counts.entry(hint.clone()).or_insert(0);
                    *count += 1;
                    format!("{hint}_{}", *count - 1)
                });
                out.push_str(name);
            }
        }
    }
    let mut out = String::new();
    walk(text, &mut out, &mut HashMap::new(), &mut HashMap::new());
    out
}

impl KwBuiltinOp {
    pub fn name(self) -> &'static str {
        match self {
            KwBuiltinOp::StringFromInt => "string_from_int",
            KwBuiltinOp::StringLen => "string_len",
            KwBuiltinOp::ListNew => "list_new",
            KwBuiltinOp::ListLen => "list_len",
            KwBuiltinOp::ListGet => "list_get",
            KwBuiltinOp::ListPush => "list_push",
            KwBuiltinOp::ListJoin => "list_join",
            KwBuiltinOp::OptionalSome => "optional_some",
            KwBuiltinOp::OptionalIsSome => "optional_is_some",
            KwBuiltinOp::OptionalUnwrap => "optional_unwrap",
            KwBuiltinOp::StructNew => "struct_new",
            KwBuiltinOp::StructGet => "struct_get",
            KwBuiltinOp::EnumNew => "enum_new",
            KwBuiltinOp::EnumGet => "enum_get",
            KwBuiltinOp::EnumEq => "enum_eq",
            KwBuiltinOp::EnumNe => "enum_ne",
            KwBuiltinOp::BoolIs => "bool_is",
            KwBuiltinOp::StringConcat => "string_concat",
            KwBuiltinOp::TextParts => "text_parts",
            KwBuiltinOp::TextFromString => "text_from_string",
            KwBuiltinOp::TextFromInt => "text_from_int",
            KwBuiltinOp::TextFresh => "text_fresh",
            KwBuiltinOp::TextRender => "text_render",
        }
    }
}

fn string(value: String) -> ComptimeValue {
    ComptimeValue::KwString(ComptimeValueKwString::new(value))
}

fn int(value: i64) -> ComptimeValue {
    ComptimeValue::KwInt(ComptimeValueKwInt { value })
}

pub fn eval(
    env: &mut Env,
    pos: &TokenPosition,
    op: KwBuiltinOp,
    args: Vec<ComptimeValue>,
) -> Result<ComptimeValue, PositionedError> {
    use ComptimeValue as V;
    Ok(match (op, args.as_slice()) {
        (KwBuiltinOp::StringFromInt, [V::KwInt(n)]) => string(n.value.to_string()),
        (KwBuiltinOp::StringLen, [V::KwString(s)]) => int(s.with_str(|s| s.chars().count()) as i64),
        (KwBuiltinOp::StringConcat, _) => {
            let mut parts = args.iter().map(|part| match part {
                V::KwString(s) => s.clone(),
                _ => unreachable!("string interpolation only joins strings"),
            });
            let first = parts
                .next()
                .unwrap_or_else(|| ComptimeValueKwString::new(String::new()));
            V::KwString(parts.fold(first, |acc, part| acc.concat(&part)))
        }
        (KwBuiltinOp::ListNew, _) => V::KwList(ComptimeValueKwList::new(args)),
        (KwBuiltinOp::ListLen, [V::KwList(items)]) => int(items.len() as i64),
        (KwBuiltinOp::ListGet, [V::KwList(items), V::KwInt(index)]) => {
            let found = usize::try_from(index.value).ok().and_then(|i| items.get(i));
            let Some(item) = found else {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!(
                        "index {} is out of range for a list of length {}",
                        index.value,
                        items.len()
                    ),
                    None,
                    None,
                ));
            };
            item
        }
        (KwBuiltinOp::ListPush, [V::KwList(items), item]) => V::KwList(items.push(item.clone())),
        (KwBuiltinOp::ListJoin, [V::KwList(items), V::KwString(separator)]) => {
            let mut out = String::new();
            for (i, item) in items.to_vec().iter().enumerate() {
                if i > 0 {
                    separator.with_str(|sep| out.push_str(sep));
                }
                match item {
                    V::KwString(s) => s.with_str(|s| out.push_str(s)),
                    _ => unreachable!("string join only takes strings"),
                }
            }
            string(out)
        }
        (KwBuiltinOp::ListJoin, [V::KwList(items), V::KwText(separator)]) => {
            let mut parts = Vec::new();
            for (i, item) in items.to_vec().into_iter().enumerate() {
                if i > 0 {
                    parts.push(separator.clone());
                }
                match item {
                    V::KwText(t) => parts.push(t),
                    _ => unreachable!("text join only takes text"),
                }
            }
            V::KwText(Rc::new(TextNode::Parts(parts)))
        }
        (KwBuiltinOp::TextParts, _) => V::KwText(Rc::new(TextNode::Parts(
            args.iter()
                .map(|part| match part {
                    V::KwText(t) => t.clone(),
                    _ => unreachable!("text parts are text"),
                })
                .collect(),
        ))),
        (KwBuiltinOp::TextFromString, [V::KwString(s)]) => {
            V::KwText(Rc::new(TextNode::Str(s.clone())))
        }
        (KwBuiltinOp::TextFromInt, [V::KwInt(n)]) => V::KwText(Rc::new(TextNode::Str(
            ComptimeValueKwString::new(n.value.to_string()),
        ))),
        (KwBuiltinOp::TextFresh, [V::KwString(hint)]) => {
            static NEXT_FRESH: AtomicU64 = AtomicU64::new(0);
            V::KwText(Rc::new(TextNode::Fresh {
                id: NEXT_FRESH.fetch_add(1, Ordering::Relaxed),
                hint: hint.to_owned_string(),
            }))
        }
        (KwBuiltinOp::TextRender, [V::KwText(t)]) => string(render_text(t)),
        (KwBuiltinOp::OptionalSome, [value]) => V::Optional(ComptimeValueOptional {
            some: Some(Box::new(value.clone())),
        }),
        (KwBuiltinOp::OptionalIsSome, [V::Optional(optional)]) => V::KwBool(ComptimeValueKwBool {
            value: optional.some.is_some(),
        }),
        (KwBuiltinOp::OptionalUnwrap, [V::Optional(optional)]) => match &optional.some {
            Some(value) => (**value).clone(),
            None => {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    "unwrapped std.kw.null with .?",
                    None,
                    None,
                ));
            }
        },
        (KwBuiltinOp::StructNew, _) => V::Struct(args),
        (KwBuiltinOp::StructGet, [V::Struct(fields), V::KwInt(index)]) => {
            fields[index.value as usize].clone()
        }
        (KwBuiltinOp::EnumNew, [V::KwInt(case)]) => V::Enum(ComptimeValueEnum {
            case: case.value as usize,
            payload: None,
        }),
        (KwBuiltinOp::EnumNew, [V::KwInt(case), payload]) => V::Enum(ComptimeValueEnum {
            case: case.value as usize,
            payload: Some(Box::new(payload.clone())),
        }),
        (KwBuiltinOp::EnumGet, [V::Enum(value), V::KwInt(case)]) => {
            V::Optional(ComptimeValueOptional {
                some: (value.case == case.value as usize).then(|| {
                    Box::new(match &value.payload {
                        Some(payload) => (**payload).clone(),
                        None => V::Void(ComptimeValueVoid),
                    })
                }),
            })
        }
        (KwBuiltinOp::EnumEq, [a @ V::Enum(_), b @ V::Enum(_)]) => V::KwBool(ComptimeValueKwBool {
            value: values_equal(a, b),
        }),
        (KwBuiltinOp::EnumNe, [a @ V::Enum(_), b @ V::Enum(_)]) => V::KwBool(ComptimeValueKwBool {
            value: !values_equal(a, b),
        }),
        (KwBuiltinOp::BoolIs, [V::KwBool(value), V::KwBool(want)]) => {
            V::Optional(ComptimeValueOptional {
                some: (value.value == want.value).then(|| Box::new(V::Void(ComptimeValueVoid))),
            })
        }
        _ => unreachable!("analysis only emits {} with matching arguments", op.name()),
    })
}

fn values_equal(a: &ComptimeValue, b: &ComptimeValue) -> bool {
    use ComptimeValue as V;
    match (a, b) {
        (V::Void(_), V::Void(_)) => true,
        (V::KwInt(a), V::KwInt(b)) => a.value == b.value,
        (V::KwBool(a), V::KwBool(b)) => a.value == b.value,
        (V::KwString(a), V::KwString(b)) => a == b,
        (V::KwText(a), V::KwText(b)) => render_text(a) == render_text(b),
        (V::CInt(a), V::CInt(b)) => a.value == b.value,
        (V::KwList(a), V::KwList(b)) => {
            let (a, b) = (a.to_vec(), b.to_vec());
            a.len() == b.len() && a.iter().zip(&b).all(|(a, b)| values_equal(a, b))
        }
        (V::Struct(a), V::Struct(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| values_equal(a, b))
        }
        (V::Optional(a), V::Optional(b)) => match (&a.some, &b.some) {
            (Some(a), Some(b)) => values_equal(a, b),
            (None, None) => true,
            _ => false,
        },
        (V::Enum(a), V::Enum(b)) => {
            a.case == b.case
                && match (&a.payload, &b.payload) {
                    (Some(a), Some(b)) => values_equal(a, b),
                    (None, None) => true,
                    _ => false,
                }
        }
        _ => false,
    }
}

pub fn emit(
    env: &mut Env,
    block: &mut AnalysisBlock,
    pos: TokenPosition,
    op: KwBuiltinOp,
    args: Vec<RuntimeValue>,
    ty: Type,
) -> Result<AnalysisResult, PositionedError> {
    let known: Option<Vec<ComptimeValue>> = args
        .iter()
        .map(|arg| match arg {
            RuntimeValue::Comptime(value) => Some(value.clone()),
            RuntimeValue::Runtime(_) => None,
        })
        .collect();
    if let (Some(known), false) = (known, op == KwBuiltinOp::TextFresh) {
        return Ok(AnalysisResult {
            ty,
            value: RuntimeValue::Comptime(eval(env, &pos, op, known)?),
        });
    }
    let idx = block_append(block, AnalysisLine::KwBuiltin { pos, op, args });
    Ok(AnalysisResult {
        ty,
        value: RuntimeValue::Runtime(idx),
    })
}

fn analyze_as(
    env: &mut Env,
    ty: &Type,
    arg: CallArg,
    block: &mut AnalysisBlock,
) -> Result<RuntimeValue, PositionedError> {
    let pos = arg.pos.clone();
    let value = analyze(env, ty.clone(), arg.pos, arg.ast, block)?;
    Ok(ty.cast_into(env, block, value, pos)?.value)
}

pub fn interpolated_literal(
    env: &mut Env,
    ty: &Type,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let is_text = matches!(ty, Type::KwText(_));
    let mut parts = Vec::new();
    for item in &ast.items {
        match item {
            SyntaxNode::Raw(raw) if raw.tag == RawTag::String => {
                let text = unescape_string(env, &raw.raw, raw.pos.clone())?;
                if text.is_empty() {
                    continue;
                }
                let value = string(text);
                parts.push(RuntimeValue::Comptime(if is_text {
                    eval(env, &raw.pos, KwBuiltinOp::TextFromString, vec![value])?
                } else {
                    value
                }));
            }
            SyntaxNode::Block(part) if part.tag == BracketTag::List => {
                let value = analyze(env, ty.clone(), part.pos.clone(), &part.items, block)?;
                parts.push(ty.cast_into(env, block, value, part.pos.clone())?.value);
            }
            other => {
                return Err(throw_err(
                    env,
                    Some(ast.pos.clone()),
                    format!("unexpected {other:?} in a string"),
                    None,
                    None,
                ));
            }
        }
    }
    let op = if is_text {
        KwBuiltinOp::TextParts
    } else {
        KwBuiltinOp::StringConcat
    };
    emit(env, block, ast.pos.clone(), op, parts, ty.clone())
}

pub fn list_literal(
    env: &mut Env,
    list: &KwList,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let mut items = Vec::new();
    for item in list_items(env, ast)? {
        let value = analyze(
            env,
            (*list.elem).clone(),
            item.pos.clone(),
            &item.items,
            block,
        )?;
        items.push(
            list.elem
                .cast_into(env, block, value, item.pos.clone())?
                .value,
        );
    }
    emit(
        env,
        block,
        ast.pos.clone(),
        KwBuiltinOp::ListNew,
        items,
        Type::KwList(list.clone()),
    )
}

pub fn value_field(
    env: &mut Env,
    block: &mut AnalysisBlock,
    ty: &Type,
    obj: &AnalysisResult,
    pos: &TokenPosition,
    name: &str,
) -> Result<Option<AnalysisResult>, PositionedError> {
    let property = |op| (op, Type::KwInt(KwInt));
    let (op, result_ty) = match (ty, name) {
        (Type::Optional(optional), "?") => (KwBuiltinOp::OptionalUnwrap, (*optional.child).clone()),
        (Type::KwString(_), "len") => property(KwBuiltinOp::StringLen),
        (Type::KwBool(_), "true" | "false") => {
            let want = RuntimeValue::Comptime(ComptimeValue::KwBool(ComptimeValueKwBool {
                value: name == "true",
            }));
            return emit(
                env,
                block,
                pos.clone(),
                KwBuiltinOp::BoolIs,
                vec![obj.value.clone(), want],
                Type::Optional(crate::ct::TypeOptional {
                    child: Box::new(Type::Void(crate::ct::TypeVoid)),
                }),
            )
            .map(Some);
        }
        (Type::KwList(_), "len") => property(KwBuiltinOp::ListLen),
        (Type::KwText(_), "render") => (KwBuiltinOp::TextRender, Type::KwString(KwString)),
        (Type::KwList(list), "get" | "push" | "join") => {
            if name == "join" && !matches!(*list.elem, Type::KwString(_) | Type::KwText(_)) {
                return Ok(None);
            }
            return Ok(Some(AnalysisResult {
                ty: Type::KwField(TypeKwField {
                    receiver: Box::new(ty.clone()),
                    name: name.to_string(),
                }),
                value: obj.value.clone(),
            }));
        }
        _ => return Ok(None),
    };
    emit(
        env,
        block,
        pos.clone(),
        op,
        vec![obj.value.clone()],
        result_ty,
    )
    .map(Some)
}

pub fn call_field(
    env: &mut Env,
    field: &TypeKwField,
    receiver: RuntimeValue,
    pos: TokenPosition,
    arg: CallArg,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let Type::KwList(list) = &*field.receiver else {
        unreachable!("only lists have callable fields")
    };
    let (op, arg_ty, result_ty) = match field.name.as_str() {
        "get" => (
            KwBuiltinOp::ListGet,
            Type::KwInt(KwInt),
            (*list.elem).clone(),
        ),
        "push" => (
            KwBuiltinOp::ListPush,
            (*list.elem).clone(),
            Type::KwList(list.clone()),
        ),
        "join" => (
            KwBuiltinOp::ListJoin,
            (*list.elem).clone(),
            (*list.elem).clone(),
        ),
        _ => unreachable!("value_field only offers get, push and join"),
    };
    let arg = analyze_as(env, &arg_ty, arg, block)?;
    emit(env, block, pos, op, vec![receiver, arg], result_ty)
}

pub fn type_field(ty: &Type, name: &str) -> Option<AnalysisResult> {
    let (op, arg, result) = match (ty, name) {
        (Type::KwString(_), "from_int") => (
            KwBuiltinOp::StringFromInt,
            Type::KwInt(KwInt),
            Type::KwString(KwString),
        ),
        (Type::KwText(_), "fresh") => (
            KwBuiltinOp::TextFresh,
            Type::KwString(KwString),
            Type::KwText(KwText),
        ),
        _ => return None,
    };
    Some(AnalysisResult {
        ty: Type::CtNamespace(CtNamespace),
        value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(StaticFn {
            name: format!("{}.{name}", ty.dump()),
            op,
            arg,
            result,
            pos: compiler_pos(),
        }))),
    })
}

#[derive(Debug)]
struct StaticFn {
    name: String,
    op: KwBuiltinOp,
    arg: Type,
    result: Type,
    pos: TokenPosition,
}

impl ComptimeNamespace for StaticFn {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        Err(throw_err(
            env,
            Some(pos),
            format!("{} has no field: {field}", self.name),
            None,
            None,
        ))
    }

    fn get_symbol(
        &self,
        _env: &mut Env,
        _pos: TokenPosition,
        _keychild: Type,
        _field: Symbol,
        _block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        Ok(None)
    }

    fn analyze_call(
        &self,
        env: &mut Env,
        _slot: Type,
        pos: TokenPosition,
        arg: CallArg<'_>,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let value = analyze_as(env, &self.arg, arg, block)?;
        emit(env, block, pos, self.op, vec![value], self.result.clone())
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}
