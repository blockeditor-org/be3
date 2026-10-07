use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::{
    AnalysisBlock, AnalysisResult, Binding, ComptimeNamespace, ComptimeScopeMap, ComptimeValue,
    ComptimeValueAst, ComptimeValueDeclaration, ComptimeValueFn, ComptimeValueKey,
    ComptimeValueKwInt, ComptimeValueType, DestructureExtract, Env, PositionedError, RuntimeValue,
    Scope, Symbol, TargetEnv, analyze, analyze_namespace, create_declaration, empty_block,
    get_declaration, read_binary, read_binary2, target_env_symbol, throw_err, trim_ws, with_scope,
};
use crate::comptime::{ComptimeValueKind, get_comptime};
use crate::ct::{
    CallArg, CtKey, CtType, Type, TypeBoundName, TypeInlineFn, TypeUnknown, TypeVoid,
    call_list_items,
};
use crate::kw::KwBuiltinOp;
use crate::parser::{
    BlockToken, BracketTag, IdentifierTag, OpTag, OperatorSegmentToken, SyntaxNode, TokenPosition,
    tokenize,
};
use crate::std_keys::{Section, StdKey, std_key, symbol_std_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Type,
    Struct,
    Enum,
}

pub struct FieldDecl {
    pub name: String,
    pub pos: TokenPosition,
    ty: Option<ComptimeValueDeclaration>,
}

pub struct Sections {
    pub fields: Option<Vec<FieldDecl>>,
    pub cases: Option<Vec<FieldDecl>>,
    statics: Rc<dyn ComptimeNamespace>,
    methods: Rc<dyn ComptimeNamespace>,
}

fn map_lines(
    env: &mut Env,
    pos: TokenPosition,
    items: &[SyntaxNode],
) -> Result<Vec<(OperatorSegmentToken, Option<OperatorSegmentToken>)>, PositionedError> {
    let mut lines = Vec::new();
    for line in read_binary(env, pos, items, OpTag::Sep)? {
        if trim_ws(&line.items).is_empty() {
            continue;
        }
        match read_binary2(env, &line.items, OpTag::Pub)? {
            Some((lhs, _, rhs)) => lines.push((lhs, Some(rhs))),
            None => lines.push((line, None)),
        }
    }
    Ok(lines)
}

fn key_of(env: &mut Env, seg: &OperatorSegmentToken) -> Result<ComptimeValueKey, PositionedError> {
    let mut block = empty_block();
    let key = analyze(
        env,
        Type::CtKey(CtKey),
        seg.pos.clone(),
        &seg.items,
        &mut block,
    )?;
    let ComptimeValue::Key(key) = get_comptime(
        env,
        Some(ComptimeValueKind::Key),
        key.value,
        seg.pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(key)
}

fn named_entries(
    env: &mut Env,
    pos: TokenPosition,
    items: &[SyntaxNode],
    what: &str,
    payload_optional: bool,
) -> Result<Vec<FieldDecl>, PositionedError> {
    let mut entries: Vec<FieldDecl> = Vec::new();
    for (lhs, rhs) in map_lines(env, pos, items)? {
        let ComptimeValueKey::String { key: name } = key_of(env, &lhs)? else {
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                format!("{what} names must be strings"),
                None,
                None,
            ));
        };
        if rhs.is_none() && !payload_optional {
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                format!("field \"{name}\" needs a type, as in \"{name}\" .= T"),
                None,
                None,
            ));
        }
        if let Some(previous) = entries.iter().find(|e| e.name == name) {
            let previous = previous.pos.clone();
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                format!("duplicate {what} \"{name}\""),
                Some(vec![(
                    Some(previous),
                    "previous definition here".to_string(),
                )]),
                None,
            ));
        }
        let ty = rhs.map(|rhs| {
            let scope = env.scope.clone();
            create_declaration(
                env,
                ComptimeValueAst {
                    ast: rhs.items.clone(),
                    pos: rhs.pos.clone(),
                    scope,
                },
            )
        });
        entries.push(FieldDecl {
            name,
            pos: lhs.pos.clone(),
            ty,
        });
    }
    Ok(entries)
}

