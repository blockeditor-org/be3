#![allow(clippy::wrong_self_convention)]

use std::collections::HashMap;

use std::rc::Rc;

use crate::backend::c::{CBinaryOp, CValidatedIdentifierName, validate_c_name};
use crate::compiler::{
    AnalysisBlock, AnalysisLine, AnalysisResult, Binary2, ComptimeFolder, ComptimeNamespace,
    ComptimeValue, ComptimeValueBuildArtifact, ComptimeValueCExportName, ComptimeValueCInt,
    ComptimeValueDeclaration, ComptimeValueExportList, ComptimeValueExportListEntry,
    ComptimeValueKey, ComptimeValueKwBool, ComptimeValueKwInt, ComptimeValueMcIdentifier,
    ComptimeValueMcNbtRef, ComptimeValueMcResult, ComptimeValueOperatorName,
    ComptimeValueUint8Array, ComptimeValueVoid, ConsumedErrorToken, Env, PositionedError, Region,
    RuntimeValue, Symbol, Uint8ArraySourcemapEntry, add_err, analyze, analyze_base, analyze_block,
    analyze_call, analyze_function, block_append, compiler_pos, create_declaration, empty_block,
    get_declaration, read_binary, throw_consumed_err, throw_err, trim_ws,
};
use crate::comptime::{ComptimeValueKind, comptime_eval, get_comptime};
use crate::parser::{
    BlockToken, BracketTag, ErrorStyle, IdentifierTag, IdentifierToken, OpTag,
    OperatorSegmentToken, RawTag, SyntaxNode, TokenPosition, is_overloadable_operator,
    unescape_string,
};
use crate::printers::printers::AST_NODE;
use crate::std_keys::{
    LiteralKind, OperatorKind, StdKey, std_key, symbol_operator, symbol_std_key,
};

pub struct CallArg<'a> {
    pub pos: TokenPosition,
    pub ast: &'a [SyntaxNode],
}

