use crate::compiler::{
    AnalysisBlock, AnalysisLine, AnalysisResult, ComptimeNamespace, ComptimeValue,
    ComptimeValueKwBool, ComptimeValueKwInt, ComptimeValueKwString, ComptimeValueOptional, Env,
    PositionedError, RuntimeValue, Symbol, analyze, analyze_base, block_append, compiler_pos,
    throw_err,
};
use crate::comptime::{ComptimeValueKind, get_comptime};
use crate::ct::{
    CallArg, CtNamespace, KwInt, KwList, KwString, Type, TypeKwField, TypeUint8Array, list_items,
};
use crate::parser::{BlockToken, SyntaxNode, TokenPosition};

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
        }
    }
}

fn string(value: String) -> ComptimeValue {
    ComptimeValue::KwString(ComptimeValueKwString { value })
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
        (KwBuiltinOp::StringLen, [V::KwString(s)]) => int(s.value.chars().count() as i64),
        (KwBuiltinOp::ListNew, _) => V::KwList(args),
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
            item.clone()
        }
        (KwBuiltinOp::ListPush, [V::KwList(items), item]) => {
            let mut items = items.clone();
            items.push(item.clone());
            V::KwList(items)
        }
        (KwBuiltinOp::ListJoin, [V::KwList(items), V::KwString(separator)]) => {
            let parts: Vec<&str> = items
                .iter()
                .map(|item| match item {
                    V::KwString(s) => s.value.as_str(),
                    _ => unreachable!("join is only offered on lists of std.kw.string"),
                })
                .collect();
            string(parts.join(&separator.value))
        }
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
        _ => unreachable!("analysis only emits {} with matching arguments", op.name()),
    })
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
    if let Some(known) = known {
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

pub fn string_literal(
    env: &mut Env,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let bytes = analyze_base(
        env,
        Type::Uint8Array(TypeUint8Array),
        &SyntaxNode::Block(Box::new(ast.clone())),
        block,
    )?;
    let ComptimeValue::Uint8Array(bytes) = get_comptime(
        env,
        Some(ComptimeValueKind::Uint8Array),
        bytes.value,
        ast.pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::KwString(KwString),
        value: RuntimeValue::Comptime(string(String::from_utf8_lossy(&bytes.value).into_owned())),
    })
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
        (Type::KwList(_), "len") => property(KwBuiltinOp::ListLen),
        (Type::KwList(list), "get" | "push" | "join") => {
            if name == "join" && !matches!(*list.elem, Type::KwString(_)) {
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
            Type::KwString(KwString),
            Type::KwString(KwString),
        ),
        _ => unreachable!("value_field only offers get, push and join"),
    };
    let arg = analyze_as(env, &arg_ty, arg, block)?;
    emit(env, block, pos, op, vec![receiver, arg], result_ty)
}

pub fn type_field(ty: &Type, name: &str) -> Option<AnalysisResult> {
    match (ty, name) {
        (Type::KwString(_), "from_int") => Some(AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(std::rc::Rc::new(
                StringFromInt {
                    pos: compiler_pos(),
                },
            ))),
        }),
        _ => None,
    }
}

#[derive(Debug)]
struct StringFromInt {
    pos: TokenPosition,
}

impl ComptimeNamespace for StringFromInt {
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
            format!("std.kw.string.from_int has no field: {field}"),
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
        let n = analyze_as(env, &Type::KwInt(KwInt), arg, block)?;
        emit(
            env,
            block,
            pos,
            KwBuiltinOp::StringFromInt,
            vec![n],
            Type::KwString(KwString),
        )
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}