impl FieldDecl {
    pub fn ty(&self, env: &mut Env) -> Result<Type, PositionedError> {
        let Some(decl) = &self.ty else {
            return Ok(Type::Void(TypeVoid));
        };
        let value = get_declaration(env, decl.clone())?;
        let ComptimeValue::Type(ty) = get_comptime(
            env,
            Some(ComptimeValueKind::Type),
            RuntimeValue::Comptime(value.value),
            self.pos.clone(),
        )?
        else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        Ok(ty.ty)
    }
}

fn build_sections(
    env: &mut Env,
    kind: DeclKind,
    pos: TokenPosition,
    items: &[SyntaxNode],
) -> Result<Sections, PositionedError> {
    let mut sections = Sections {
        fields: None,
        cases: None,
        statics: analyze_namespace(env, pos.clone(), &[])?,
        methods: analyze_namespace(env, pos.clone(), &[])?,
    };
    match kind {
        DeclKind::Struct => {
            sections.fields = Some(named_entries(env, pos, items, "field", false)?);
            return Ok(sections);
        }
        DeclKind::Enum => {
            sections.cases = Some(named_entries(env, pos, items, "case", true)?);
            return Ok(sections);
        }
        DeclKind::Type => {}
    }
    for (lhs, rhs) in map_lines(env, pos.clone(), items)? {
        let unknown_section = matches!(
            trim_ws(&lhs.items).as_slice(),
            [SyntaxNode::Identifier(id)]
                if id.ident_tag == IdentifierTag::Access && Section::from_name(&id.str).is_none()
        );
        if unknown_section {
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                "std.Type entries must be .fields, .cases, .statics or .methods",
                None,
                None,
            ));
        }
        let section = match key_of(env, &lhs)? {
            ComptimeValueKey::Symbol { key, .. } => match symbol_std_key(key) {
                Some(StdKey::Section(section)) => Some(section),
                _ => None,
            },
            ComptimeValueKey::String { .. } => None,
        };
        let (Some(section), Some(rhs)) = (section, rhs) else {
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                "std.Type entries must be .fields, .cases, .statics or .methods",
                None,
                None,
            ));
        };
        let items = trim_ws(&rhs.items);
        let [SyntaxNode::Block(map)] = items.as_slice() else {
            return Err(throw_err(
                env,
                Some(rhs.pos.clone()),
                "a std.Type section must be a [ ... ] map",
                None,
                None,
            ));
        };
        let map = map.clone();
        if map.tag != BracketTag::Map {
            return Err(throw_err(
                env,
                Some(rhs.pos.clone()),
                "a std.Type section must be a [ ... ] map",
                None,
                None,
            ));
        }
        match section {
            Section::Fields => {
                sections.fields = Some(named_entries(
                    env,
                    map.pos.clone(),
                    &map.items,
                    "field",
                    false,
                )?)
            }
            Section::Cases => {
                sections.cases = Some(named_entries(
                    env,
                    map.pos.clone(),
                    &map.items,
                    "case",
                    true,
                )?)
            }
            Section::Statics => {
                sections.statics = analyze_namespace(env, map.pos.clone(), &map.items)?
            }
            Section::Methods => {
                sections.methods = analyze_namespace(env, map.pos.clone(), &map.items)?
            }
        }
    }
    if sections.fields.is_some() && sections.cases.is_some() {
        return Err(throw_err(
            env,
            Some(pos),
            "a std.Type can't have both .fields and .cases",
            None,
            None,
        ));
    }
    Ok(sections)
}

pub struct UserTypeInner {
    pos: TokenPosition,
    kind: DeclKind,
    ast: ComptimeValueAst,
    name: OnceCell<String>,
    sections: RefCell<Option<Rc<Sections>>>,
    building: Cell<bool>,
}

#[derive(Clone)]
pub struct UserType(Rc<UserTypeInner>);

impl PartialEq for UserType {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserType({})", self.name())
    }
}

fn kw_int(value: usize) -> RuntimeValue {
    RuntimeValue::Comptime(ComptimeValue::KwInt(ComptimeValueKwInt {
        value: value as i64,
    }))
}

impl UserType {
    pub fn name(&self) -> String {
        match self.0.name.get() {
            Some(name) => name.clone(),
            None => format!("std.Type@{}:{}", self.0.pos.lyn, self.0.pos.col),
        }
    }

    pub fn set_name(&self, name: &str) {
        let _ = self.0.name.set(name.to_string());
    }

