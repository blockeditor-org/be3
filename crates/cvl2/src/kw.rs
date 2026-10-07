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
    ReflectIndex,
    ReflectType,
    ConstantType,
    ConstantInt,
    TypeName,
    TypeBase,
    ReflectFail,
    DataType,
    DataAs,
    ConstantFn,
    MapNew,
    MapLen,
    MapGet,
    MapSet,
    MapKeys,
    MapValues,
    StringSplit,
    FolderFromMap,
    ReflectFunction,
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
            KwBuiltinOp::ReflectIndex => "reflect_index",
            KwBuiltinOp::ReflectType => "reflect_type",
            KwBuiltinOp::ConstantType => "constant_type",
            KwBuiltinOp::ConstantInt => "constant_int",
            KwBuiltinOp::TypeName => "type_name",
            KwBuiltinOp::TypeBase => "type_base",
            KwBuiltinOp::ReflectFail => "reflect_fail",
            KwBuiltinOp::DataType => "data_type",
            KwBuiltinOp::DataAs => "data_as",
            KwBuiltinOp::ConstantFn => "constant_fn",
            KwBuiltinOp::MapNew => "map_new",
            KwBuiltinOp::MapLen => "map_len",
            KwBuiltinOp::MapGet => "map_get",
            KwBuiltinOp::MapSet => "map_set",
            KwBuiltinOp::MapKeys => "map_keys",
            KwBuiltinOp::MapValues => "map_values",
            KwBuiltinOp::StringSplit => "string_split",
            KwBuiltinOp::FolderFromMap => "folder_from_map",
            KwBuiltinOp::ReflectFunction => "reflect_function",
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
        (KwBuiltinOp::ReflectIndex, [V::ReflectValue(v)]) => int(v.index as i64),
        (KwBuiltinOp::ReflectType, [V::ReflectValue(v)]) => {
            V::Type(crate::compiler::ComptimeValueType { ty: v.ty() })
        }
        (KwBuiltinOp::ConstantType, [V::ReflectConstant(c)]) => {
            V::Type(crate::compiler::ComptimeValueType {
                ty: crate::reflect::constant_type(c),
            })
        }
        (KwBuiltinOp::ConstantInt, [V::ReflectConstant(c)]) => V::Optional(ComptimeValueOptional {
            some: crate::reflect::constant_int(c).map(|n| Box::new(int(n))),
        }),
        (KwBuiltinOp::TypeName, [V::Type(t)]) => string(t.ty.dump()),
        (KwBuiltinOp::ConstantFn, [V::ReflectConstant(c)]) => V::Optional(ComptimeValueOptional {
            some: match &**c {
                V::Fn(f) => Some(Box::new(V::Fn(f.clone()))),
                _ => None,
            },
        }),
        (KwBuiltinOp::MapNew, _) => {
            let mut entries: Vec<(ComptimeValue, ComptimeValue)> = Vec::new();
            for pair in args.chunks(2) {
                map_insert(env, pos, &mut entries, pair[0].clone(), pair[1].clone())?;
            }
            V::KwMap(Rc::new(entries))
        }
        (KwBuiltinOp::ReflectFunction, [V::Target(target), V::Fn(f)]) => {
            crate::reflect::reflect_function(env, target, f)?
        }
        (KwBuiltinOp::StringSplit, [V::KwString(text), V::KwString(separator)]) => {
            let parts = text.with_str(|text| {
                separator.with_str(|separator| {
                    if separator.is_empty() {
                        return vec![text.to_string()];
                    }
                    text.split(separator)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
            });
            V::KwList(ComptimeValueKwList::new(
                parts.into_iter().map(string).collect(),
            ))
        }
        (KwBuiltinOp::FolderFromMap, [V::KwMap(entries)]) => {
            let mut files = Vec::new();
            for (name, content) in entries.iter() {
                let V::KwString(name) = name else {
                    unreachable!("analysis only folds string-keyed maps into folders")
                };
                let artifact = match content {
                    V::KwString(s) => crate::compiler::ComptimeValueBuildArtifact::File(
                        crate::compiler::ComptimeFile {
                            value: s.to_owned_string().into_bytes(),
                        },
                    ),
                    V::KwText(t) => crate::compiler::ComptimeValueBuildArtifact::File(
                        crate::compiler::ComptimeFile {
                            value: render_text(t).into_bytes(),
                        },
                    ),
                    V::BuildArtifact(artifact) => artifact.clone(),
                    _ => unreachable!("analysis only folds files, strings and text into folders"),
                };
                files.push((name.to_owned_string(), artifact));
            }
            V::BuildArtifact(crate::compiler::ComptimeValueBuildArtifact::Folder(
                crate::compiler::ComptimeFolder { value: files },
            ))
        }
        (KwBuiltinOp::MapLen, [V::KwMap(entries)]) => int(entries.len() as i64),
        (KwBuiltinOp::MapGet, [V::KwMap(entries), key]) => V::Optional(ComptimeValueOptional {
            some: entries
                .iter()
                .find(|(k, _)| values_equal(k, key))
                .map(|(_, v)| Box::new(v.clone())),
        }),
        (KwBuiltinOp::MapSet, [V::KwMap(entries), key, value]) => {
            let mut entries = (**entries).clone();
            map_insert(env, pos, &mut entries, key.clone(), value.clone())?;
            V::KwMap(Rc::new(entries))
        }
        (KwBuiltinOp::MapKeys, [V::KwMap(entries)]) => V::KwList(ComptimeValueKwList::new(
            entries.iter().map(|(k, _)| k.clone()).collect(),
        )),
        (KwBuiltinOp::MapValues, [V::KwMap(entries)]) => V::KwList(ComptimeValueKwList::new(
            entries.iter().map(|(_, v)| v.clone()).collect(),
        )),
        (KwBuiltinOp::DataType, [V::ReflectData(data)]) => {
            V::Type(crate::compiler::ComptimeValueType { ty: data.1.clone() })
        }
        (KwBuiltinOp::DataAs, [V::ReflectData(data), V::Type(t)]) => {
            V::Optional(ComptimeValueOptional {
                some: (data.1 == t.ty).then(|| Box::new(data.0.clone())),
            })
        }
        (KwBuiltinOp::TypeBase, [V::Type(t)]) => V::Type(crate::compiler::ComptimeValueType {
            ty: crate::reflect::base_type(env, &t.ty, pos)?,
        }),
        (KwBuiltinOp::ReflectFail, [V::ReflectValue(at), V::KwString(message)]) => {
            return Err(throw_err(
                env,
                Some(at.pos().clone()),
                message.to_owned_string(),
                Some(vec![(Some(pos.clone()), "reported here".to_string())]),
                None,
            ));
        }
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

fn map_insert(
    env: &mut Env,
    pos: &TokenPosition,
    entries: &mut Vec<(ComptimeValue, ComptimeValue)>,
    key: ComptimeValue,
    value: ComptimeValue,
) -> Result<(), PositionedError> {
    if !can_compare(&key) {
        return Err(throw_err(
            env,
            Some(pos.clone()),
            "this value can't be a std.kw.map key",
            None,
            None,
        ));
    }
    match entries.iter_mut().find(|(k, _)| values_equal(k, &key)) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
    Ok(())
}

pub fn can_compare(value: &ComptimeValue) -> bool {
    use ComptimeValue as V;
    match value {
        V::Void(_)
        | V::KwInt(_)
        | V::KwBool(_)
        | V::KwString(_)
        | V::CInt(_)
        | V::Fn(_)
        | V::Type(_)
        | V::Target(_)
        | V::OperatorName(_) => true,
        V::KwList(items) => items.to_vec().iter().all(can_compare),
        V::Struct(items) => items.iter().all(can_compare),
        V::Optional(optional) => optional.some.as_deref().is_none_or(can_compare),
        V::Enum(value) => value.payload.as_deref().is_none_or(can_compare),
        _ => false,
    }
}

pub fn values_equal(a: &ComptimeValue, b: &ComptimeValue) -> bool {
    use ComptimeValue as V;
    match (a, b) {
        (V::Fn(a), V::Fn(b)) => a == b,
        (V::Type(a), V::Type(b)) => a.ty == b.ty,
        (V::Target(a), V::Target(b)) => a.env == b.env,
        (V::OperatorName(a), V::OperatorName(b)) => a.value == b.value,
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
    let folds = !matches!(op, KwBuiltinOp::TextFresh | KwBuiltinOp::ReflectFail);
    if let (Some(known), true) = (known, folds) {
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

pub fn map_literal(
    env: &mut Env,
    map: &crate::ct::KwMap,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let mut args = Vec::new();
    for line in list_items(env, ast)? {
        let Some((key, _, value)) =
            crate::compiler::read_binary2(env, &line.items, crate::parser::OpTag::Pub)?
        else {
            return Err(throw_err(
                env,
                Some(line.pos.clone()),
                "expected key .= value in a std.kw.map literal",
                None,
                None,
            ));
        };
        for (seg, ty) in [(key, &*map.key), (value, &*map.value)] {
            let analyzed = analyze(env, ty.clone(), seg.pos.clone(), &seg.items, block)?;
            args.push(ty.cast_into(env, block, analyzed, seg.pos.clone())?.value);
        }
    }
    emit(
        env,
        block,
        ast.pos.clone(),
        KwBuiltinOp::MapNew,
        args,
        Type::KwMap(map.clone()),
    )
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
        (Type::ReflectValue(_), "index") => property(KwBuiltinOp::ReflectIndex),
        (Type::ReflectValue(_), "ty") => {
            (KwBuiltinOp::ReflectType, Type::CtType(crate::ct::CtType))
        }
        (Type::ReflectConstant(_), "ty") => {
            (KwBuiltinOp::ConstantType, Type::CtType(crate::ct::CtType))
        }
        (Type::KwMap(_), "len") => property(KwBuiltinOp::MapLen),
        (Type::KwString(_), "split") => {
            return Ok(Some(AnalysisResult {
                ty: Type::KwField(TypeKwField {
                    receiver: Box::new(ty.clone()),
                    name: name.to_string(),
                }),
                value: obj.value.clone(),
            }));
        }
        (Type::KwMap(map), "keys") => (
            KwBuiltinOp::MapKeys,
            Type::KwList(KwList {
                elem: map.key.clone(),
            }),
        ),
        (Type::KwMap(map), "values") => (
            KwBuiltinOp::MapValues,
            Type::KwList(KwList {
                elem: map.value.clone(),
            }),
        ),
        (Type::KwMap(_), "get" | "set") => {
            return Ok(Some(AnalysisResult {
                ty: Type::KwField(TypeKwField {
                    receiver: Box::new(ty.clone()),
                    name: name.to_string(),
                }),
                value: obj.value.clone(),
            }));
        }
        (Type::ReflectConstant(_), "fn") => (
            KwBuiltinOp::ConstantFn,
            Type::Optional(crate::ct::TypeOptional {
                child: Box::new(Type::ReflectFn(crate::ct::TypeReflectFn)),
            }),
        ),
        (Type::ReflectData(_), "ty") => (KwBuiltinOp::DataType, Type::CtType(crate::ct::CtType)),
        (Type::ReflectData(_), "as") => {
            return Ok(Some(AnalysisResult {
                ty: Type::KwField(TypeKwField {
                    receiver: Box::new(ty.clone()),
                    name: name.to_string(),
                }),
                value: obj.value.clone(),
            }));
        }
        (Type::ReflectConstant(_), "int") => (
            KwBuiltinOp::ConstantInt,
            Type::Optional(crate::ct::TypeOptional {
                child: Box::new(Type::KwInt(KwInt)),
            }),
        ),
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
    if let Type::ReflectData(_) = &*field.receiver {
        let ty_value = analyze_as(env, &Type::CtType(crate::ct::CtType), arg, block)?;
        let RuntimeValue::Comptime(ComptimeValue::Type(ty)) = &ty_value else {
            return Err(throw_err(
                env,
                Some(pos),
                "std.reflect.Data.as needs a type known at compile time",
                None,
                None,
            ));
        };
        let result = Type::Optional(crate::ct::TypeOptional {
            child: Box::new(ty.ty.clone()),
        });
        return emit(
            env,
            block,
            pos,
            KwBuiltinOp::DataAs,
            vec![receiver, ty_value],
            result,
        );
    }
    if let Type::KwString(_) = &*field.receiver {
        let separator = analyze_as(env, &Type::KwString(KwString), arg, block)?;
        return emit(
            env,
            block,
            pos,
            KwBuiltinOp::StringSplit,
            vec![receiver, separator],
            Type::KwList(KwList {
                elem: Box::new(Type::KwString(KwString)),
            }),
        );
    }
    if let Type::KwMap(map) = &*field.receiver {
        let optional = |ty: &Type| {
            Type::Optional(crate::ct::TypeOptional {
                child: Box::new(ty.clone()),
            })
        };
        if field.name == "get" {
            let key = analyze_as(env, &map.key, arg, block)?;
            return emit(
                env,
                block,
                pos,
                KwBuiltinOp::MapGet,
                vec![receiver, key],
                optional(&map.value),
            );
        }
        let items = match crate::ct::call_list_items(env, &arg)? {
            Some(items) if items.len() == 2 => items,
            _ => {
                return Err(throw_err(
                    env,
                    Some(pos),
                    "set takes a key and a value, as in m.set(k, v)",
                    None,
                    None,
                ));
            }
        };
        let mut args = vec![receiver];
        for (item, ty) in items.iter().zip([&*map.key, &*map.value]) {
            let value = analyze(env, ty.clone(), item.pos.clone(), &item.items, block)?;
            args.push(ty.cast_into(env, block, value, item.pos.clone())?.value);
        }
        return emit(
            env,
            block,
            pos,
            KwBuiltinOp::MapSet,
            args,
            Type::KwMap(map.clone()),
        );
    }
    let Type::KwList(list) = &*field.receiver else {
        unreachable!("only lists, maps and std.reflect.Data have callable fields")
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

pub fn static_fn(name: &str, op: KwBuiltinOp, arg: Type, result: Type) -> AnalysisResult {
    AnalysisResult {
        ty: Type::CtNamespace(CtNamespace),
        value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(StaticFn {
            name: name.to_string(),
            op,
            arg,
            result,
            pos: compiler_pos(),
        }))),
    }
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