#[derive(Debug, Clone)]
pub struct ImplicitArgRet {
    pub arg: Type,
    pub ret: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Void(TypeVoid),
    Unknown(TypeUnknown),
    Fn(TypeFn),
    Uint8Array(TypeUint8Array),
    Tuple(TypeTuple),
    Optional(TypeOptional),
    CtExportList(CtExportList),
    CtKey(CtKey),
    CtAst(CtAst),
    CtNamespace(CtNamespace),
    CtType(CtType),
    CtBuildArtifact(CtBuildArtifact),
    McResult(McResult),
    McNbtRef(McNbtRef),
    McIdentifier(McIdentifier),
    CExportName(CExportName),
    CInt(CInt),
    CIf(CIf),
    Label(TypeLabel),
    OperatorName(OperatorName),
    Bound(TypeBound),
    KwInt(KwInt),
    KwBool(KwBool),
    KwIf(KwIf),
    KwIfResult(KwIfResult),
    User(crate::user_type::UserType),
    InlineFn(TypeInlineFn),
    KwMut(KwMut),
    Never(TypeNever),
    Infer(TypeInfer),
    KwString(KwString),
    KwText(KwText),
    KwList(KwList),
    KwField(TypeKwField),
    Null(TypeNull),
    KwIfOptional(TypeKwIfOptional),
    BoundName(TypeBoundName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeBoundName {
    pub receiver: Box<Type>,
    pub name: String,
}

fn enum_comparison(key: Symbol) -> Option<crate::kw::KwBuiltinOp> {
    match symbol_operator(key)? {
        (OperatorKind::Lhs, op) if op == "==" => Some(crate::kw::KwBuiltinOp::EnumEq),
        (OperatorKind::Lhs, op) if op == "!=" => Some(crate::kw::KwBuiltinOp::EnumNe),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeNull;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeKwIfOptional {
    pub child: Box<Type>,
    pub bind: Option<Box<KwIfBinding>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwIfBinding {
    pub name: String,
    pub pos: TokenPosition,
    pub ty: Type,
}

impl TypeKwIfOptional {
    fn analyze_call(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        optional: RuntimeValue,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let is_some = crate::kw::emit(
            env,
            block,
            pos.clone(),
            crate::kw::KwBuiltinOp::OptionalIsSome,
            vec![optional.clone()],
            Type::KwBool(KwBool),
        )?;
        block_append(
            block,
            AnalysisLine::RegionBegin {
                pos: pos.clone(),
                region: Region::KwIf {
                    cond: is_some.value,
                },
            },
        );

        let is_block = matches!(
            trim_ws(arg_in.ast).as_slice(),
            [SyntaxNode::Block(b)] if b.tag == BracketTag::Code
        );
        let body_never = if is_block {
            let saved = env.scope.bindings.clone();
            if let Some(bind) = &self.bind {
                let payload = block_append(
                    block,
                    AnalysisLine::KwBuiltin {
                        pos: bind.pos.clone(),
                        op: crate::kw::KwBuiltinOp::OptionalUnwrap,
                        args: vec![optional.clone()],
                    },
                );
                let payload = AnalysisResult {
                    ty: (*self.child).clone(),
                    value: RuntimeValue::Runtime(payload),
                };
                let payload = bind.ty.cast_into(env, block, payload, bind.pos.clone())?;
                let mut bindings = saved.borrow().clone();
                bindings.insert(
                    bind.name.clone(),
                    crate::compiler::Binding::Runtime {
                        pos: bind.pos.clone(),
                        runtime: payload,
                    },
                );
                env.scope.bindings = Rc::new(std::cell::RefCell::new(bindings));
            }
            let body = analyze(env, Type::Void(TypeVoid), arg_in.pos, arg_in.ast, block);
            env.scope.bindings = saved;
            matches!(body?.ty, Type::Never(_))
        } else if self.bind.is_some() {
            return Err(throw_err(
                env,
                Some(arg_in.pos),
                "std.kw.if (v := opt) takes a { ... } block",
                None,
                None,
            ));
        } else {
            let body = analyze(
                env,
                Type::Unknown(TypeUnknown),
                arg_in.pos.clone(),
                arg_in.ast,
                block,
            )?;
            let ComptimeValue::Fn(func) = get_comptime(
                env,
                Some(ComptimeValueKind::Fn),
                body.value,
                arg_in.pos.clone(),
            )?
            else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let payload = block_append(
                block,
                AnalysisLine::KwBuiltin {
                    pos: pos.clone(),
                    op: crate::kw::KwBuiltinOp::OptionalUnwrap,
                    args: vec![optional],
                },
            );
            let body = crate::user_type::inline_call(
                env,
                &func,
                vec![AnalysisResult {
                    ty: (*self.child).clone(),
                    value: RuntimeValue::Runtime(payload),
                }],
                CallArg {
                    pos: arg_in.pos,
                    ast: &[],
                },
                block,
            )?;
            matches!(body.ty, Type::Never(_))
        };
        let end = block_append(block, AnalysisLine::RegionEnd { pos });
        Ok(AnalysisResult {
            ty: Type::KwIfResult(KwIfResult { body_never }),
            value: RuntimeValue::Runtime(end),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwString;

#[derive(Debug, Clone, PartialEq)]
pub struct KwText;

#[derive(Debug, Clone, PartialEq)]
pub struct KwList {
    pub elem: Box<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeKwField {
    pub receiver: Box<Type>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfer {
    pub key: (usize, crate::compiler::ComptimeSnapshot),
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwMut {
    pub inner: Box<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeNever;

fn is_mut_assign(ty: &Type, key: Symbol) -> bool {
    matches!(ty, Type::KwMut(_))
        && symbol_operator(key) == Some((OperatorKind::Lhs, "=".to_string()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeInlineFn {
    pub func: crate::compiler::ComptimeValueFn,
}

impl Type {
    pub fn cast_into(
        &self,
        env: &mut Env,
        block: &mut AnalysisBlock,
        other: AnalysisResult,
        pos: TokenPosition,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::McResult(m) => m.cast_into(env, block, other, pos),
            Type::KwText(_) if matches!(other.ty, Type::KwString(_) | Type::KwInt(_)) => {
                let op = match other.ty {
                    Type::KwString(_) => crate::kw::KwBuiltinOp::TextFromString,
                    _ => crate::kw::KwBuiltinOp::TextFromInt,
                };
                crate::kw::emit(env, block, pos, op, vec![other.value], self.clone())
            }
            Type::CtBuildArtifact(artifact)
                if matches!(other.ty, Type::KwString(_) | Type::KwText(_))
                    && artifact.narrow != Some(CtBuildArtifactNarrow::Folder) =>
            {
                let idx = block_append(
                    block,
                    AnalysisLine::ComptimeFileCreate {
                        pos,
                        value: other.value,
                    },
                );
                Ok(AnalysisResult {
                    ty: Type::CtBuildArtifact(CtBuildArtifact {
                        narrow: Some(CtBuildArtifactNarrow::File),
                    }),
                    value: RuntimeValue::Runtime(idx),
                })
            }
            Type::Unknown(_) | Type::Infer(_) => Ok(other),
            Type::Optional(optional) if other.ty != *self => {
                if let Type::Null(_) = other.ty {
                    return Ok(AnalysisResult {
                        ty: self.clone(),
                        value: other.value,
                    });
                }
                if other.ty == *optional.child {
                    return crate::kw::emit(
                        env,
                        block,
                        pos,
                        crate::kw::KwBuiltinOp::OptionalSome,
                        vec![other.value],
                        self.clone(),
                    );
                }
                if let Type::Never(_) = other.ty {
                    return Ok(AnalysisResult {
                        ty: self.clone(),
                        value: other.value,
                    });
                }
                Err(throw_err(
                    env,
                    Some(pos),
                    format!("expected {}, got {}", self.dump(), other.ty.dump()),
                    None,
                    None,
                ))
            }
            _ if matches!(other.ty, Type::Never(_)) => Ok(AnalysisResult {
                ty: self.clone(),
                value: other.value,
            }),
            Type::Void(_) if matches!(other.ty, Type::KwIfResult(_)) => Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            }),
            _ => {
                if &other.ty == self {
                    Ok(other)
                } else {
                    Err(throw_err(
                        env,
                        Some(pos),
                        format!("expected {}, got {}", self.dump(), other.ty.dump()),
                        None,
                        None,
                    ))
                }
            }
        }
    }

    fn supports_literal(&self, kind: LiteralKind) -> bool {
        match kind {
            LiteralKind::String => matches!(
                self,
                Type::Uint8Array(_)
                    | Type::CtBuildArtifact(_)
                    | Type::McNbtRef(_)
                    | Type::McIdentifier(_)
                    | Type::CExportName(_)
                    | Type::OperatorName(_)
                    | Type::CtKey(_)
                    | Type::KwString(_)
                    | Type::KwText(_)
            ),
            LiteralKind::Map => matches!(
                self,
                Type::CtExportList(_) | Type::CtBuildArtifact(_) | Type::CtType(_)
            ),
            LiteralKind::List => matches!(self, Type::Tuple(_) | Type::KwList(_)),
            LiteralKind::Number => {
                matches!(self, Type::McResult(_) | Type::CInt(_) | Type::KwInt(_))
            }
        }
    }

    fn analyze_literal(
        &self,
        env: &mut Env,
        kind: LiteralKind,
        pos: TokenPosition,
        node: &SyntaxNode,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let slot = self.clone();
        let result = match (kind, self) {
            (LiteralKind::String, Type::Uint8Array(t)) => literal_block(node, BracketTag::String)
                .map(|ast| t.from_string(env, slot, ast, block)),
            (LiteralKind::String, Type::CtBuildArtifact(t)) => {
                literal_block(node, BracketTag::String)
                    .map(|ast| t.from_string(env, slot, ast, block))
            }
            (LiteralKind::String, Type::McNbtRef(t)) => literal_block(node, BracketTag::String)
                .map(|ast| t.from_string(env, slot, ast, block)),
            (LiteralKind::String, Type::McIdentifier(t)) => literal_block(node, BracketTag::String)
                .map(|ast| t.from_string(env, slot, ast, block)),
            (LiteralKind::String, Type::CExportName(t)) => literal_block(node, BracketTag::String)
                .map(|ast| t.from_string(env, slot, ast, block)),
            (LiteralKind::String, Type::KwString(_) | Type::KwText(_)) => {
                literal_block(node, BracketTag::String)
                    .map(|ast| crate::kw::interpolated_literal(env, &slot, ast, block))
            }
            (LiteralKind::List, Type::KwList(t)) => literal_block(node, BracketTag::List)
                .map(|ast| crate::kw::list_literal(env, t, ast, block)),
            (LiteralKind::String, Type::CtKey(_)) => {
                literal_block(node, BracketTag::String).map(|ast| string_key(env, ast, block))
            }
            (LiteralKind::String, Type::OperatorName(t)) => {
                literal_block(node, BracketTag::String).map(|ast| t.from_string(env, ast, block))
            }
            (LiteralKind::Map, Type::CtExportList(t)) => {
                literal_block(node, BracketTag::Map).map(|ast| t.from_map(env, slot, ast, block))
            }
            (LiteralKind::Map, Type::CtBuildArtifact(t)) => {
                literal_block(node, BracketTag::Map).map(|ast| t.from_map(env, slot, ast, block))
            }
            (LiteralKind::Map, Type::CtType(_)) => {
                literal_block(node, BracketTag::Map).map(|ast| {
                    Ok(crate::user_type::declare(
                        env,
                        ast,
                        crate::user_type::DeclKind::Type,
                    ))
                })
            }
            (LiteralKind::List, Type::Tuple(t)) => {
                literal_block(node, BracketTag::List).map(|ast| t.from_list(env, ast, block))
            }
            (LiteralKind::Number, Type::McResult(t)) => {
                literal_number(node).map(|ast| t.from_number(env, slot, ast, block))
            }
            (LiteralKind::Number, Type::CInt(t)) => {
                literal_number(node).map(|ast| t.from_number(env, ast))
            }
            (LiteralKind::Number, Type::KwInt(t)) => {
                literal_number(node).map(|ast| t.from_number(env, ast))
            }
            _ => unreachable!("literal hooks only exist for supported literals"),
        };
        result.unwrap_or_else(|| {
            Err(throw_err(
                env,
                Some(pos),
                format!("expected a {} literal", kind.name()),
                None,
                None,
            ))
        })
    }

    pub fn dump(&self) -> String {
        if let Type::User(t) = self {
            return t.name();
        }
        match self {
            Type::Void(_) => "TypeVoid",
            Type::Unknown(_) => "TypeUnknown",
            Type::Fn(_) => "TypeFn",
            Type::Uint8Array(_) => "TypeUint8Array",
            Type::Tuple(_) => "TypeTuple",
            Type::Optional(o) => return format!("?{}", o.child.dump()),
            Type::CtExportList(_) => "CtExportList",
            Type::CtKey(_) => "CtKey",
            Type::CtAst(_) => "CtAst",
            Type::CtNamespace(_) => "CtNamespace",
            Type::CtType(_) => "CtType",
            Type::CtBuildArtifact(_) => "CtBuildArtifact",
            Type::McResult(_) => "McResult",
            Type::McNbtRef(_) => "McNbtRef",
            Type::McIdentifier(_) => "McIdentifier",
            Type::CExportName(_) => "CExportName",
            Type::CInt(_) => "CInt",
            Type::CIf(_) => "CIf",
            Type::Label(_) => "Label",
            Type::OperatorName(_) => "OperatorName",
            Type::Bound(_) => "Bound",
            Type::KwInt(_) => "KwInt",
            Type::KwBool(_) => "KwBool",
            Type::KwIf(_) => "KwIf",
            Type::KwIfResult(_) => "KwIfResult",
            Type::User(_) => unreachable!("handled above"),
            Type::InlineFn(_) => "InlineFn",
            Type::KwMut(_) => "KwMut",
            Type::Never(_) => "Never",
            Type::Infer(_) => "TypeUnknown",
            Type::KwString(_) => "KwString",
            Type::KwText(_) => "KwText",
            Type::KwList(_) => "KwList",
            Type::KwField(_) => "KwField",
            Type::Null(_) => "Null",
            Type::KwIfOptional(_) => "KwIf",
            Type::BoundName(_) => "BoundName",
        }
        .to_string()
    }

    pub fn analyze_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let key = std_key(StdKey::Call);
        if !self.has_value_symbol(env, key)? {
            return Err(throw_err(
                env,
                Some(pos),
                format!("not supported call type: {}", self.dump()),
                None,
                None,
            ));
        }
        self.call_bound(env, slot, key, method, pos, arg_in, block)
    }

    #[allow(clippy::too_many_arguments)]
    fn builtin_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::Fn(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            Type::CtNamespace(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            Type::CtType(t) => t.analyze_call(env, slot, pos, method, arg_in, block),
            Type::CIf(t) => t.analyze_call(env, pos, method, arg_in, block),
            Type::KwIf(t) => t.analyze_call(env, pos, method, arg_in, block),
            Type::Label(t) => t.analyze_call(env, pos, arg_in, block),
            Type::KwField(t) => crate::kw::call_field(env, t, method.value, pos, arg_in, block),
            Type::KwIfOptional(t) => t.analyze_call(env, pos, method.value, arg_in, block),
            Type::BoundName(t) => {
                let Type::User(user) = &*t.receiver else {
                    unreachable!("only declared types have named methods")
                };
                let receiver = AnalysisResult {
                    ty: (*t.receiver).clone(),
                    value: method.value,
                };
                user.call_method_name(env, &t.name, receiver, arg_in, block)
            }
            Type::InlineFn(t) => {
                crate::user_type::inline_call(env, &t.func, Vec::new(), arg_in, block)
            }
            Type::Bound(t) => {
                let receiver = AnalysisResult {
                    ty: (*t.receiver).clone(),
                    value: method.value,
                };
                t.receiver
                    .call_bound(env, slot, t.key, receiver, pos, arg_in, block)
            }
            _ => unreachable!("has_value_symbol only accepts calls on callable types"),
        }
    }

    pub fn analyze_access(
        &self,
        env: &mut Env,
        slot: Type,
        obj: AnalysisResult,
        pos: TokenPosition,
        prop: AnalysisResult,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self {
            Type::CtNamespace(t) => t.analyze_access(env, slot, obj, pos, prop, block),
            Type::CtType(_) => {
                let key = access_key(env, block, prop, &pos)?;
                let ty = get_comptime(env, Some(ComptimeValueKind::Type), obj.value, pos.clone())?;
                let ComptimeValue::Type(ty) = ty else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let found = match &key {
                    ComptimeValueKey::Symbol { key, .. } => ty.ty.type_symbol(env, *key)?,
                    ComptimeValueKey::String { key } => match &ty.ty {
                        Type::User(user) => user.type_field(env, block, &pos, key)?,
                        other => other.type_field(key),
                    },
                };
                if found.is_none()
                    && matches!(&key, ComptimeValueKey::String { key } if key == "else")
                {
                    return Err(throw_err(
                        env,
                        Some(pos),
                        ".else must follow the } of a std.kw.if on the same line",
                        None,
                        None,
                    ));
                }
                found.ok_or_else(|| {
                    throw_err(
                        env,
                        Some(pos),
                        format!("{} has no {}", ty.ty.dump(), key_description(&key)),
                        None,
                        None,
                    )
                })
            }
            _ => {
                let key = access_key(env, block, prop, &pos)?;
                if let (ComptimeValueKey::String { key: name }, Type::User(user)) = (&key, self)
                    && let Some(field) = user.value_field(env, block, &obj, &pos, name)?
                {
                    return Ok(field);
                }
                if let ComptimeValueKey::String { key: name } = &key
                    && let Some(field) = crate::kw::value_field(env, block, self, &obj, &pos, name)?
                {
                    return Ok(field);
                }
                let has_value_symbol = match &key {
                    ComptimeValueKey::Symbol { key, .. } => self.has_value_symbol(env, *key)?,
                    ComptimeValueKey::String { .. } => false,
                };
                match key {
                    ComptimeValueKey::String { key }
                        if key == "*" && matches!(self, Type::KwMut(_)) =>
                    {
                        let Type::KwMut(cell) = self else {
                            unreachable!("matched KwMut above")
                        };
                        let idx = block_append(
                            block,
                            AnalysisLine::MutGet {
                                pos,
                                cell: obj.value,
                            },
                        );
                        Ok(AnalysisResult {
                            ty: (*cell.inner).clone(),
                            value: RuntimeValue::Runtime(idx),
                        })
                    }
                    ComptimeValueKey::String { key }
                        if key == "else" && matches!(self, Type::KwIfResult(_)) =>
                    {
                        let Type::KwIfResult(result) = self else {
                            unreachable!("matched KwIfResult above")
                        };
                        Ok(AnalysisResult {
                            ty: Type::KwIf(KwIf {
                                region: KwIfRegion::Else {
                                    then_never: result.body_never,
                                },
                            }),
                            value: obj.value,
                        })
                    }
                    ComptimeValueKey::Symbol { key, .. } if has_value_symbol => {
                        Ok(AnalysisResult {
                            ty: Type::Bound(TypeBound {
                                receiver: Box::new(self.clone()),
                                key,
                            }),
                            value: obj.value,
                        })
                    }
                    key => Err(throw_err(
                        env,
                        Some(pos),
                        format!("{} has no {}", self.dump(), key_description(&key)),
                        None,
                        None,
                    )),
                }
            }
        }
    }

    pub fn type_symbol(
        &self,
        env: &mut Env,
        key: Symbol,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        if let Type::User(t) = self {
            return Ok(t
                .static_symbol(env, key)?
                .map(crate::user_type::inline_entry));
        }
        if let (Type::Optional(optional), Some(StdKey::Literal(_))) = (self, symbol_std_key(key)) {
            return optional.child.type_symbol(env, key);
        }
        Ok(self.builtin_type_symbol(key))
    }

    fn builtin_type_symbol(&self, key: Symbol) -> Option<AnalysisResult> {
        if let Some(StdKey::Literal(kind)) = symbol_std_key(key) {
            return self.supports_literal(kind).then(|| AnalysisResult {
                ty: Type::CtNamespace(CtNamespace),
                value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(LiteralHook {
                    ty: self.clone(),
                    kind,
                    pos: compiler_pos(),
                }))),
            });
        }
        builtin_operator(self, OperatorKind::Slot, key).map(|op| AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(SlotOperator {
                ty: self.clone(),
                op,
                pos: compiler_pos(),
            }))),
        })
    }

    pub fn type_field(&self, name: &str) -> Option<AnalysisResult> {
        if let Some(field) = crate::kw::type_field(self, name) {
            return Some(field);
        }
        if let Type::Optional(optional) = self {
            return optional.child.type_field(name);
        }
        if let (Type::CtKey(_), Some(section)) = (self, crate::std_keys::Section::from_name(name)) {
            return Some(AnalysisResult {
                ty: Type::CtKey(CtKey),
                value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::Symbol {
                    key: std_key(StdKey::Section(section)),
                    child: Type::Unknown(TypeUnknown),
                })),
            });
        }
        match (self, name) {
            (Type::KwBool(_), "true" | "false") => Some(AnalysisResult {
                ty: Type::KwBool(KwBool),
                value: RuntimeValue::Comptime(ComptimeValue::KwBool(ComptimeValueKwBool {
                    value: name == "true",
                })),
            }),
            _ => None,
        }
    }

    pub fn has_value_symbol(&self, env: &mut Env, key: Symbol) -> Result<bool, PositionedError> {
        if let Type::User(t) = self {
            if t.method_symbol(env, key)?.is_some() {
                return Ok(true);
            }
            return Ok(t.is_enum(env)? && enum_comparison(key).is_some());
        }
        Ok(self.builtin_has_value_symbol(key))
    }

    fn builtin_has_value_symbol(&self, key: Symbol) -> bool {
        if is_mut_assign(self, key) {
            return true;
        }
        if symbol_std_key(key) == Some(StdKey::Call) {
            return matches!(
                self,
                Type::Fn(_)
                    | Type::CtNamespace(_)
                    | Type::CtType(_)
                    | Type::CIf(_)
                    | Type::KwIf(_)
                    | Type::Label(_)
                    | Type::Bound(_)
                    | Type::InlineFn(_)
                    | Type::KwField(_)
                    | Type::KwIfOptional(_)
                    | Type::BoundName(_)
            );
        }
        builtin_operator(self, OperatorKind::Lhs, key).is_some()
    }

    #[allow(clippy::too_many_arguments)]
    fn call_bound(
        &self,
        env: &mut Env,
        slot: Type,
        key: Symbol,
        receiver: AnalysisResult,
        pos: TokenPosition,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if let Type::User(t) = self {
            let Some(entry) = t.method_symbol(env, key)? else {
                let Some(op) = enum_comparison(key) else {
                    unreachable!("bound only to symbols has_value_symbol accepted")
                };
                let rhs = analyze(env, self.clone(), arg_in.pos, arg_in.ast, block)?;
                let rhs = self.cast_into(env, block, rhs, pos.clone())?;
                return crate::kw::emit(
                    env,
                    block,
                    pos,
                    op,
                    vec![receiver.value, rhs.value],
                    Type::KwBool(KwBool),
                );
            };
            let ComptimeValue::Fn(func) = get_comptime(env, None, entry.value, pos.clone())? else {
                return Err(throw_err(
                    env,
                    Some(pos),
                    format!(
                        "{}'s {} is not a function",
                        self.dump(),
                        key_description(&ComptimeValueKey::Symbol {
                            key,
                            child: Type::Unknown(TypeUnknown),
                        })
                    ),
                    None,
                    None,
                ));
            };
            return crate::user_type::inline_call(env, &func, vec![receiver], arg_in, block);
        }
        if symbol_std_key(key) == Some(StdKey::Call) {
            return self.builtin_call(env, slot, pos, receiver, arg_in, block);
        }
        if let (true, Type::KwMut(cell)) = (is_mut_assign(self, key), self) {
            let value = analyze(env, (*cell.inner).clone(), arg_in.pos, arg_in.ast, block)?;
            let value = cell.inner.cast_into(env, block, value, pos.clone())?;
            block_append(
                block,
                AnalysisLine::MutSet {
                    pos,
                    cell: receiver.value,
                    value: value.value,
                },
            );
            return Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            });
        }
        let Some(op) = builtin_operator(self, OperatorKind::Lhs, key) else {
            unreachable!("bound only to symbols has_value_symbol accepted")
        };
        let rhs = analyze(env, self.clone(), arg_in.pos, arg_in.ast, block)?;
        let rhs = self.cast_into(env, block, rhs, pos.clone())?;
        builtin_binary(env, block, self, pos, op, receiver.value, rhs.value)
    }

    pub fn implicit_arg_ret_for_arrow_fn(&self) -> ImplicitArgRet {
        ImplicitArgRet {
            arg: Type::Unknown(TypeUnknown),
            ret: Type::Unknown(TypeUnknown),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeVoid;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeUnknown;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeFn {
    pub pos: TokenPosition,
    pub arg: Box<Type>,
    pub ret: Box<Type>,
}

impl TypeFn {
    fn analyze_call(
        &self,
        env: &mut Env,
        _slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let arg = analyze(env, (*self.arg).clone(), arg_in.pos, arg_in.ast, block)?;
        let ret = match &*self.ret {
            Type::Unknown(_) => {
                let callee = get_comptime(
                    env,
                    Some(ComptimeValueKind::Fn),
                    method.value.clone(),
                    pos.clone(),
                )?;
                let ComptimeValue::Fn(callee) = callee else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                match crate::compiler::posted_return(&callee, &env.scope.comptime) {
                    Some(ty) => ty,
                    None if env.fn_cache.is_in_progress(&callee, &env.scope.comptime) => {
                        return Err(throw_err(
                            env,
                            Some(pos),
                            "this call needs the function's return type before it is known; start the function's body with its type, as in `T: ...`",
                            None,
                            None,
                        ));
                    }
                    None => analyze_function(env, &callee)?.ty,
                }
            }
            ret => ret.clone(),
        };
        let idx = block_append(
            block,
            AnalysisLine::Call {
                pos,
                method: method.value,
                arg: arg.value,
            },
        );
        Ok(AnalysisResult {
            ty: ret,
            value: RuntimeValue::Runtime(idx),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeUint8Array;

impl TypeUint8Array {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if ast.items.len() != 1 {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!(
                    "TODO str items len != 1 todo{}",
                    AST_NODE.dump_list(&ast.items, 3)
                ),
                None,
                Some(ErrorStyle::Todo),
            ));
        }
        let it0 = &ast.items[0];
        let is_raw_string = matches!(it0, SyntaxNode::Raw(raw) if raw.tag == RawTag::String);
        if !is_raw_string {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("TODO str item 0 ! raw string{}", AST_NODE.dump(it0, 3)),
                None,
                None,
            ));
        }
        let SyntaxNode::Raw(raw) = it0 else {
            unreachable!("just matched SyntaxNode::Raw above")
        };
        let unescaped = unescape_string(env, &raw.raw, raw.pos.clone())?;
        let sourcemap: Vec<Uint8ArraySourcemapEntry> = Vec::new();
        Ok(AnalysisResult {
            ty: Type::Uint8Array(TypeUint8Array),
            value: RuntimeValue::Comptime(ComptimeValue::Uint8Array(ComptimeValueUint8Array {
                value: unescaped.into_bytes(),
                sourcemap,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeTuple {
    pub children: Vec<Type>,
}

impl TypeTuple {
    fn from_list(
        &self,
        env: &mut Env,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let items = list_items(env, ast)?;
        if items.len() != self.children.len() {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!(
                    "expected {} items, found {}",
                    self.children.len(),
                    items.len()
                ),
                None,
                None,
            ));
        }
        let ty = Type::Tuple(self.clone());
        if items.is_empty() {
            return Ok(AnalysisResult {
                ty,
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            });
        }
        let mut values = Vec::new();
        for (item, child) in items.iter().zip(&self.children) {
            let value = analyze(env, child.clone(), item.pos.clone(), &item.items, block)?;
            values.push(child.cast_into(env, block, value, item.pos.clone())?.value);
        }
        let idx = block_append(
            block,
            AnalysisLine::Tuple {
                pos: ast.pos.clone(),
                items: values,
            },
        );
        Ok(AnalysisResult {
            ty,
            value: RuntimeValue::Runtime(idx),
        })
    }
}

pub fn list_items(
    env: &mut Env,
    ast: &BlockToken,
) -> Result<Vec<OperatorSegmentToken>, PositionedError> {
    Ok(read_binary(env, ast.pos.clone(), &ast.items, OpTag::Sep)?
        .into_iter()
        .filter(|item| !trim_ws(&item.items).is_empty())
        .collect())
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeOptional {
    pub child: Box<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtExportList {
    pub key: Box<Type>,
}

impl CtExportList {
    fn from_map(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let thiskey = (*self.key).clone();
        let mut exports_block = empty_block();
        let arr_entry_idx = block_append(
            &mut exports_block,
            AnalysisLine::ComptimeKvListInit {
                pos: ast.pos.clone(),
            },
        );
        let arr_entry = RuntimeValue::Runtime(arr_entry_idx);
        analyze_block(
            env,
            slot,
            ast.pos.clone(),
            &ast.items,
            &mut exports_block,
            |env, b2: Binary2, block| {
                let (lhs, op, rhs) = b2;
                let key = analyze(env, thiskey.clone(), lhs.pos.clone(), &lhs.items, block)?;
                let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
                let ret = block_append(
                    block,
                    AnalysisLine::ComptimeKvListAppend {
                        pos: op.pos.clone(),
                        list: arr_entry.clone(),
                        key: key.value,
                        value: value.value,
                    },
                );
                Ok(AnalysisResult {
                    ty: Type::Void(TypeVoid),
                    value: RuntimeValue::Runtime(ret),
                })
            },
        )?;

        let evaluated = comptime_eval(env, &exports_block, arr_entry, ast.pos.clone())?;
        let arr_value = get_comptime(
            env,
            Some(ComptimeValueKind::KvFields),
            RuntimeValue::Comptime(evaluated),
            ast.pos.clone(),
        )?;
        let ComptimeValue::KvFields(mut arr_value) = arr_value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        arr_value.locked = true;

        let mut exports = Vec::new();
        for entry in arr_value.entries {
            let value = get_comptime(
                env,
                Some(ComptimeValueKind::Ast),
                RuntimeValue::Comptime(entry.value),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Ast(value) = value else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            exports.push(ComptimeValueExportListEntry {
                key: entry.key,
                key_pos: entry.pos,
                value,
            });
        }

        Ok(AnalysisResult {
            ty: Type::CtExportList(CtExportList {
                key: self.key.clone(),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::ExportList(ComptimeValueExportList {
                exports,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtKey;

#[derive(Debug, Clone, PartialEq)]
pub struct CtAst;

#[derive(Debug, Clone, PartialEq)]
pub struct CtNamespace;

impl CtNamespace {
    fn analyze_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let value = get_comptime(
            env,
            Some(ComptimeValueKind::Namespace),
            method.value,
            pos.clone(),
        )?;
        let ComptimeValue::Namespace(ns) = value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        ns.analyze_call(env, slot, pos, arg_in, block)
    }

    fn analyze_access(
        &self,
        env: &mut Env,
        _slot: Type,
        obj: AnalysisResult,
        pos: TokenPosition,
        prop: AnalysisResult,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let RuntimeValue::Comptime(ComptimeValue::Namespace(ns)) = &obj.value else {
            return Err(throw_err(
                env,
                Some(pos),
                format!(
                    "cannot access on namespace type with value kind {:?}",
                    obj.value
                ),
                None,
                None,
            ));
        };
        let as_key = Type::CtKey(CtKey).cast_into(env, block, prop, pos.clone())?;
        let kval = get_comptime(env, Some(ComptimeValueKind::Key), as_key.value, pos.clone())?;
        let ComptimeValue::Key(key) = kval else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        match key {
            ComptimeValueKey::String { key } => ns.get_string(env, pos, &key, block),
            ComptimeValueKey::Symbol { key, child } => {
                let found = ns.get_symbol(env, pos.clone(), child.clone(), key, block)?;
                found.ok_or_else(|| {
                    throw_err(
                        env,
                        Some(pos),
                        format!(
                            "namespace has no {}",
                            key_description(&ComptimeValueKey::Symbol { key, child })
                        ),
                        Some(vec![(
                            Some(ns.pos().clone()),
                            "namespace defined here".to_string(),
                        )]),
                        None,
                    )
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtType;

impl CtType {
    fn analyze_call(
        &self,
        env: &mut Env,
        slot: Type,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let slot_type = get_comptime(
            env,
            Some(ComptimeValueKind::Type),
            method.value,
            pos.clone(),
        )?;
        let ComptimeValue::Type(slot_type) = slot_type else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        if let Type::Infer(infer) = &slot {
            crate::compiler::post_return(&infer.key, slot_type.ty.clone());
        }
        let result = analyze(env, slot_type.ty.clone(), arg_in.pos, arg_in.ast, block)?;
        if result.ty == slot_type.ty {
            return Ok(result);
        }
        slot_type.ty.cast_into(env, block, result, pos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtBuildArtifactNarrow {
    Folder,
    File,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CtBuildArtifact {
    pub narrow: Option<CtBuildArtifactNarrow>,
}

enum FolderRegisteredEntry {
    Ok {
        decl: ComptimeValueDeclaration,
        pos: TokenPosition,
    },
    Error {
        etok: ConsumedErrorToken,
        pos: TokenPosition,
    },
}

impl CtBuildArtifact {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let idx = block_append(
            block,
            AnalysisLine::ComptimeFileCreate {
                pos: ast.pos.clone(),
                value: str_result.value,
            },
        );
        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact {
                narrow: Some(CtBuildArtifactNarrow::File),
            }),
            value: RuntimeValue::Runtime(idx),
        })
    }

    fn from_map(
        &self,
        env: &mut Env,
        slot: Type,
        ast: &BlockToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let mut exports_block = empty_block();
        let arr_entry_idx = block_append(
            &mut exports_block,
            AnalysisLine::ComptimeKvListInit {
                pos: ast.pos.clone(),
            },
        );
        let arr_entry = RuntimeValue::Runtime(arr_entry_idx);
        analyze_block(
            env,
            slot,
            ast.pos.clone(),
            &ast.items,
            &mut exports_block,
            |env, b2: Binary2, block| {
                let (lhs, op, rhs) = b2;
                let key = analyze(
                    env,
                    Type::Uint8Array(TypeUint8Array),
                    lhs.pos.clone(),
                    &lhs.items,
                    block,
                )?;
                let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
                let ret = block_append(
                    block,
                    AnalysisLine::ComptimeKvListAppend {
                        pos: op.pos.clone(),
                        list: arr_entry.clone(),
                        key: key.value,
                        value: value.value,
                    },
                );
                Ok(AnalysisResult {
                    ty: Type::Void(TypeVoid),
                    value: RuntimeValue::Runtime(ret),
                })
            },
        )?;

        let evaluated = comptime_eval(env, &exports_block, arr_entry, ast.pos.clone())?;
        let arr_value = get_comptime(
            env,
            Some(ComptimeValueKind::KvFields),
            RuntimeValue::Comptime(evaluated),
            ast.pos.clone(),
        )?;
        let ComptimeValue::KvFields(mut arr_value) = arr_value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        arr_value.locked = true;

        let mut registered: Vec<(String, FolderRegisteredEntry)> = Vec::new();
        for entry in arr_value.entries {
            let raw_key = get_comptime(
                env,
                Some(ComptimeValueKind::Uint8Array),
                RuntimeValue::Comptime(entry.key),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Uint8Array(raw_key) = raw_key else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let value = get_comptime(
                env,
                Some(ComptimeValueKind::Ast),
                RuntimeValue::Comptime(entry.value),
                entry.pos.clone(),
            )?;
            let ComptimeValue::Ast(value) = value else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let key = String::from_utf8_lossy(&raw_key.value).into_owned();
            if let Some((_, prev)) = registered.iter_mut().find(|(k, _)| *k == key) {
                let prev_pos = match prev {
                    FolderRegisteredEntry::Ok { pos, .. } => pos.clone(),
                    FolderRegisteredEntry::Error { pos, .. } => pos.clone(),
                };
                let etok = add_err(
                    env,
                    Some(entry.pos.clone()),
                    "duplicate definition",
                    Some(vec![(
                        Some(prev_pos.clone()),
                        "previous definition here".to_string(),
                    )]),
                );
                *prev = FolderRegisteredEntry::Error {
                    etok,
                    pos: prev_pos,
                };
                continue;
            }
            registered.push((
                key,
                FolderRegisteredEntry::Ok {
                    decl: create_declaration(env, value),
                    pos: entry.pos.clone(),
                },
            ));
        }

        let mut result = Vec::new();
        for (key, entry) in registered {
            match entry {
                FolderRegisteredEntry::Ok { decl, pos } => {
                    let declared = get_declaration(env, decl)?;
                    let subitm = get_comptime(
                        env,
                        Some(ComptimeValueKind::BuildArtifact),
                        RuntimeValue::Comptime(declared.value),
                        pos,
                    )?;
                    let ComptimeValue::BuildArtifact(subitm) = subitm else {
                        unreachable!("get_comptime guarantees a matching kind")
                    };
                    result.push((key, subitm));
                }
                FolderRegisteredEntry::Error { etok, .. } => {
                    return Err(throw_consumed_err(etok));
                }
            }
        }

        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact {
                narrow: Some(CtBuildArtifactNarrow::Folder),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::BuildArtifact(
                ComptimeValueBuildArtifact::Folder(ComptimeFolder { value: result }),
            )),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McResultNarrow {
    I32,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McResult {
    pub narrow: Option<McResultNarrow>,
}

impl McResult {
    fn cast_into(
        &self,
        env: &mut Env,
        _block: &mut AnalysisBlock,
        other: AnalysisResult,
        pos: TokenPosition,
    ) -> Result<AnalysisResult, PositionedError> {
        let Type::McResult(other_narrow) = &other.ty else {
            return Err(throw_err(
                env,
                Some(pos),
                "no implicit cast available",
                None,
                None,
            ));
        };
        if self.narrow.is_some() && self.narrow != other_narrow.narrow {
            return Err(throw_err(env, Some(pos), "cannot widen", None, None));
        }
        Ok(other)
    }

    fn from_number(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &IdentifierToken,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let parsed = ast
            .str
            .parse::<i32>()
            .ok()
            .filter(|v| v.to_string() == ast.str);
        let Some(parsed) = parsed else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("invalid i32: got '{}'", ast.str),
                None,
                None,
            ));
        };
        Ok(AnalysisResult {
            ty: Type::McResult(McResult {
                narrow: Some(McResultNarrow::I32),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::McResult(ComptimeValueMcResult {
                result: parsed,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum McNbtRefNarrow {
    String,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    List(Box<McNbtRef>),
    Compound(HashMap<String, McNbtRef>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct McNbtRef {
    pub narrow: Option<McNbtRefNarrow>,
}

impl McNbtRef {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        Ok(AnalysisResult {
            ty: Type::McNbtRef(McNbtRef {
                narrow: Some(McNbtRefNarrow::String),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::McNbtRef(ComptimeValueMcNbtRef::String(
                decoded,
            ))),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct McIdentifier {
    pub category: Option<String>,
}

impl McIdentifier {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        let Some((namespace, path)) = parse_mc_identifier(&decoded) else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid minecraft identifier name",
                None,
                None,
            ));
        };
        if namespace == ".." {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid minecraft identifier name",
                None,
                None,
            ));
        }
        Ok(AnalysisResult {
            ty: Type::McIdentifier(McIdentifier { category: None }),
            value: RuntimeValue::Comptime(ComptimeValue::McIdentifier(ComptimeValueMcIdentifier {
                namespace,
                path,
            })),
        })
    }
}

fn parse_mc_identifier(decoded: &str) -> Option<(String, String)> {
    fn is_id_char(c: char) -> bool {
        matches!(c, '-' | '.' | '_' | '0'..='9' | 'a'..='z')
    }
    let (namespace_part, path_part) = match decoded.split_once(':') {
        Some((ns, path)) => (Some(ns), path),
        None => (None, decoded),
    };
    if let Some(ns) = namespace_part
        && (ns.is_empty() || !ns.chars().all(is_id_char))
    {
        return None;
    }
    if path_part.is_empty() || !path_part.chars().all(|c| is_id_char(c) || c == '/') {
        return None;
    }
    Some((
        namespace_part.unwrap_or("minecraft").to_string(),
        path_part.to_string(),
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct CExportName;

impl CExportName {
    fn from_string(
        &self,
        env: &mut Env,
        _slot: Type,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        if !validate_c_name(&decoded) {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                "invalid c identifier name",
                None,
                None,
            ));
        }
        let Some(validated) = CValidatedIdentifierName::new(decoded) else {
            unreachable!("validate_c_name already confirmed this name is valid")
        };
        Ok(AnalysisResult {
            ty: Type::CExportName(CExportName),
            value: RuntimeValue::Comptime(ComptimeValue::CExportName(ComptimeValueCExportName {
                value: validated,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CInt;

impl CInt {
    fn from_number(
        &self,
        env: &mut Env,
        ast: &IdentifierToken,
    ) -> Result<AnalysisResult, PositionedError> {
        let parsed = ast
            .str
            .parse::<i32>()
            .ok()
            .filter(|v| v.to_string() == ast.str);
        let Some(value) = parsed else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("invalid c int: got '{}'", ast.str),
                None,
                None,
            ));
        };
        Ok(AnalysisResult {
            ty: Type::CInt(CInt),
            value: RuntimeValue::Comptime(ComptimeValue::CInt(ComptimeValueCInt { value })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CIf;

impl CIf {
    fn analyze_call(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        block_append(
            block,
            AnalysisLine::RegionBegin {
                pos: pos.clone(),
                region: Region::CIf { cond: method.value },
            },
        );
        analyze(env, Type::Void(TypeVoid), arg_in.pos, arg_in.ast, block)?;
        block_append(block, AnalysisLine::RegionEnd { pos });
        Ok(AnalysisResult {
            ty: Type::Void(TypeVoid),
            value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwInt;

impl KwInt {
    fn from_number(
        &self,
        env: &mut Env,
        ast: &IdentifierToken,
    ) -> Result<AnalysisResult, PositionedError> {
        let parsed = ast
            .str
            .parse::<i64>()
            .ok()
            .filter(|v| v.to_string() == ast.str);
        let Some(value) = parsed else {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("invalid std.kw.int: got '{}'", ast.str),
                None,
                None,
            ));
        };
        Ok(AnalysisResult {
            ty: Type::KwInt(KwInt),
            value: RuntimeValue::Comptime(ComptimeValue::KwInt(ComptimeValueKwInt { value })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwBool;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KwIfRegion {
    If,
    Else { then_never: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwIf {
    pub region: KwIfRegion,
}

impl KwIf {
    fn analyze_call(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        method: AnalysisResult,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let region = match self.region {
            KwIfRegion::If => Region::KwIf { cond: method.value },
            KwIfRegion::Else { .. } => {
                let RuntimeValue::Runtime(if_end) = method.value else {
                    unreachable!(".else is only reachable from an if's result")
                };
                Region::KwElse { if_end }
            }
        };
        block_append(
            block,
            AnalysisLine::RegionBegin {
                pos: pos.clone(),
                region,
            },
        );
        let body = analyze(env, Type::Void(TypeVoid), arg_in.pos, arg_in.ast, block)?;
        let body_never = matches!(body.ty, Type::Never(_));
        let end = block_append(block, AnalysisLine::RegionEnd { pos });
        Ok(match self.region {
            KwIfRegion::If => AnalysisResult {
                ty: Type::KwIfResult(KwIfResult { body_never }),
                value: RuntimeValue::Runtime(end),
            },
            KwIfRegion::Else { then_never } => AnalysisResult {
                ty: if then_never && body_never {
                    Type::Never(TypeNever)
                } else {
                    Type::Void(TypeVoid)
                },
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KwIfResult {
    pub body_never: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeLabel {
    pub label: Symbol,
    pub ty: Box<Type>,
}

impl TypeLabel {
    fn analyze_call(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        arg_in: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let value = analyze(env, (*self.ty).clone(), arg_in.pos, arg_in.ast, block)?;
        let value = self.ty.cast_into(env, block, value, pos.clone())?;
        block_append(
            block,
            AnalysisLine::Break {
                pos,
                label: self.label,
                value: value.value,
            },
        );
        Ok(AnalysisResult {
            ty: Type::Never(TypeNever),
            value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        })
    }
}

fn access_key(
    env: &mut Env,
    block: &mut AnalysisBlock,
    prop: AnalysisResult,
    pos: &TokenPosition,
) -> Result<ComptimeValueKey, PositionedError> {
    let as_key = Type::CtKey(CtKey).cast_into(env, block, prop, pos.clone())?;
    let kval = get_comptime(env, Some(ComptimeValueKind::Key), as_key.value, pos.clone())?;
    let ComptimeValue::Key(key) = kval else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(key)
}

fn key_description(key: &ComptimeValueKey) -> String {
    match key {
        ComptimeValueKey::String { key } => format!("field '{key}'"),
        ComptimeValueKey::Symbol { key, .. } => symbol_std_key(*key)
            .map(|key| key.describe())
            .unwrap_or_else(|| "such symbol".to_string()),
    }
}

fn literal_block(node: &SyntaxNode, tag: BracketTag) -> Option<&BlockToken> {
    match node {
        SyntaxNode::Block(b) if b.tag == tag => Some(b),
        _ => None,
    }
}

fn literal_number(node: &SyntaxNode) -> Option<&IdentifierToken> {
    match node {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Number => Some(id),
        _ => None,
    }
}

pub fn analyze_literal(
    env: &mut Env,
    slot: Type,
    kind: LiteralKind,
    pos: TokenPosition,
    node: &SyntaxNode,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let Some(hook) = slot.type_symbol(env, std_key(StdKey::Literal(kind)))? {
        return analyze_call(
            env,
            slot,
            pos.clone(),
            hook,
            CallArg {
                pos,
                ast: std::slice::from_ref(node),
            },
            block,
        );
    }
    if let (LiteralKind::Map, SyntaxNode::Block(ast), Type::User(user)) = (kind, node, &slot)
        && user.has_fields(env)?
    {
        return user.struct_literal(env, ast, block);
    }
    if let (LiteralKind::List, SyntaxNode::Block(ast)) = (kind, node) {
        let items = list_items(env, ast)?;
        if let [item] = items.as_slice() {
            return analyze(env, slot, item.pos.clone(), &item.items, block);
        }
    }
    let title = match kind {
        LiteralKind::String => "String",
        LiteralKind::Number => "Number",
        LiteralKind::List => "List",
        LiteralKind::Map => "Map",
    };
    Err(throw_err(
        env,
        Some(pos),
        format!("{title} is not supported in slot: {}", slot.dump()),
        None,
        None,
    ))
}

#[derive(Debug)]
struct LiteralHook {
    ty: Type,
    kind: LiteralKind,
    pos: TokenPosition,
}

impl ComptimeNamespace for LiteralHook {
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
            format!("std.literal.{} has no field: {field}", self.kind.name()),
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
        let items = trim_ws(arg.ast);
        let [node] = items.as_slice() else {
            return Err(throw_err(
                env,
                Some(arg.pos),
                format!("expected a {} literal", self.kind.name()),
                None,
                None,
            ));
        };
        self.ty.analyze_literal(env, self.kind, pos, node, block)
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorName;

impl OperatorName {
    fn from_string(
        &self,
        env: &mut Env,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let str_result = analyze_base(
            env,
            Type::Uint8Array(TypeUint8Array),
            &SyntaxNode::Block(Box::new(ast.clone())),
            block,
        )?;
        let u8a = get_comptime(
            env,
            Some(ComptimeValueKind::Uint8Array),
            str_result.value,
            ast.pos.clone(),
        )?;
        let ComptimeValue::Uint8Array(u8a) = u8a else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let decoded = String::from_utf8_lossy(&u8a.value).into_owned();
        if !is_overloadable_operator(&decoded) {
            return Err(throw_err(
                env,
                Some(ast.pos.clone()),
                format!("not an operator: \"{decoded}\""),
                None,
                None,
            ));
        }
        Ok(AnalysisResult {
            ty: Type::OperatorName(OperatorName),
            value: RuntimeValue::Comptime(ComptimeValue::OperatorName(ComptimeValueOperatorName {
                value: decoded,
            })),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeBound {
    pub receiver: Box<Type>,
    pub key: Symbol,
}

fn builtin_operator(ty: &Type, kind: OperatorKind, key: Symbol) -> Option<CBinaryOp> {
    let (key_kind, op) = symbol_operator(key)?;
    if key_kind != kind {
        return None;
    }
    let op = CBinaryOp::from_token(&op)?;
    match (ty, kind, op.is_comparison()) {
        (Type::KwInt(_), OperatorKind::Slot, false) => Some(op),
        (Type::KwInt(_), OperatorKind::Lhs, _) => Some(op),
        (Type::KwString(_) | Type::KwText(_), OperatorKind::Slot, false)
            if op == CBinaryOp::Add =>
        {
            Some(op)
        }
        (Type::KwText(_), OperatorKind::Lhs, _) if op == CBinaryOp::Add => Some(op),
        (Type::KwString(_), OperatorKind::Lhs, _)
            if matches!(op, CBinaryOp::Add | CBinaryOp::Eq | CBinaryOp::Ne) =>
        {
            Some(op)
        }
        (Type::KwBool(_), OperatorKind::Lhs, _) if matches!(op, CBinaryOp::Eq | CBinaryOp::Ne) => {
            Some(op)
        }
        _ => None,
    }
}

fn builtin_binary(
    env: &mut Env,
    block: &mut AnalysisBlock,
    ty: &Type,
    pos: TokenPosition,
    op: CBinaryOp,
    lhs: RuntimeValue,
    rhs: RuntimeValue,
) -> Result<AnalysisResult, PositionedError> {
    if let Type::CInt(_) = ty {
        let idx = block_append(block, AnalysisLine::CBinary { pos, op, lhs, rhs });
        return Ok(AnalysisResult {
            ty: Type::CInt(CInt),
            value: RuntimeValue::Runtime(idx),
        });
    }
    if let Type::KwText(_) = ty {
        return crate::kw::emit(
            env,
            block,
            pos,
            crate::kw::KwBuiltinOp::TextParts,
            vec![lhs, rhs],
            Type::KwText(KwText),
        );
    }
    let ty = if op.is_comparison() {
        Type::KwBool(KwBool)
    } else {
        ty.clone()
    };
    if let (RuntimeValue::Comptime(l), RuntimeValue::Comptime(r)) = (&lhs, &rhs) {
        let value = fold_kw_binary(env, &pos, op, l, r)?;
        return Ok(AnalysisResult {
            ty,
            value: RuntimeValue::Comptime(value),
        });
    }
    let idx = block_append(block, AnalysisLine::KwBinary { pos, op, lhs, rhs });
    Ok(AnalysisResult {
        ty,
        value: RuntimeValue::Runtime(idx),
    })
}

pub fn fold_kw_binary(
    env: &mut Env,
    pos: &TokenPosition,
    op: CBinaryOp,
    lhs: &ComptimeValue,
    rhs: &ComptimeValue,
) -> Result<ComptimeValue, PositionedError> {
    let int = |value| Ok(ComptimeValue::KwInt(ComptimeValueKwInt { value }));
    let bool = |value| Ok(ComptimeValue::KwBool(ComptimeValueKwBool { value }));
    match (lhs, rhs) {
        (ComptimeValue::KwInt(a), ComptimeValue::KwInt(b)) => {
            let (a, b) = (a.value, b.value);
            let arithmetic = match op {
                CBinaryOp::Eq => return bool(a == b),
                CBinaryOp::Ne => return bool(a != b),
                CBinaryOp::Lt => return bool(a < b),
                CBinaryOp::Le => return bool(a <= b),
                CBinaryOp::Gt => return bool(a > b),
                CBinaryOp::Ge => return bool(a >= b),
                CBinaryOp::Add => a.checked_add(b),
                CBinaryOp::Sub => a.checked_sub(b),
                CBinaryOp::Mul => a.checked_mul(b),
                CBinaryOp::Div => a.checked_div(b),
                CBinaryOp::Rem => a.checked_rem(b),
            };
            match arithmetic {
                Some(value) => int(value),
                None => Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!("{a} {} {b} overflows or divides by zero", op.as_str()),
                    None,
                    None,
                )),
            }
        }
        (ComptimeValue::KwString(a), ComptimeValue::KwString(b)) => match op {
            CBinaryOp::Add => Ok(ComptimeValue::KwString(a.concat(b))),
            CBinaryOp::Eq => bool(a == b),
            CBinaryOp::Ne => bool(a != b),
            _ => unreachable!("std.kw.string only has +, == and !="),
        },
        (ComptimeValue::KwBool(a), ComptimeValue::KwBool(b)) => match op {
            CBinaryOp::Eq => bool(a.value == b.value),
            CBinaryOp::Ne => bool(a.value != b.value),
            _ => unreachable!("std.kw.bool only has == and !="),
        },
        _ => unreachable!("std.kw operators only take std.kw values"),
    }
}

#[derive(Debug)]
struct SlotOperator {
    ty: Type,
    op: CBinaryOp,
    pos: TokenPosition,
}

impl ComptimeNamespace for SlotOperator {
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
            format!("operator {} has no field: {field}", self.op.as_str()),
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
        let args = Type::Tuple(TypeTuple {
            children: vec![self.ty.clone(), self.ty.clone()],
        });
        let args = analyze(env, args, arg.pos, arg.ast, block)?;
        let RuntimeValue::Runtime(tuple) = &args.value else {
            unreachable!("a two item tuple is always a tuple line")
        };
        let AnalysisLine::Tuple { items, .. } = &block.lines[tuple.0] else {
            unreachable!("a two item tuple is always a tuple line")
        };
        let [lhs, rhs] = items.as_slice() else {
            unreachable!("the tuple type has two items")
        };
        let (lhs, rhs) = (lhs.clone(), rhs.clone());
        builtin_binary(env, block, &self.ty, pos, self.op, lhs, rhs)
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

pub fn slot_operator_value(ty: Type, op: CBinaryOp) -> AnalysisResult {
    AnalysisResult {
        ty: Type::CtNamespace(CtNamespace),
        value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(SlotOperator {
            ty,
            op,
            pos: compiler_pos(),
        }))),
    }
}

pub fn call_list_items(
    env: &mut Env,
    arg: &CallArg,
) -> Result<Option<Vec<OperatorSegmentToken>>, PositionedError> {
    let items = trim_ws(arg.ast);
    match items.as_slice() {
        [SyntaxNode::Block(list)] if list.tag == BracketTag::List => {
            Ok(Some(list_items(env, list)?))
        }
        _ => Ok(None),
    }
}

fn string_key(
    env: &mut Env,
    ast: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let str_result = analyze_base(
        env,
        Type::Uint8Array(TypeUint8Array),
        &SyntaxNode::Block(Box::new(ast.clone())),
        block,
    )?;
    let ComptimeValue::Uint8Array(u8a) = get_comptime(
        env,
        Some(ComptimeValueKind::Uint8Array),
        str_result.value,
        ast.pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::CtKey(CtKey),
        value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
            key: String::from_utf8_lossy(&u8a.value).into_owned(),
        })),
    })
}