    fn sections(&self, env: &mut Env) -> Result<Rc<Sections>, PositionedError> {
        if let Some(sections) = self.0.sections.borrow().as_ref() {
            return Ok(sections.clone());
        }
        if self.0.building.replace(true) {
            return Err(throw_err(
                env,
                Some(self.0.pos.clone()),
                format!("dependency loop while reading the keys of {}", self.name()),
                None,
                None,
            ));
        }
        let pos = self.0.pos.clone();
        let ast = self.0.ast.ast.clone();
        let kind = self.0.kind;
        let result = with_scope(env, self.0.ast.scope.clone(), |env| {
            build_sections(env, kind, pos, &ast)
        });
        self.0.building.set(false);
        let sections = Rc::new(result?);
        *self.0.sections.borrow_mut() = Some(sections.clone());
        Ok(sections)
    }

    pub fn static_symbol(
        &self,
        env: &mut Env,
        key: Symbol,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let sections = self.sections(env)?;
        sections.statics.get_symbol(
            env,
            self.0.pos.clone(),
            Type::Unknown(TypeUnknown),
            key,
            &mut empty_block(),
        )
    }

    pub fn method_symbol(
        &self,
        env: &mut Env,
        key: Symbol,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let sections = self.sections(env)?;
        sections.methods.get_symbol(
            env,
            self.0.pos.clone(),
            Type::Unknown(TypeUnknown),
            key,
            &mut empty_block(),
        )
    }

    pub fn case(
        &self,
        env: &mut Env,
        name: &str,
    ) -> Result<Option<(usize, bool)>, PositionedError> {
        let sections = self.sections(env)?;
        Ok(sections.cases.as_ref().and_then(|cases| {
            cases
                .iter()
                .position(|case| case.name == name)
                .map(|index| (index, cases[index].ty.is_some()))
        }))
    }

    pub fn case_payload(&self, env: &mut Env, index: usize) -> Result<Type, PositionedError> {
        let sections = self.sections(env)?;
        let cases = sections.cases.as_ref().expect("case found the index");
        cases[index].ty(env)
    }

    pub fn field_names(&self, env: &mut Env) -> Result<Vec<String>, PositionedError> {
        let sections = self.sections(env)?;
        Ok(sections
            .fields
            .iter()
            .flatten()
            .map(|field| field.name.clone())
            .collect())
    }

    pub fn has_fields(&self, env: &mut Env) -> Result<bool, PositionedError> {
        Ok(self.sections(env)?.fields.is_some())
    }

    pub fn is_enum(&self, env: &mut Env) -> Result<bool, PositionedError> {
        Ok(self.sections(env)?.cases.is_some())
    }

    pub fn repr(&self, env: &mut Env, pos: &TokenPosition) -> Result<Type, PositionedError> {
        let Some(repr) = self.static_symbol(env, std_key(StdKey::Repr))? else {
            return Err(throw_err(
                env,
                Some(pos.clone()),
                format!("{} has no std.type.repr", self.name()),
                None,
                None,
            ));
        };
        let ComptimeValue::Type(repr) =
            get_comptime(env, Some(ComptimeValueKind::Type), repr.value, pos.clone())?
        else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        Ok(repr.ty)
    }

