use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::{
    AnalysisBlock, AnalysisResult, Binding, ComptimeNamespace, ComptimeScopeMap, ComptimeValue,
    ComptimeValueAst, ComptimeValueFn, ComptimeValueType, DestructureExtract, Env, PositionedError,
    RuntimeValue, Scope, Symbol, TargetEnv, analyze, analyze_namespace, empty_block,
    target_env_symbol, throw_err, trim_ws, with_scope,
};
use crate::comptime::{ComptimeValueKind, get_comptime};
use crate::ct::{CallArg, CtType, Type, TypeInlineFn, TypeUnknown, call_list_items};
use crate::parser::{BlockToken, TokenPosition, tokenize};
use crate::std_keys::{StdKey, std_key};

pub struct UserTypeInner {
    pos: TokenPosition,
    ast: ComptimeValueAst,
    name: OnceCell<String>,
    namespace: RefCell<Option<Rc<dyn ComptimeNamespace>>>,
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

    fn namespace(&self, env: &mut Env) -> Result<Rc<dyn ComptimeNamespace>, PositionedError> {
        if let Some(namespace) = self.0.namespace.borrow().as_ref() {
            return Ok(namespace.clone());
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
        let result = with_scope(env, self.0.ast.scope.clone(), |env| {
            analyze_namespace(env, pos, &ast)
        });
        self.0.building.set(false);
        let namespace = result?;
        *self.0.namespace.borrow_mut() = Some(namespace.clone());
        Ok(namespace)
    }

    pub fn lookup(
        &self,
        env: &mut Env,
        key: Symbol,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let namespace = self.namespace(env)?;
        namespace.get_symbol(
            env,
            self.0.pos.clone(),
            Type::Unknown(TypeUnknown),
            key,
            &mut empty_block(),
        )
    }

    pub fn repr(&self, env: &mut Env, pos: &TokenPosition) -> Result<Type, PositionedError> {
        let Some(repr) = self.lookup(env, std_key(StdKey::Repr))? else {
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
}

pub fn declare(env: &mut Env, ast: &BlockToken) -> AnalysisResult {
    let ty = UserType(Rc::new(UserTypeInner {
        pos: ast.pos.clone(),
        ast: ComptimeValueAst {
            ast: ast.items.clone(),
            pos: ast.pos.clone(),
            scope: env.scope.clone(),
        },
        name: OnceCell::new(),
        namespace: RefCell::new(None),
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

pub fn prelude_sources() -> Vec<crate::parser::Source> {
    vec![crate::parser::Source::new("prelude/c.qxc", PRELUDE_C)]
}

#[derive(Debug)]
pub struct LazyPrelude {
    filename: &'static str,
    source: &'static str,
    pos: TokenPosition,
    namespace: RefCell<Option<Rc<dyn ComptimeNamespace>>>,
}

impl LazyPrelude {
    pub fn c() -> Self {
        LazyPrelude {
            filename: "prelude/c.qxc",
            source: PRELUDE_C,
            pos: TokenPosition {
                fyl: "prelude/c.qxc".to_string(),
                idx: 0,
                lyn: 1,
                col: 1,
            },
            namespace: RefCell::new(None),
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