    pub fn type_field(
        &self,
        env: &mut Env,
        block: &mut AnalysisBlock,
        pos: &TokenPosition,
        name: &str,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let sections = self.sections(env)?;
        if let Some(cases) = &sections.cases
            && let Some(index) = cases.iter().position(|case| case.name == name)
        {
            if cases[index].ty.is_some() {
                return Ok(Some(AnalysisResult {
                    ty: Type::CtNamespace(crate::ct::CtNamespace),
                    value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(
                        EnumConstructor {
                            ty: self.clone(),
                            case: index,
                            pos: cases[index].pos.clone(),
                        },
                    ))),
                }));
            }
            return crate::kw::emit(
                env,
                block,
                pos.clone(),
                KwBuiltinOp::EnumNew,
                vec![kw_int(index)],
                Type::User(self.clone()),
            )
            .map(Some);
        }
        Ok(sections
            .statics
            .try_get_string(env, pos.clone(), name, block)?
            .map(inline_entry))
    }

    pub fn value_field(
        &self,
        env: &mut Env,
        block: &mut AnalysisBlock,
        obj: &AnalysisResult,
        pos: &TokenPosition,
        name: &str,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let sections = self.sections(env)?;
        if let Some(fields) = &sections.fields
            && let Some(index) = fields.iter().position(|field| field.name == name)
        {
            let ty = fields[index].ty(env)?;
            return crate::kw::emit(
                env,
                block,
                pos.clone(),
                KwBuiltinOp::StructGet,
                vec![obj.value.clone(), kw_int(index)],
                ty,
            )
            .map(Some);
        }
        if let Some(cases) = &sections.cases
            && let Some(index) = cases.iter().position(|case| case.name == name)
        {
            let payload = cases[index].ty(env)?;
            return crate::kw::emit(
                env,
                block,
                pos.clone(),
                KwBuiltinOp::EnumGet,
                vec![obj.value.clone(), kw_int(index)],
                Type::Optional(crate::ct::TypeOptional {
                    child: Box::new(payload),
                }),
            )
            .map(Some);
        }
        if sections
            .methods
            .try_get_string(env, pos.clone(), name, block)?
            .is_some()
        {
            return Ok(Some(AnalysisResult {
                ty: Type::BoundName(TypeBoundName {
                    receiver: Box::new(Type::User(self.clone())),
                    name: name.to_string(),
                }),
                value: obj.value.clone(),
            }));
        }
        Ok(None)
    }

    pub fn call_method_name(
        &self,
        env: &mut Env,
        name: &str,
        receiver: AnalysisResult,
        arg: CallArg,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let pos = arg.pos.clone();
        let sections = self.sections(env)?;
        let Some(method) = sections
            .methods
            .try_get_string(env, pos.clone(), name, block)?
        else {
            unreachable!("bound only to methods value_field found")
        };
        let ComptimeValue::Fn(func) = get_comptime(env, None, method.value, pos.clone())? else {
            return Err(throw_err(
                env,
                Some(pos),
                format!("{}'s method {name} is not a function", self.name()),
                None,
                None,
            ));
        };
        inline_call(env, &func, vec![receiver], arg, block)
    }

    pub fn struct_literal(
        &self,
        env: &mut Env,
        ast: &BlockToken,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let sections = self.sections(env)?;
        let fields = sections.fields.as_ref().expect("checked has_fields before");
        let mut values: Vec<Option<RuntimeValue>> = vec![None; fields.len()];
        for (lhs, rhs) in map_lines(env, ast.pos.clone(), &ast.items)? {
            let key = key_of(env, &lhs)?;
            let (ComptimeValueKey::String { key: name }, Some(rhs)) = (key, rhs) else {
                return Err(throw_err(
                    env,
                    Some(lhs.pos.clone()),
                    format!("expected \"field\" .= value in a {} literal", self.name()),
                    None,
                    None,
                ));
            };
            let Some(index) = fields.iter().position(|field| field.name == name) else {
                return Err(throw_err(
                    env,
                    Some(lhs.pos.clone()),
                    format!("{} has no field \"{name}\"", self.name()),
                    None,
                    None,
                ));
            };
            let ty = fields[index].ty(env)?;
            let value = analyze(env, ty.clone(), rhs.pos.clone(), &rhs.items, block)?;
            values[index] = Some(ty.cast_into(env, block, value, rhs.pos.clone())?.value);
        }
        let mut args = Vec::new();
        for (field, value) in fields.iter().zip(values) {
            let Some(value) = value else {
                return Err(throw_err(
                    env,
                    Some(ast.pos.clone()),
                    format!("missing field \"{}\" for {}", field.name, self.name()),
                    None,
                    None,
                ));
            };
            args.push(value);
        }
        crate::kw::emit(
            env,
            block,
            ast.pos.clone(),
            KwBuiltinOp::StructNew,
            args,
            Type::User(self.clone()),
        )
    }
}

#[derive(Debug)]
struct EnumConstructor {
    ty: UserType,
    case: usize,
    pos: TokenPosition,
}

impl ComptimeNamespace for EnumConstructor {
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
            format!("an enum constructor has no field: {field}"),
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
        let sections = self.ty.sections(env)?;
        let payload_ty = sections
            .cases
            .as_ref()
            .expect("constructors come from cases")[self.case]
            .ty(env)?;
        let payload = analyze(env, payload_ty.clone(), arg.pos.clone(), arg.ast, block)?;
        let payload = payload_ty.cast_into(env, block, payload, arg.pos)?;
        crate::kw::emit(
            env,
            block,
            pos,
            KwBuiltinOp::EnumNew,
            vec![kw_int(self.case), payload.value],
            Type::User(self.ty.clone()),
        )
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

pub fn declare(env: &mut Env, ast: &BlockToken, kind: DeclKind) -> AnalysisResult {
    let ty = UserType(Rc::new(UserTypeInner {
        pos: ast.pos.clone(),
        kind,
        ast: ComptimeValueAst {
            ast: ast.items.clone(),
            pos: ast.pos.clone(),
            scope: env.scope.clone(),
        },
        name: OnceCell::new(),
        sections: RefCell::new(None),
        building: Cell::new(false),
    }));
    AnalysisResult {
        ty: Type::CtType(CtType),
        value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
            ty: Type::User(ty),
        })),
    }
}

pub fn inline_entry(entry: AnalysisResult) -> AnalysisResult {
    match entry.value {
        RuntimeValue::Comptime(ComptimeValue::Fn(func)) => AnalysisResult {
            ty: Type::InlineFn(TypeInlineFn { func: func.clone() }),
            value: RuntimeValue::Comptime(ComptimeValue::Fn(func)),
        },
        value => AnalysisResult {
            ty: entry.ty,
            value,
        },
    }
}

pub fn unwrap_to(
    env: &mut Env,
    value: AnalysisResult,
    pos: &TokenPosition,
) -> Result<AnalysisResult, PositionedError> {
    let mut value = value;
    while let Type::User(t) = &value.ty {
        value = AnalysisResult {
            ty: t.repr(env, pos)?,
            value: value.value,
        };
    }
    Ok(value)
}

thread_local! {
    static INLINE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub fn inline_call(
    env: &mut Env,
    func: &ComptimeValueFn,
    pre: Vec<AnalysisResult>,
    arg: CallArg,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let pos = arg.pos.clone();
    let Type::Tuple(arg_types) = &func.args().ty else {
        return Err(throw_err(
            env,
            Some(func.pos().clone()),
            "a function used through a type's keys must take a list of arguments",
            None,
            None,
        ));
    };
    let DestructureExtract::List {
        items: extracts, ..
    } = &func.args().extract
    else {
        unreachable!("a tuple argument type comes from a list destructure")
    };
    let rest_types = &arg_types.children[pre.len().min(arg_types.children.len())..];

    let mut values = Vec::new();
    for (value, ty) in pre.into_iter().zip(&arg_types.children) {
        values.push(ty.cast_into(env, block, value, pos.clone())?);
    }
    let list = call_list_items(env, &arg)?;
    match (rest_types, list) {
        ([ty], None) => {
            let value = analyze(env, ty.clone(), arg.pos.clone(), arg.ast, block)?;
            values.push(ty.cast_into(env, block, value, pos.clone())?);
        }
        (types, Some(items)) if items.len() == types.len() => {
            for (item, ty) in items.iter().zip(types) {
                let value = analyze(env, ty.clone(), item.pos.clone(), &item.items, block)?;
                values.push(ty.cast_into(env, block, value, item.pos.clone())?);
            }
        }
        ([], None) if trim_ws(arg.ast).is_empty() => {}
        (types, _) => {
            return Err(throw_err(
                env,
                Some(pos),
                format!("expected {} more arguments", types.len()),
                None,
                None,
            ));
        }
    }

    let depth = INLINE_DEPTH.with(|d| d.get());
    if depth > 256 {
        return Err(throw_err(
            env,
            Some(pos),
            "inlining is nested too deeply; is a type's key calling itself?",
            None,
            None,
        ));
    }
    let mut bindings = func.body().scope.bindings.borrow().clone();
    let targets = &func.args().targets;
    for (extract, value) in extracts.iter().zip(values) {
        match extract {
            DestructureExtract::SingleItem { target, pos } => {
                bindings.insert(
                    targets[*target].name.clone(),
                    Binding::Runtime {
                        pos: pos.clone(),
                        runtime: value,
                    },
                );
            }
            DestructureExtract::Discard { .. } => {}
            DestructureExtract::Comptime { target, pos } => {
                let name = &targets[*target].name;
                if let RuntimeValue::Runtime(_) = value.value {
                    return Err(throw_err(
                        env,
                        Some(arg.pos.clone()),
                        format!("{name} must be known at compile time"),
                        Some(vec![(Some(pos.clone()), "declared here".to_string())]),
                        None,
                    ));
                }
                bindings.insert(
                    name.clone(),
                    Binding::Runtime {
                        pos: pos.clone(),
                        runtime: value,
                    },
                );
            }
            _ => {
                return Err(throw_err(
                    env,
                    Some(func.pos().clone()),
                    "TODO nested destructuring in an inlined function's arguments",
                    None,
                    None,
                ));
            }
        }
    }
    let saved = std::mem::replace(&mut env.scope.bindings, Rc::new(RefCell::new(bindings)));
    INLINE_DEPTH.with(|d| d.set(depth + 1));
    let result = analyze(
        env,
        Type::Unknown(TypeUnknown),
        func.pos().clone(),
        &func.body().ast,
        block,
    );
    INLINE_DEPTH.with(|d| d.set(depth));
    env.scope.bindings = saved;
    result
}

const PRELUDE_C: &str = include_str!("prelude/c.qxc");
const PRELUDE_REFLECT: &str = include_str!("prelude/reflect.qxc");

pub fn prelude_sources() -> Vec<crate::parser::Source> {
    vec![
        crate::parser::Source::new("prelude/c.qxc", PRELUDE_C),
        crate::parser::Source::new("prelude/reflect.qxc", PRELUDE_REFLECT),
    ]
}

#[derive(Debug)]
pub struct LazyPrelude {
    filename: &'static str,
    source: &'static str,
    pos: TokenPosition,
    namespace: RefCell<Option<Rc<dyn ComptimeNamespace>>>,
}

impl LazyPrelude {
    fn new(filename: &'static str, source: &'static str) -> Self {
        LazyPrelude {
            filename,
            source,
            pos: TokenPosition {
                fyl: filename.to_string(),
                idx: 0,
                lyn: 1,
                col: 1,
            },
            namespace: RefCell::new(None),
        }
    }

    pub fn c() -> Self {
        LazyPrelude::new("prelude/c.qxc", PRELUDE_C)
    }

    pub fn reflect() -> Rc<Self> {
        thread_local! {
            static REFLECT: Rc<LazyPrelude> =
                Rc::new(LazyPrelude::new("prelude/reflect.qxc", PRELUDE_REFLECT));
        }
        REFLECT.with(Rc::clone)
    }

    pub fn user_type(&self, env: &mut Env, name: &str) -> Result<UserType, PositionedError> {
        let pos = self.pos.clone();
        let found = self.get_string(env, pos.clone(), name, &mut empty_block())?;
        match get_comptime(env, Some(ComptimeValueKind::Type), found.value, pos.clone())? {
            ComptimeValue::Type(ComptimeValueType {
                ty: Type::User(user),
            }) => Ok(user),
            _ => Err(throw_err(
                env,
                Some(pos),
                format!("{}'s {name} is not a std.Type", self.filename),
                None,
                None,
            )),
        }
    }

    fn namespace(&self, env: &mut Env) -> Result<Rc<dyn ComptimeNamespace>, PositionedError> {
        if let Some(namespace) = self.namespace.borrow().as_ref() {
            return Ok(namespace.clone());
        }
        let mut source = crate::parser::Source::new(self.filename, self.source);
        let tokenized = tokenize(&mut source);
        if let Some(error) = tokenized.errors.first() {
            return Err(PositionedError::Fresh(error.clone()));
        }
        let mut changes = HashMap::new();
        changes.insert(target_env_symbol(), TargetEnv::Build);
        let scope = Scope {
            comptime: ComptimeScopeMap::root(changes),
            bindings: Rc::new(RefCell::new(HashMap::new())),
        };
        let pos = self.pos.clone();
        let namespace = with_scope(env, scope, |env| {
            analyze_namespace(env, pos, &tokenized.result)
        })?;
        *self.namespace.borrow_mut() = Some(namespace.clone());
        Ok(namespace)
    }
}

impl ComptimeNamespace for LazyPrelude {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        self.namespace(env)?.get_string(env, pos, field, block)
    }

    fn get_symbol(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        keychild: Type,
        field: Symbol,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        self.namespace(env)?
            .get_symbol(env, pos, keychild, field, block)
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}
