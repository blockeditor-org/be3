use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ct::{
    CExportName, CInt, CallArg, CtAst, CtBuildArtifact, CtBuildArtifactNarrow, CtExportList, CtKey,
    CtNamespace, CtType, OperatorName, Type, TypeBound, TypeFn, TypeLabel, TypeTuple, TypeUnknown,
    TypeVoid,
};
use crate::parser::{
    BinaryExpressionToken, BlockToken, BracketTag, IdentifierTag, OpTag, OperatorSegmentToken,
    OperatorToken, RawTag, RawToken, Source, SyntaxNode, TokenPosition, TokenizationError,
    TokenizationErrorEntry, TraceEntry, tokenize,
};
use crate::printers::printers::AST_NODE;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum PositionedError {
    Fresh(TokenizationError),
    Consumed,
}

impl std::fmt::Display for PositionedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let e = match self {
            PositionedError::Fresh(e) => e,
            PositionedError::Consumed => return write!(f, "(error already reported)"),
        };
        let mut lines = Vec::new();
        for entry in &e.entries {
            let fyl = entry.pos.as_ref().map(|p| p.fyl.as_str()).unwrap_or("???");
            let lyn = entry
                .pos
                .as_ref()
                .map(|p| p.lyn.to_string())
                .unwrap_or_else(|| "???".to_string());
            let col = entry
                .pos
                .as_ref()
                .map(|p| p.col.to_string())
                .unwrap_or_else(|| "???".to_string());
            lines.push(format!(
                "{fyl}:{lyn}:{col}: {:?}: {}",
                entry.style, entry.message
            ));
        }
        for trace in &e.trace {
            lines.push(format!(
                " at {}:{}:{} ({})",
                trace.pos.fyl, trace.pos.lyn, trace.pos.col, trace.text
            ));
        }
        write!(f, "{}", lines.join("\n"))
    }
}

impl std::error::Error for PositionedError {}

pub fn compiler_pos() -> TokenPosition {
    TokenPosition {
        fyl: "compiler".to_string(),
        lyn: 0,
        col: 0,
        idx: 0,
    }
}

pub fn empty_block() -> AnalysisBlock {
    AnalysisBlock {
        offset: 0,
        lines: Vec::new(),
        validate: Symbol::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u64);

impl Symbol {
    pub fn new() -> Self {
        static NEXT_SYMBOL_ID: AtomicU64 = AtomicU64::new(0);
        Symbol(NEXT_SYMBOL_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NsKey {
    Str(String),
    Sym(Symbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumedErrorToken;

pub fn throw_consumed_err(_consumed: ConsumedErrorToken) -> PositionedError {
    PositionedError::Consumed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetEnv {
    Build,
    C,
    User(Symbol),
}

pub fn target_env_symbol() -> Symbol {
    static TARGET_ENV_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *TARGET_ENV_SYMBOL.get_or_init(Symbol::new)
}

#[derive(Debug)]
pub struct ComptimeScopeMap {
    parent: Option<Rc<ComptimeScopeMap>>,
    changes: HashMap<Symbol, TargetEnv>,
}

impl ComptimeScopeMap {
    pub fn root(changes: HashMap<Symbol, TargetEnv>) -> Rc<Self> {
        Rc::new(ComptimeScopeMap {
            parent: None,
            changes,
        })
    }

    pub fn get(self: &Rc<Self>, key: Symbol) -> Option<TargetEnv> {
        if let Some(v) = self.changes.get(&key) {
            return Some(*v);
        }
        self.parent.as_ref().and_then(|p| p.get(key))
    }

    pub fn snapshot(&self) -> ComptimeSnapshot {
        let mut values: HashMap<Symbol, TargetEnv> = HashMap::new();
        let mut current = Some(self);
        while let Some(map) = current {
            for (key, value) in &map.changes {
                values.entry(*key).or_insert(*value);
            }
            current = map.parent.as_deref();
        }
        let mut values: Vec<(Symbol, TargetEnv)> = values.into_iter().collect();
        values.sort_by_key(|(key, _)| *key);
        values
    }

    pub fn sub(self: &Rc<Self>, changes: HashMap<Symbol, TargetEnv>) -> Rc<Self> {
        Rc::new(ComptimeScopeMap {
            parent: Some(self.clone()),
            changes,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Binding {
    Valid {
        pos: TokenPosition,
        decl: ComptimeValueDeclaration,
    },
    Runtime {
        pos: TokenPosition,
        runtime: AnalysisResult,
    },
    Error {
        pos: TokenPosition,
        consumed: ConsumedErrorToken,
    },
    Removed {
        pos: TokenPosition,
    },
}

fn binding_pos(binding: &Binding) -> &TokenPosition {
    match binding {
        Binding::Valid { pos, .. } => pos,
        Binding::Runtime { pos, .. } => pos,
        Binding::Error { pos, .. } => pos,
        Binding::Removed { pos } => pos,
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub comptime: Rc<ComptimeScopeMap>,
    pub bindings: Rc<RefCell<HashMap<String, Binding>>>,
}

pub trait CacheKey {
    fn cache_ptr(&self) -> usize;
}

pub type ComptimeSnapshot = Vec<(Symbol, TargetEnv)>;

pub struct PerComptimeScopeCache<K, V> {
    entries: RefCell<HashMap<(usize, ComptimeSnapshot), V>>,
    in_progress: RefCell<HashSet<(usize, ComptimeSnapshot)>>,
    _marker: PhantomData<K>,
}

impl<K, V> Default for PerComptimeScopeCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> PerComptimeScopeCache<K, V> {
    pub fn new() -> Self {
        PerComptimeScopeCache {
            entries: RefCell::new(HashMap::new()),
            in_progress: RefCell::new(HashSet::new()),
            _marker: PhantomData,
        }
    }
}

impl<K: CacheKey, V: Clone> PerComptimeScopeCache<K, V> {
    pub fn is_in_progress(&self, key: &K, comptime: &ComptimeScopeMap) -> bool {
        self.in_progress
            .borrow()
            .contains(&(key.cache_ptr(), comptime.snapshot()))
    }

    pub fn get_or_put(
        &self,
        key: &K,
        comptime: &ComptimeScopeMap,
        env: &mut Env,
        cb: impl FnOnce(&mut Env) -> Result<V, PositionedError>,
    ) -> Result<V, PositionedError> {
        let key = (key.cache_ptr(), comptime.snapshot());
        if let Some(v) = self.entries.borrow().get(&key) {
            return Ok(v.clone());
        }
        if !self.in_progress.borrow_mut().insert(key.clone()) {
            return Err(throw_err(env, None, "dependency loop", None, None));
        }
        match cb(env) {
            Ok(v) => {
                self.in_progress.borrow_mut().remove(&key);
                self.entries.borrow_mut().insert(key, v.clone());
                Ok(v)
            }
            Err(e) => Err(e),
        }
    }
}

pub struct Env {
    pub trace: Vec<TraceEntry>,
    pub errors: Vec<TokenizationError>,
    pub scope: Scope,
    pub fn_cache: Rc<PerComptimeScopeCache<ComptimeValueFn, AnalyzedFn>>,
    pub decl_cache: Rc<PerComptimeScopeCache<ComptimeValueDeclaration, ComptimeAnalysisResult>>,
    pub builtin_cache: Rc<PerComptimeScopeCache<Rc<dyn Descriptor>, AnalysisResult>>,
}

pub fn with_scope<R>(
    env: &mut Env,
    scope: Scope,
    f: impl FnOnce(&mut Env) -> Result<R, PositionedError>,
) -> Result<R, PositionedError> {
    let saved = std::mem::replace(&mut env.scope, scope);
    let result = f(env);
    env.scope = saved;
    result
}

pub fn with_target_env<R>(
    env: &mut Env,
    target: TargetEnv,
    f: impl FnOnce(&mut Env) -> Result<R, PositionedError>,
) -> Result<R, PositionedError> {
    let mut changes = HashMap::new();
    changes.insert(target_env_symbol(), target);
    let new_comptime = env.scope.comptime.sub(changes);
    let old_comptime = std::mem::replace(&mut env.scope.comptime, new_comptime);
    let result = f(env);
    env.scope.comptime = old_comptime;
    result
}

#[derive(Debug, Clone)]
pub struct ComptimeValueAst {
    pub ast: Vec<SyntaxNode>,
    pub pos: TokenPosition,
    pub scope: Scope,
}

#[derive(Debug)]
struct ComptimeValueDeclarationInner {
    ast: ComptimeValueAst,
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueDeclaration(Rc<ComptimeValueDeclarationInner>);

impl ComptimeValueDeclaration {
    pub fn ast(&self) -> &ComptimeValueAst {
        &self.0.ast
    }
}

impl CacheKey for ComptimeValueDeclaration {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}

pub fn create_declaration(_env: &mut Env, ast: ComptimeValueAst) -> ComptimeValueDeclaration {
    ComptimeValueDeclaration(Rc::new(ComptimeValueDeclarationInner { ast, name: None }))
}

pub fn create_named_declaration(ast: ComptimeValueAst, name: &str) -> ComptimeValueDeclaration {
    ComptimeValueDeclaration(Rc::new(ComptimeValueDeclarationInner {
        ast,
        name: Some(name.to_string()),
    }))
}

pub fn get_declaration(
    env: &mut Env,
    decl: ComptimeValueDeclaration,
) -> Result<ComptimeAnalysisResult, PositionedError> {
    let cache = env.decl_cache.clone();
    let key = decl.clone();
    let comptime = decl.ast().scope.comptime.clone();
    cache.get_or_put(&key, &comptime, env, move |env| {
        let scope = decl.ast().scope.clone();
        with_scope(env, scope, |env| {
            let mut block = empty_block();
            let result = analyze(
                env,
                Type::Unknown(TypeUnknown),
                decl.ast().pos.clone(),
                &decl.ast().ast,
                &mut block,
            )?;
            let evald =
                crate::comptime::comptime_eval(env, &block, result.value, decl.ast().pos.clone())?;
            if let (Some(name), ComptimeValue::Type(ComptimeValueType { ty: Type::User(t) })) =
                (&decl.0.name, &evald)
            {
                t.set_name(name);
            }
            Ok(ComptimeAnalysisResult {
                ty: result.ty,
                value: evald,
            })
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockIdx(pub usize, pub Symbol);

#[derive(Debug, Clone)]
pub enum AnalysisLine {
    ComptimeKvListInit {
        pos: TokenPosition,
    },
    ComptimeKvListAppend {
        pos: TokenPosition,
        key: RuntimeValue,
        list: RuntimeValue,
        value: RuntimeValue,
    },
    Call {
        pos: TokenPosition,
        method: RuntimeValue,
        arg: RuntimeValue,
    },
    Break {
        pos: TokenPosition,
        label: Symbol,
        value: RuntimeValue,
    },
    Args {
        pos: TokenPosition,
    },
    Tuple {
        pos: TokenPosition,
        items: Vec<RuntimeValue>,
    },
    TupleGet {
        pos: TokenPosition,
        tuple: RuntimeValue,
        index: usize,
    },
    CBinary {
        pos: TokenPosition,
        op: crate::backend::c::CBinaryOp,
        lhs: RuntimeValue,
        rhs: RuntimeValue,
    },
    LabelBegin {
        pos: TokenPosition,
        label: Symbol,
        ty: Type,
    },
    LabelEnd {
        pos: TokenPosition,
        label: Symbol,
        value: RuntimeValue,
    },
    KwBinary {
        pos: TokenPosition,
        op: crate::backend::c::CBinaryOp,
        lhs: RuntimeValue,
        rhs: RuntimeValue,
    },
    RegionBegin {
        pos: TokenPosition,
        region: Region,
    },
    RegionEnd {
        pos: TokenPosition,
    },
    MutNew {
        pos: TokenPosition,
        init: RuntimeValue,
    },
    KwBuiltin {
        pos: TokenPosition,
        op: crate::kw::KwBuiltinOp,
        args: Vec<RuntimeValue>,
    },
    MutGet {
        pos: TokenPosition,
        cell: RuntimeValue,
    },
    MutSet {
        pos: TokenPosition,
        cell: RuntimeValue,
        value: RuntimeValue,
    },
    ComptimeFileCreate {
        pos: TokenPosition,
        value: RuntimeValue,
    },
    Emit {
        pos: TokenPosition,
        data: ComptimeValue,
        data_ty: Type,
        operands: Vec<RuntimeValue>,
        ty: Type,
    },
}

pub fn set_line_pos(line: &mut AnalysisLine, new_pos: TokenPosition) {
    match line {
        AnalysisLine::ComptimeKvListInit { pos }
        | AnalysisLine::ComptimeKvListAppend { pos, .. }
        | AnalysisLine::Call { pos, .. }
        | AnalysisLine::Break { pos, .. }
        | AnalysisLine::Args { pos }
        | AnalysisLine::Tuple { pos, .. }
        | AnalysisLine::TupleGet { pos, .. }
        | AnalysisLine::CBinary { pos, .. }
        | AnalysisLine::LabelBegin { pos, .. }
        | AnalysisLine::LabelEnd { pos, .. }
        | AnalysisLine::KwBinary { pos, .. }
        | AnalysisLine::RegionBegin { pos, .. }
        | AnalysisLine::RegionEnd { pos }
        | AnalysisLine::MutNew { pos, .. }
        | AnalysisLine::KwBuiltin { pos, .. }
        | AnalysisLine::MutGet { pos, .. }
        | AnalysisLine::MutSet { pos, .. }
        | AnalysisLine::ComptimeFileCreate { pos, .. }
        | AnalysisLine::Emit { pos, .. } => *pos = new_pos,
    }
}

#[derive(Debug, Clone)]
pub enum Region {
    CIf { cond: RuntimeValue },
    KwIf { cond: RuntimeValue },
    KwElse { if_end: BlockIdx },
    KwLoop,
}

#[derive(Debug, Clone)]
pub struct AnalysisBlock {
    pub offset: usize,
    pub lines: Vec<AnalysisLine>,
    pub validate: Symbol,
}

pub fn block_append(block: &mut AnalysisBlock, instr: AnalysisLine) -> BlockIdx {
    block.lines.push(instr);
    BlockIdx(block.lines.len() - 1, block.validate)
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub ty: Type,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone)]
pub struct ComptimeAnalysisResult {
    pub ty: Type,
    pub value: ComptimeValue,
}

pub fn cast_value(to: Type, result: AnalysisResult) -> AnalysisResult {
    AnalysisResult {
        ty: to,
        value: result.value,
    }
}

#[derive(Debug, Clone)]
pub enum ComptimeValueKey {
    Symbol { key: Symbol, child: Type },
    String { key: String },
}

pub trait ComptimeNamespace: std::fmt::Debug {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>;
    fn get_symbol(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        keychild: Type,
        field: Symbol,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError>;
    fn analyze_call(
        &self,
        env: &mut Env,
        _slot: Type,
        pos: TokenPosition,
        _arg: CallArg<'_>,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        Err(throw_err(
            env,
            Some(pos),
            "this namespace does not support call",
            Some(vec![(Some(self.pos().clone()), "defined here".to_string())]),
            None,
        ))
    }
    fn try_get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        self.get_string(env, pos, field, block).map(Some)
    }
    fn pos(&self) -> &TokenPosition;
}

pub type BuiltinFn = for<'a> fn(
    &mut Env,
    Type,
    TokenPosition,
    CallArg<'a>,
    &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError>;

#[derive(Debug, Clone)]
pub struct ComptimeValueType {
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueVoid;

#[derive(Debug, Clone)]
pub struct NsFieldsEntry {
    pub pos: TokenPosition,
    pub key: ComptimeValue,
    pub value: ComptimeValue,
}

#[derive(Debug, Clone)]
pub struct NsFields {
    pub locked: bool,
    pub entries: Vec<NsFieldsEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueOptional {
    pub some: Option<Box<ComptimeValue>>,
}

#[derive(Debug, Clone)]
pub struct ComptimeFile {
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ComptimeFolder {
    pub value: Vec<(String, ComptimeValueBuildArtifact)>,
}

#[derive(Debug, Clone)]
pub enum ComptimeValueBuildArtifact {
    File(ComptimeFile),
    Folder(ComptimeFolder),
}

#[derive(Debug, Clone)]
pub struct Uint8ArraySourcemapEntry {
    pub len_bytes: usize,
    pub start_pos: TokenPosition,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueUint8Array {
    pub value: Vec<u8>,
    pub sourcemap: Vec<Uint8ArraySourcemapEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueCExportName {
    pub value: crate::backend::c::CValidatedIdentifierName,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueCInt {
    pub value: i32,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueKwInt {
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueKwString {
    buf: Rc<RefCell<String>>,
    len: usize,
}

impl ComptimeValueKwString {
    pub fn new(value: String) -> Self {
        let len = value.len();
        ComptimeValueKwString {
            buf: Rc::new(RefCell::new(value)),
            len,
        }
    }

    pub fn with_str<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.buf.borrow()[..self.len])
    }

    pub fn to_owned_string(&self) -> String {
        self.with_str(str::to_string)
    }

    pub fn concat(&self, other: &ComptimeValueKwString) -> ComptimeValueKwString {
        let appended = other.with_str(|other| {
            let mut buf = self.buf.borrow_mut();
            if buf.len() != self.len {
                return None;
            }
            buf.push_str(other);
            Some(buf.len())
        });
        match appended {
            Some(len) => ComptimeValueKwString {
                buf: self.buf.clone(),
                len,
            },
            None => {
                let mut value = self.to_owned_string();
                other.with_str(|other| value.push_str(other));
                ComptimeValueKwString::new(value)
            }
        }
    }
}

impl PartialEq for ComptimeValueKwString {
    fn eq(&self, other: &Self) -> bool {
        self.with_str(|a| other.with_str(|b| a == b))
    }
}

#[derive(Debug, Clone)]
pub struct ComptimeValueKwList {
    buf: Rc<RefCell<Vec<ComptimeValue>>>,
    len: usize,
}

impl ComptimeValueKwList {
    pub fn new(items: Vec<ComptimeValue>) -> Self {
        let len = items.len();
        ComptimeValueKwList {
            buf: Rc::new(RefCell::new(items)),
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<ComptimeValue> {
        (index < self.len).then(|| self.buf.borrow()[index].clone())
    }

    pub fn to_vec(&self) -> Vec<ComptimeValue> {
        self.buf.borrow()[..self.len].to_vec()
    }

    pub fn push(&self, item: ComptimeValue) -> ComptimeValueKwList {
        let mut buf = self.buf.borrow_mut();
        if buf.len() == self.len {
            buf.push(item);
            return ComptimeValueKwList {
                buf: self.buf.clone(),
                len: self.len + 1,
            };
        }
        let mut items = buf[..self.len].to_vec();
        items.push(item);
        ComptimeValueKwList::new(items)
    }
}

#[derive(Debug, Clone)]
pub struct ComptimeValueKwBool {
    pub value: bool,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueEnum {
    pub case: usize,
    pub payload: Option<Box<ComptimeValue>>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueTuple {
    pub items: Vec<ComptimeValue>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueOperatorName {
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueExportListEntry {
    pub key: ComptimeValue,
    pub key_pos: TokenPosition,
    pub value: ComptimeValueAst,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueExportList {
    pub exports: Vec<ComptimeValueExportListEntry>,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueError {
    pub etok: ConsumedErrorToken,
}

type Specializations = RefCell<Vec<(Vec<ComptimeValue>, ComptimeValueFn)>>;

#[derive(Debug)]
struct ComptimeValueFnInner {
    args: Destructure,
    body: ComptimeValueAst,
    pos: TokenPosition,
    specializations: Specializations,
}

#[derive(Debug, Clone)]
pub struct ComptimeValueFn(Rc<ComptimeValueFnInner>);

impl ComptimeValueFn {
    pub fn new(args: Destructure, body: ComptimeValueAst, pos: TokenPosition) -> Self {
        ComptimeValueFn(Rc::new(ComptimeValueFnInner {
            args,
            body,
            pos,
            specializations: RefCell::new(Vec::new()),
        }))
    }

    pub fn is_inline(&self) -> bool {
        self.0
            .args
            .tags
            .iter()
            .any(|tag| matches!(tag, DestructureTag::Inline { .. }))
    }

    pub fn has_comptime_params(&self) -> bool {
        matches!(
            &self.0.args.extract,
            DestructureExtract::List { items, .. }
                if items.iter().any(|item| matches!(item, DestructureExtract::Comptime { .. }))
        )
    }

    pub fn args(&self) -> &Destructure {
        &self.0.args
    }

    pub fn body(&self) -> &ComptimeValueAst {
        &self.0.body
    }

    pub fn pos(&self) -> &TokenPosition {
        &self.0.pos
    }
}

impl PartialEq for ComptimeValueFn {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ComptimeValueFn {}

impl std::hash::Hash for ComptimeValueFn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Rc::as_ptr(&self.0) as usize).hash(state);
    }
}

impl CacheKey for ComptimeValueFn {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedFn {
    pub block: AnalysisBlock,
    pub ty: Type,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone)]
pub enum ComptimeValue {
    Key(ComptimeValueKey),
    Namespace(Rc<dyn ComptimeNamespace>),
    Type(ComptimeValueType),
    Ast(ComptimeValueAst),
    Void(ComptimeValueVoid),
    KvFields(NsFields),
    Fn(ComptimeValueFn),
    Optional(ComptimeValueOptional),
    BuildArtifact(ComptimeValueBuildArtifact),
    Uint8Array(ComptimeValueUint8Array),
    ExportList(ComptimeValueExportList),
    CExportName(ComptimeValueCExportName),
    CInt(ComptimeValueCInt),
    OperatorName(ComptimeValueOperatorName),
    KwInt(ComptimeValueKwInt),
    KwBool(ComptimeValueKwBool),
    KwString(ComptimeValueKwString),
    KwList(ComptimeValueKwList),
    KwText(Rc<crate::kw::TextNode>),
    Tuple(ComptimeValueTuple),
    Struct(Vec<ComptimeValue>),
    Enum(ComptimeValueEnum),
    KwMut(Rc<RefCell<ComptimeValue>>),
    Error(ComptimeValueError),
    Target(ComptimeValueTarget),
    ReflectValue(crate::reflect::ReflectValue),
    ReflectConstant(Rc<ComptimeValue>),
    ReflectData(Rc<(ComptimeValue, Type)>),
    KwMap(Rc<Vec<(ComptimeValue, ComptimeValue)>>),
}

#[derive(Debug, Clone)]
pub struct ComptimeValueTarget {
    pub env: TargetEnv,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Comptime(ComptimeValue),
    Runtime(BlockIdx),
}

pub fn throw_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
    style: Option<crate::parser::ErrorStyle>,
) -> PositionedError {
    PositionedError::Fresh(get_err(env, pos, msg, notes, style))
}

pub fn add_err(
    env: &mut Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
) -> ConsumedErrorToken {
    let err = get_err(env, pos, msg, notes, None);
    env.errors.push(err);
    ConsumedErrorToken
}

pub fn get_err(
    env: &Env,
    pos: Option<TokenPosition>,
    msg: impl Into<String>,
    notes: Option<Vec<(Option<TokenPosition>, String)>>,
    style: Option<crate::parser::ErrorStyle>,
) -> TokenizationError {
    let mut entries = vec![TokenizationErrorEntry {
        pos,
        style: style.unwrap_or(crate::parser::ErrorStyle::Error),
        message: msg.into(),
    }];
    for (note_pos, note_msg) in notes.into_iter().flatten() {
        entries.push(TokenizationErrorEntry {
            pos: note_pos,
            style: crate::parser::ErrorStyle::Note,
            message: note_msg,
        });
    }
    TokenizationError {
        entries,
        trace: env.trace.clone(),
    }
}

fn handle_err(env: &mut Env, err: PositionedError) {
    if let PositionedError::Fresh(e) = err {
        env.errors.push(e);
    }
}

pub fn assert(condition: bool) {
    if !condition {
        panic!("assertion failed");
    }
}

pub(crate) fn syntax_node_kind(node: &SyntaxNode) -> &'static str {
    match node {
        SyntaxNode::Identifier(_) => "ident",
        SyntaxNode::Whitespace(_) => "ws",
        SyntaxNode::Operator(_) => "op",
        SyntaxNode::OperatorSegment(_) => "opSeg",
        SyntaxNode::Block(_) => "block",
        SyntaxNode::BinaryExpression(_) => "binary",
        SyntaxNode::Raw(_) => "raw",
        SyntaxNode::Err(_) => "err",
    }
}

fn syntax_node_pos(node: &SyntaxNode) -> &TokenPosition {
    match node {
        SyntaxNode::Identifier(t) => &t.pos,
        SyntaxNode::Whitespace(t) => &t.pos,
        SyntaxNode::Operator(t) => &t.pos,
        SyntaxNode::OperatorSegment(t) => &t.pos,
        SyntaxNode::Block(t) => &t.pos,
        SyntaxNode::BinaryExpression(t) => &t.pos,
        SyntaxNode::Raw(t) => &t.pos,
        SyntaxNode::Err(t) => &t.pos,
    }
}

fn op_tag_str(tag: OpTag) -> &'static str {
    match tag {
        OpTag::Sep => "sep",
        OpTag::Def => "def",
        OpTag::Pub => "pub",
        OpTag::Var => "var",
        OpTag::Assign => "assign",
        OpTag::Compare => "compare",
        OpTag::Add => "add",
        OpTag::Mul => "mul",
        OpTag::None => "",
    }
}

pub fn trim_ws(src: &[SyntaxNode]) -> Vec<SyntaxNode> {
    src.iter()
        .filter(|item| match item {
            SyntaxNode::Whitespace(_) => false,
            SyntaxNode::Block(b) if b.tag == BracketTag::InlineComment => false,
            _ => true,
        })
        .cloned()
        .collect()
}

fn strip_return(items: &[SyntaxNode]) -> Option<(TokenPosition, Vec<SyntaxNode>)> {
    let trimmed = trim_ws(items);
    match trimmed.first()? {
        SyntaxNode::Raw(r) if r.tag == RawTag::Return => {
            Some((r.pos.clone(), trimmed[1..].to_vec()))
        }
        SyntaxNode::BinaryExpression(bin) if trimmed.len() == 1 => {
            let first = bin
                .items
                .iter()
                .position(|item| matches!(item, SyntaxNode::OperatorSegment(_)))?;
            let SyntaxNode::OperatorSegment(seg) = &bin.items[first] else {
                unreachable!("position matched an operator segment")
            };
            let (return_pos, rest) = strip_return(&seg.items)?;
            let mut bin = bin.clone();
            bin.items[first] = SyntaxNode::OperatorSegment(OperatorSegmentToken {
                pos: seg.pos.clone(),
                items: rest,
            });
            Some((return_pos, vec![SyntaxNode::BinaryExpression(bin)]))
        }
        _ => None,
    }
}

pub type Binary2 = (OperatorSegmentToken, OperatorToken, OperatorSegmentToken);

pub fn read_binary2(
    env: &mut Env,
    root_src: &[SyntaxNode],
    kw: OpTag,
) -> Result<Option<Binary2>, PositionedError> {
    let root_src = trim_ws(root_src);
    if root_src.is_empty() {
        return Ok(None);
    }
    let SyntaxNode::BinaryExpression(bin) = &root_src[0] else {
        return Ok(None);
    };
    if bin.tag != kw {
        return Ok(None);
    }
    let src = trim_ws(&bin.items);
    if src.len() != 3 {
        return Err(throw_err(
            env,
            Some(bin.pos.clone()),
            "Expected LHS op RHS, found not that",
            None,
            None,
        ));
    }
    match (&src[0], &src[1], &src[2]) {
        (
            SyntaxNode::OperatorSegment(lhs),
            SyntaxNode::Operator(op),
            SyntaxNode::OperatorSegment(rhs),
        ) => Ok(Some((lhs.clone(), op.clone(), rhs.clone()))),
        _ => Ok(None),
    }
}

pub fn read_binary(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
    kw: OpTag,
) -> Result<Vec<OperatorSegmentToken>, PositionedError> {
    let src = trim_ws(src);
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let is_kw_binary = matches!(&src[0], SyntaxNode::BinaryExpression(b) if b.tag == kw);
    if !is_kw_binary {
        return Ok(vec![OperatorSegmentToken { pos, items: src }]);
    }
    if src.len() > 1 {
        return Err(throw_err(
            env,
            Some(syntax_node_pos(&src[1]).clone()),
            "Found extra trailing items while parsing readBinary",
            None,
            None,
        ));
    }
    let SyntaxNode::BinaryExpression(bin) = &src[0] else {
        unreachable!()
    };
    let mut out = Vec::new();
    for itm in &bin.items {
        match itm {
            SyntaxNode::OperatorSegment(seg) => out.push(seg.clone()),
            SyntaxNode::Operator(_) => {}
            other => {
                return Err(throw_err(
                    env,
                    Some(syntax_node_pos(other).clone()),
                    format!(
                        "Unexpected token in {}: {}",
                        op_tag_str(kw),
                        syntax_node_kind(other)
                    ),
                    None,
                    None,
                ));
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct DestructureTarget {
    pub name: String,
    pub pos: TokenPosition,
}

#[derive(Debug, Clone)]
pub enum DestructureTag {
    CallConv {
        pos: TokenPosition,
    },
    Inline {
        pos: TokenPosition,
    },
    Error {
        pos: TokenPosition,
        tok: ConsumedErrorToken,
    },
}

#[derive(Debug, Clone)]
pub struct Destructure {
    pub targets: Vec<DestructureTarget>,
    pub extract: DestructureExtract,
    pub ty: Type,
    pub tags: Vec<DestructureTag>,
}

#[derive(Debug, Clone)]
pub enum DestructureExtract {
    SingleItem {
        target: usize,
        pos: TokenPosition,
    },
    List {
        items: Vec<DestructureExtract>,
        pos: TokenPosition,
    },
    Map {
        items: Vec<(ComptimeValueKey, DestructureExtract)>,
        pos: TokenPosition,
    },
    Discard {
        pos: TokenPosition,
    },
    Comptime {
        target: usize,
        pos: TokenPosition,
    },
}

fn destructure_extract_pos(extract: &DestructureExtract) -> &TokenPosition {
    match extract {
        DestructureExtract::SingleItem { pos, .. } => pos,
        DestructureExtract::List { pos, .. } => pos,
        DestructureExtract::Map { pos, .. } => pos,
        DestructureExtract::Discard { pos } => pos,
        DestructureExtract::Comptime { pos, .. } => pos,
    }
}

fn read_comptime_param(
    env: &mut Env,
    arg: &OperatorSegmentToken,
    targets: &mut Vec<DestructureTarget>,
) -> Result<Option<(DestructureExtract, Type)>, PositionedError> {
    let Some((lhs, _, rhs)) = read_binary2(env, &arg.items, OpTag::Def)? else {
        return Ok(None);
    };
    let name = match trim_ws(&lhs.items).as_slice() {
        [SyntaxNode::Identifier(id)] if id.ident_tag == IdentifierTag::Normal => id.clone(),
        _ => {
            return Err(throw_err(
                env,
                Some(lhs.pos.clone()),
                "a compile-time parameter is a name, as in `name :: T`",
                None,
                None,
            ));
        }
    };
    let mut sub_block = empty_block();
    let body = analyze(
        env,
        Type::CtType(CtType),
        rhs.pos.clone(),
        &rhs.items,
        &mut sub_block,
    )?;
    let evaluated = crate::comptime::comptime_eval(env, &sub_block, body.value, rhs.pos.clone())?;
    let ComptimeValue::Type(ty) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::Type),
        RuntimeValue::Comptime(evaluated),
        rhs.pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    let target = targets.len();
    targets.push(DestructureTarget {
        name: name.str.clone(),
        pos: name.pos.clone(),
    });
    Ok(Some((
        DestructureExtract::Comptime {
            target,
            pos: name.pos.clone(),
        },
        ty.ty,
    )))
}

pub fn read_destructure(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
    targets: &mut Vec<DestructureTarget>,
) -> Result<Destructure, PositionedError> {
    let trimmed = trim_ws(src);
    let is_builtin_tag = |itm: &&SyntaxNode| matches!(itm, SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Builtin);
    let raw_tags: Vec<SyntaxNode> = trimmed.iter().filter(is_builtin_tag).cloned().collect();
    let mut lhs_items: Vec<SyntaxNode> = trimmed
        .iter()
        .filter(|itm| !is_builtin_tag(itm))
        .cloned()
        .collect();
    if lhs_items.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            format!(
                "Expected at least one item to destructure{}",
                AST_NODE.dump_list(src, 2)
            ),
            None,
            None,
        ));
    }

    let mut ty: Option<Type> = None;
    {
        let last = lhs_items.last().expect("checked non-empty above");
        if let SyntaxNode::Block(b) = last
            && b.tag == BracketTag::ColonCall
        {
            let mut sub_block = empty_block();
            let body = analyze(
                env,
                Type::CtType(CtType),
                b.pos.clone(),
                &b.items,
                &mut sub_block,
            )?;
            let evaluated =
                crate::comptime::comptime_eval(env, &sub_block, body.value, b.pos.clone())?;
            let got = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::Type),
                RuntimeValue::Comptime(evaluated),
                b.pos.clone(),
            )?;
            let ComptimeValue::Type(got) = got else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            ty = Some(got.ty);
        }
    }
    if ty.is_some() {
        lhs_items.pop();
        if lhs_items.is_empty() {
            return Err(throw_err(
                env,
                Some(pos),
                "Expected a name before the type",
                None,
                None,
            ));
        }
    }

    if lhs_items.len() > 1 {
        return Err(throw_err(
            env,
            Some(syntax_node_pos(&lhs_items[1]).clone()),
            format!(
                "Unexpected item for destructuring. TODO support eg 'name: type := value'{}",
                AST_NODE.dump_list(src, 2)
            ),
            None,
            None,
        ));
    }

    let tags: Vec<DestructureTag> = raw_tags
        .into_iter()
        .map(|tag| {
            if let SyntaxNode::Identifier(id) = &tag
                && id.str == "callconv_c"
            {
                return DestructureTag::CallConv {
                    pos: id.pos.clone(),
                };
            }
            if let SyntaxNode::Identifier(id) = &tag
                && id.str == "inline"
            {
                return DestructureTag::Inline {
                    pos: id.pos.clone(),
                };
            }
            let tag_pos = syntax_node_pos(&tag).clone();
            let tok = add_err(
                env,
                Some(tag_pos.clone()),
                format!("Bad tag: {}", AST_NODE.dump(&tag, 3)),
                None,
            );
            DestructureTag::Error { pos: tag_pos, tok }
        })
        .collect();

    let ident = &lhs_items[0];
    match ident {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Normal => {
            let target_idx = targets.len();
            targets.push(DestructureTarget {
                name: id.str.clone(),
                pos: id.pos.clone(),
            });
            Ok(Destructure {
                extract: DestructureExtract::SingleItem {
                    target: target_idx,
                    pos: id.pos.clone(),
                },
                ty: ty.unwrap_or(Type::Unknown(TypeUnknown)),
                tags: tags.clone(),
                targets: targets.clone(),
            })
        }
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Discard => Ok(Destructure {
            extract: DestructureExtract::Discard {
                pos: id.pos.clone(),
            },
            ty: Type::Unknown(TypeUnknown),
            tags: tags.clone(),
            targets: targets.clone(),
        }),
        SyntaxNode::Block(b) if b.tag == BracketTag::List => {
            let args = read_binary(env, b.pos.clone(), &b.items, OpTag::Sep)?;
            let mut extracts = Vec::new();
            let mut types = Vec::new();
            for arg in &args {
                if arg.items.is_empty() {
                    continue;
                }
                if let Some((extract, ty)) = read_comptime_param(env, arg, targets)? {
                    extracts.push(extract);
                    types.push(ty);
                    continue;
                }
                let sub = read_destructure(env, arg.pos.clone(), &arg.items, targets)?;
                extracts.push(sub.extract);
                types.push(sub.ty);
            }
            if ty.is_some() {
                return Err(throw_err(
                    env,
                    Some(b.pos.clone()),
                    "TODO support setting type on block in destructure",
                    None,
                    None,
                ));
            }
            Ok(Destructure {
                extract: DestructureExtract::List {
                    items: extracts,
                    pos: b.pos.clone(),
                },
                ty: Type::Tuple(TypeTuple { children: types }),
                tags: tags.clone(),
                targets: targets.clone(),
            })
        }
        _ => Err(throw_err(
            env,
            Some(syntax_node_pos(ident).clone()),
            format!(
                "Unsupported kind for destructuring: {}",
                syntax_node_kind(ident)
            ),
            None,
            None,
        )),
    }
}

fn analyze_destructure(
    env: &mut Env,
    destructure: &Destructure,
    body: AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<Vec<AnalysisResult>, PositionedError> {
    let mut targets: Vec<Option<AnalysisResult>> =
        (0..destructure.targets.len()).map(|_| None).collect();
    analyze_destructure_inner(env, &destructure.extract, body, block, &mut targets)?;
    assert!(targets.iter().all(|t| t.is_some()));
    Ok(targets
        .into_iter()
        .map(|t| t.expect("checked all Some above"))
        .collect())
}

fn analyze_destructure_inner(
    env: &mut Env,
    extract: &DestructureExtract,
    body: AnalysisResult,
    block: &mut AnalysisBlock,
    targets: &mut [Option<AnalysisResult>],
) -> Result<(), PositionedError> {
    match extract {
        DestructureExtract::SingleItem { target, .. } => {
            assert!(targets[*target].is_none());
            targets[*target] = Some(body);
            Ok(())
        }
        DestructureExtract::Discard { .. } => Ok(()),
        DestructureExtract::Comptime { pos, .. } => Err(throw_err(
            env,
            Some(pos.clone()),
            "a function with compile-time parameters can only be called, which gives them values",
            None,
            None,
        )),
        DestructureExtract::List { items, pos } => {
            let Type::Tuple(tuple) = &body.ty else {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!("cannot destructure a list from {}", body.ty.dump()),
                    None,
                    None,
                ));
            };
            if tuple.children.len() != items.len() {
                return Err(throw_err(
                    env,
                    Some(pos.clone()),
                    format!(
                        "expected {} items to destructure, found {}",
                        tuple.children.len(),
                        items.len()
                    ),
                    None,
                    None,
                ));
            }
            for (index, (item, child)) in items.iter().zip(&tuple.children).enumerate() {
                let idx = block_append(
                    block,
                    AnalysisLine::TupleGet {
                        pos: destructure_extract_pos(item).clone(),
                        tuple: body.value.clone(),
                        index,
                    },
                );
                analyze_destructure_inner(
                    env,
                    item,
                    AnalysisResult {
                        ty: child.clone(),
                        value: RuntimeValue::Runtime(idx),
                    },
                    block,
                    targets,
                )?;
            }
            Ok(())
        }
        _ => Err(throw_err(
            env,
            Some(destructure_extract_pos(extract).clone()),
            format!(
                "TODO support extract: {}",
                crate::printers::printers::DESTRUCTURE_EXTRACT.dump(extract, 3)
            ),
            None,
            None,
        )),
    }
}

pub struct ReadContainer {
    pub lines: Vec<OperatorSegmentToken>,
}

fn read_container_line(
    env: &mut Env,
    res: &mut ReadContainer,
    line_raw: &OperatorSegmentToken,
) -> Result<(), PositionedError> {
    let line_items = trim_ws(&line_raw.items);
    if line_items.is_empty() {
        return Ok(());
    }
    if let Some((lhs, op, rhs)) = read_binary2(env, &line_items, OpTag::Def)? {
        let mut targets = Vec::new();
        let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
        if !matches!(destructure.extract, DestructureExtract::SingleItem { .. }) {
            return Err(throw_err(
                env,
                Some(destructure_extract_pos(&destructure.extract).clone()),
                "TODO: support multiple destructure targets using analyze_destructure(), called when any of the names is resolved (but if two multiple names are resolved, called only once)",
                None,
                None,
            ));
        }
        for target in &destructure.targets {
            let prev_pos = env
                .scope
                .bindings
                .borrow()
                .get(&target.name)
                .map(|prev| binding_pos(prev).clone());
            if let Some(prev_pos) = prev_pos {
                let tok = add_err(
                    env,
                    Some(destructure_extract_pos(&destructure.extract).clone()),
                    format!("Duplicate binding name {}", target.name),
                    Some(vec![(
                        Some(prev_pos.clone()),
                        "Previous definition here".to_string(),
                    )]),
                );
                env.scope.bindings.borrow_mut().insert(
                    target.name.clone(),
                    Binding::Error {
                        pos: prev_pos,
                        consumed: tok,
                    },
                );
            } else {
                let scope = env.scope.clone();
                let decl = create_named_declaration(
                    ComptimeValueAst {
                        ast: rhs.items.clone(),
                        pos: rhs.pos.clone(),
                        scope,
                    },
                    &target.name,
                );
                env.scope.bindings.borrow_mut().insert(
                    target.name.clone(),
                    Binding::Valid {
                        pos: op.pos.clone(),
                        decl,
                    },
                );
            }
        }
    } else {
        res.lines.push(line_raw.clone());
    }
    Ok(())
}

pub fn read_container(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<ReadContainer, PositionedError> {
    let lines = read_binary(env, pos, src, OpTag::Sep)?;
    let mut res = ReadContainer { lines: Vec::new() };
    for line in &lines {
        if let Err(e) = read_container_line(env, &mut res, line) {
            handle_err(env, e);
        }
    }
    Ok(res)
}

pub fn analyze_block(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    src: &[SyntaxNode],
    block: &mut AnalysisBlock,
    mut analyze_bind: impl FnMut(
        &mut Env,
        Binary2,
        &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>,
) -> Result<AnalysisResult, PositionedError> {
    let saved_bindings = env.scope.bindings.clone();
    env.scope.bindings = Rc::new(RefCell::new(saved_bindings.borrow().clone()));
    let result = analyze_block_body(env, slot, pos, src, block, &mut analyze_bind);
    env.scope.bindings = saved_bindings;
    result
}

fn analyze_block_body(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    src: &[SyntaxNode],
    block: &mut AnalysisBlock,
    analyze_bind: &mut dyn FnMut(
        &mut Env,
        Binary2,
        &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError>,
) -> Result<AnalysisResult, PositionedError> {
    let container = read_container(env, pos, src)?;

    let mut ret: Option<AnalysisResult> = None;
    let mut retloc: Option<TokenPosition> = None;
    for line in &container.lines {
        if ret.is_some() {
            add_err(
                env,
                Some(line.pos.clone()),
                "extra lines not allowed after return",
                retloc
                    .clone()
                    .map(|p| vec![(Some(p), "returned here".to_string())]),
            );
            break;
        }
        let rb2 = read_binary2(env, &line.items, OpTag::Pub)?;
        if let Some(b2) = rb2 {
            analyze_bind(env, b2, block)?;
        } else {
            let rb3 = read_binary2(env, &line.items, OpTag::Var)?;
            if let Some((lhs, _op, rhs)) = rb3 {
                let mut targets = Vec::new();
                let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
                let rhs_analyzed = analyze(
                    env,
                    destructure.ty.clone(),
                    rhs.pos.clone(),
                    &rhs.items,
                    block,
                )?;
                let destructured = analyze_destructure(env, &destructure, rhs_analyzed, block)?;
                let mut new_bindings = env.scope.bindings.borrow().clone();
                for (target, value) in destructure.targets.iter().zip(destructured) {
                    new_bindings.insert(
                        target.name.clone(),
                        Binding::Runtime {
                            pos: target.pos.clone(),
                            runtime: value,
                        },
                    );
                }
                env.scope.bindings = Rc::new(RefCell::new(new_bindings));
            } else {
                if let Some((return_pos, returned)) = strip_return(&line.items) {
                    retloc = Some(return_pos);
                    ret = Some(analyze(
                        env,
                        slot.clone(),
                        line.pos.clone(),
                        &returned,
                        block,
                    )?);
                    continue;
                }
                let result = analyze(
                    env,
                    Type::Void(TypeVoid),
                    line.pos.clone(),
                    &line.items,
                    block,
                )?;
                if let Type::Never(_) = result.ty {
                    retloc = Some(line.pos.clone());
                    ret = Some(result);
                }
            }
        }
    }

    if let Some(ret) = ret {
        return Ok(ret);
    }
    Ok(AnalysisResult {
        ty: Type::Void(TypeVoid),
        value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
    })
}

#[derive(Debug)]
enum RegisteredEntry {
    Ok {
        decl: ComptimeValueDeclaration,
        pos: TokenPosition,
    },
    Error {
        etok: ConsumedErrorToken,
        pos: TokenPosition,
    },
}

#[derive(Debug)]
struct NamespaceImpl {
    pos: TokenPosition,
    registered: HashMap<NsKey, RegisteredEntry>,
}

impl ComptimeNamespace for NamespaceImpl {
    fn try_get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        if !self.registered.contains_key(&NsKey::Str(field.to_string())) {
            return Ok(None);
        }
        self.get_string(env, pos, field, block).map(Some)
    }

    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        match self.registered.get(&NsKey::Str(field.to_string())) {
            Some(RegisteredEntry::Error { etok, .. }) => return Err(throw_consumed_err(*etok)),
            Some(RegisteredEntry::Ok { decl, .. }) => {
                let r = get_declaration(env, decl.clone())?;
                return Ok(AnalysisResult {
                    ty: r.ty,
                    value: RuntimeValue::Comptime(r.value),
                });
            }
            None => {}
        }
        Err(throw_err(
            env,
            Some(pos.clone()),
            format!("string field '{field}' is not defined on namespace"),
            Some(vec![(Some(pos), "namespace declared here".to_string())]),
            None,
        ))
    }

    fn get_symbol(
        &self,
        env: &mut Env,
        _pos: TokenPosition,
        _keychild: Type,
        field: Symbol,
        _block: &mut AnalysisBlock,
    ) -> Result<Option<AnalysisResult>, PositionedError> {
        let Some(entry) = self.registered.get(&NsKey::Sym(field)) else {
            return Ok(None);
        };
        match entry {
            RegisteredEntry::Error { etok, .. } => Err(throw_consumed_err(*etok)),
            RegisteredEntry::Ok { decl, .. } => {
                let r = get_declaration(env, decl.clone())?;
                Ok(Some(AnalysisResult {
                    ty: r.ty,
                    value: RuntimeValue::Comptime(r.value),
                }))
            }
        }
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

pub fn analyze_namespace(
    env: &mut Env,
    pos: TokenPosition,
    src: &[SyntaxNode],
) -> Result<Rc<dyn ComptimeNamespace>, PositionedError> {
    let mut block = empty_block();
    let arr_entry = block_append(
        &mut block,
        AnalysisLine::ComptimeKvListInit { pos: pos.clone() },
    );

    analyze_block(
        env,
        Type::Void(TypeVoid),
        pos.clone(),
        src,
        &mut block,
        |env, b2, block| {
            let (lhs, op, rhs) = b2;
            let key = analyze(env, Type::CtKey(CtKey), lhs.pos.clone(), &lhs.items, block)?;
            let kval = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::Key),
                key.value,
                lhs.pos.clone(),
            )?;
            let value = analyze(env, Type::CtAst(CtAst), rhs.pos.clone(), &rhs.items, block)?;
            let ret = block_append(
                block,
                AnalysisLine::ComptimeKvListAppend {
                    pos: op.pos.clone(),
                    list: RuntimeValue::Runtime(arr_entry),
                    key: RuntimeValue::Comptime(kval),
                    value: value.value,
                },
            );
            Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Runtime(ret),
            })
        },
    )?;

    let evaluated =
        crate::comptime::comptime_eval(env, &block, RuntimeValue::Runtime(arr_entry), pos.clone())?;
    let arr_value = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::KvFields),
        RuntimeValue::Comptime(evaluated),
        pos.clone(),
    )?;
    let ComptimeValue::KvFields(mut arr_value) = arr_value else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    arr_value.locked = true;

    let mut registered: HashMap<NsKey, RegisteredEntry> = HashMap::new();
    for entry in arr_value.entries {
        let key = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::Key),
            RuntimeValue::Comptime(entry.key),
            entry.pos.clone(),
        )?;
        let ComptimeValue::Key(key_val) = key else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let value = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::Ast),
            RuntimeValue::Comptime(entry.value),
            entry.pos.clone(),
        )?;
        let ComptimeValue::Ast(value_val) = value else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        let ns_key = match &key_val {
            ComptimeValueKey::String { key } => NsKey::Str(key.clone()),
            ComptimeValueKey::Symbol { key, .. } => NsKey::Sym(*key),
        };
        if let Some(prev) = registered.get(&ns_key) {
            let prev_pos = match prev {
                RegisteredEntry::Ok { pos, .. } => pos.clone(),
                RegisteredEntry::Error { pos, .. } => pos.clone(),
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
            registered.insert(
                ns_key,
                RegisteredEntry::Error {
                    etok,
                    pos: prev_pos,
                },
            );
            continue;
        }
        registered.insert(
            ns_key,
            RegisteredEntry::Ok {
                decl: create_declaration(env, value_val),
                pos: entry.pos.clone(),
            },
        );
    }

    Ok(Rc::new(NamespaceImpl { pos, registered }))
}

thread_local! {
    static POSTED_RETURNS: RefCell<HashMap<(usize, ComptimeSnapshot), Type>> =
        RefCell::new(HashMap::new());
}

pub fn post_return(key: &(usize, ComptimeSnapshot), ty: Type) {
    POSTED_RETURNS.with(|posted| {
        posted.borrow_mut().entry(key.clone()).or_insert(ty);
    });
}

pub fn posted_return(fn_value: &ComptimeValueFn, comptime: &ComptimeScopeMap) -> Option<Type> {
    POSTED_RETURNS.with(|posted| {
        posted
            .borrow()
            .get(&(fn_value.cache_ptr(), comptime.snapshot()))
            .cloned()
    })
}

pub fn analyze_function(
    env: &mut Env,
    fn_value: &ComptimeValueFn,
) -> Result<AnalyzedFn, PositionedError> {
    let saved_bindings = std::mem::replace(
        &mut env.scope.bindings,
        fn_value.body().scope.bindings.clone(),
    );
    let cache = env.fn_cache.clone();
    let comptime = env.scope.comptime.clone();
    let result = cache.get_or_put(fn_value, &comptime, env, |env| {
        let mut block = empty_block();
        let args_idx = block_append(
            &mut block,
            AnalysisLine::Args {
                pos: destructure_extract_pos(&fn_value.args().extract).clone(),
            },
        );
        let fn_bindings = env.scope.bindings.borrow().clone();
        env.scope.bindings = Rc::new(RefCell::new(fn_bindings));
        let args = fn_value.args();
        let destructured = analyze_destructure(
            env,
            args,
            AnalysisResult {
                ty: args.ty.clone(),
                value: RuntimeValue::Runtime(args_idx),
            },
            &mut block,
        )?;
        for (target, value) in args.targets.iter().zip(destructured) {
            env.scope.bindings.borrow_mut().insert(
                target.name.clone(),
                Binding::Runtime {
                    pos: target.pos.clone(),
                    runtime: value,
                },
            );
        }
        let key = (fn_value.cache_ptr(), env.scope.comptime.snapshot());
        let result = analyze(
            env,
            Type::Infer(crate::ct::TypeInfer { key: key.clone() }),
            fn_value.pos().clone(),
            &fn_value.body().ast,
            &mut block,
        );
        POSTED_RETURNS.with(|p| p.borrow_mut().remove(&key));
        let result = result?;
        Ok(AnalyzedFn {
            block,
            ty: result.ty,
            value: result.value,
        })
    });
    env.scope.bindings = saved_bindings;
    result
}

pub fn cast_or_analyze() {}

pub fn analyze(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    ast: &[SyntaxNode],
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let Type::CtAst(_) = &slot {
        let value = ComptimeValueAst {
            ast: ast.to_vec(),
            pos,
            scope: env.scope.clone(),
        };
        return Ok(AnalysisResult {
            ty: Type::CtAst(CtAst),
            value: RuntimeValue::Comptime(ComptimeValue::Ast(value)),
        });
    }
    let ast = trim_ws(ast);

    if ast.is_empty() {
        return Err(throw_err(
            env,
            Some(pos),
            "failed to analyze empty expression",
            None,
            None,
        ));
    }
    if let SyntaxNode::Raw(r) = &ast[0]
        && r.tag == RawTag::Return
    {
        return Err(throw_err(
            env,
            Some(r.pos.clone()),
            "can't return here",
            None,
            None,
        ));
    }

    let last = ast.len() - 1;
    analyze_sub(env, slot.clone(), slot, &ast, last, block)
}

pub fn analyze_sub(
    env: &mut Env,
    slot: Type,
    root_slot: Type,
    ast: &[SyntaxNode],
    index: usize,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let expr = &ast[index];

    if let SyntaxNode::Identifier(id) = expr {
        if id.ident_tag == IdentifierTag::Access {
            let lhs = if index >= 1 {
                analyze_sub(
                    env,
                    Type::Unknown(TypeUnknown),
                    root_slot.clone(),
                    ast,
                    index - 1,
                    block,
                )?
            } else {
                AnalysisResult {
                    ty: Type::CtType(CtType),
                    value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
                        ty: root_slot.clone(),
                    })),
                }
            };
            let prop = AnalysisResult {
                ty: Type::CtKey(CtKey),
                value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::String {
                    key: id.str.clone(),
                })),
            };
            return analyze_access(env, slot, lhs, id.pos.clone(), prop, block);
        }
    } else if let SyntaxNode::Block(b) = expr {
        if b.tag == BracketTag::ArrowFn {
            let sts = slot.implicit_arg_ret_for_arrow_fn();
            let mut targets = Vec::new();
            let args = read_destructure(env, b.pos.clone(), &ast[..index], &mut targets)?;
            let ret_ty = Type::Fn(TypeFn {
                pos: b.pos.clone(),
                arg: Box::new(args.ty.clone()),
                ret: Box::new(sts.ret),
            });
            let fn_value = ComptimeValueFn::new(
                args,
                ComptimeValueAst {
                    ast: b.items.clone(),
                    pos: b.pos.clone(),
                    scope: env.scope.clone(),
                },
                b.pos.clone(),
            );
            return Ok(AnalysisResult {
                ty: ret_ty,
                value: RuntimeValue::Comptime(ComptimeValue::Fn(fn_value)),
            });
        } else if b.tag == BracketTag::SymbolAccess {
            let lhs = if index >= 1 {
                analyze_sub(
                    env,
                    Type::Unknown(TypeUnknown),
                    root_slot.clone(),
                    ast,
                    index - 1,
                    block,
                )?
            } else {
                AnalysisResult {
                    ty: Type::CtType(CtType),
                    value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
                        ty: root_slot.clone(),
                    })),
                }
            };
            let key = analyze(env, Type::CtKey(CtKey), b.pos.clone(), &b.items, block)?;
            return analyze_access(env, slot, lhs, b.pos.clone(), key, block);
        } else if b.tag == BracketTag::ColonCall {
            if index == 0 {
                return analyze_label(env, slot, b, block);
            }
            let lhs = analyze_sub(
                env,
                Type::Unknown(TypeUnknown),
                root_slot,
                ast,
                index - 1,
                block,
            )?;
            return analyze_call(
                env,
                slot,
                b.pos.clone(),
                lhs,
                CallArg {
                    pos: b.pos.clone(),
                    ast: &b.items,
                },
                block,
            );
        }
    }

    if let SyntaxNode::Block(b) = expr
        && matches!(b.tag, BracketTag::List | BracketTag::Code | BracketTag::Map)
        && index > 0
    {
        let lhs = analyze_sub(
            env,
            Type::Unknown(TypeUnknown),
            root_slot,
            ast,
            index - 1,
            block,
        )?;
        return analyze_call(
            env,
            slot,
            b.pos.clone(),
            lhs,
            CallArg {
                pos: b.pos.clone(),
                ast: std::slice::from_ref(expr),
            },
            block,
        );
    }

    if index == 0 {
        analyze_base(env, slot, expr, block)
    } else {
        Err(throw_err(
            env,
            Some(syntax_node_pos(expr).clone()),
            format!(
                "TODO analyzeSuffix: {}{}",
                syntax_node_kind(expr),
                AST_NODE.dump_list(std::slice::from_ref(expr), 2)
            ),
            None,
            None,
        ))
    }
}

fn assigns_to_discard(bin: &BinaryExpressionToken) -> bool {
    let Some(SyntaxNode::OperatorSegment(lhs)) = bin.items.first() else {
        return false;
    };
    matches!(
        trim_ws(&lhs.items).as_slice(),
        [SyntaxNode::Identifier(id)] if id.ident_tag == IdentifierTag::Discard
    )
}

fn analyze_label(
    env: &mut Env,
    slot: Type,
    b: &BlockToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = trim_ws(&b.items);
    let Some(SyntaxNode::Identifier(label)) = items.first() else {
        return Err(throw_err(
            env,
            Some(b.pos.clone()),
            "expected a label name after ':'",
            None,
            None,
        ));
    };
    if label.ident_tag != IdentifierTag::Normal {
        return Err(throw_err(
            env,
            Some(label.pos.clone()),
            "expected a label name after ':'",
            None,
            None,
        ));
    }
    if items.len() < 2 {
        return Err(throw_err(
            env,
            Some(label.pos.clone()),
            "expected a body after the label",
            None,
            None,
        ));
    }
    if let Type::Unknown(_) | Type::Infer(_) = slot {
        return Err(throw_err(
            env,
            Some(label.pos.clone()),
            "a labelled block needs a known result type",
            None,
            None,
        ));
    }
    let symbol = Symbol::new();
    block_append(
        block,
        AnalysisLine::LabelBegin {
            pos: label.pos.clone(),
            label: symbol,
            ty: slot.clone(),
        },
    );
    let saved_bindings = env.scope.bindings.clone();
    let mut inner_bindings = saved_bindings.borrow().clone();
    inner_bindings.insert(
        label.str.clone(),
        Binding::Runtime {
            pos: label.pos.clone(),
            runtime: AnalysisResult {
                ty: Type::Label(TypeLabel {
                    label: symbol,
                    ty: Box::new(slot.clone()),
                }),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            },
        },
    );
    env.scope.bindings = Rc::new(RefCell::new(inner_bindings));
    let body = analyze(env, slot.clone(), b.pos.clone(), &items[1..], block);
    env.scope.bindings = saved_bindings;
    let body = slot.cast_into(env, block, body?, b.pos.clone())?;
    let end = block_append(
        block,
        AnalysisLine::LabelEnd {
            pos: b.pos.clone(),
            label: symbol,
            value: body.value,
        },
    );
    Ok(AnalysisResult {
        ty: slot,
        value: RuntimeValue::Runtime(end),
    })
}

fn analyze_binary_op(
    env: &mut Env,
    slot: Type,
    bin: &BinaryExpressionToken,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    for item in &bin.items {
        if !matches!(
            item,
            SyntaxNode::OperatorSegment(_) | SyntaxNode::Operator(_) | SyntaxNode::Whitespace(_)
        ) {
            return Err(throw_err(
                env,
                Some(syntax_node_pos(item).clone()),
                format!(
                    "unexpected {} in operator expression",
                    syntax_node_kind(item)
                ),
                None,
                None,
            ));
        }
    }
    let Some(op_index) = bin
        .items
        .iter()
        .rposition(|item| matches!(item, SyntaxNode::Operator(_)))
    else {
        return Err(throw_err(
            env,
            Some(bin.pos.clone()),
            "expected an operator",
            None,
            None,
        ));
    };
    let SyntaxNode::Operator(op) = &bin.items[op_index] else {
        unreachable!("rposition matched an operator")
    };
    let lhs_segments = bin.items[..op_index]
        .iter()
        .filter(|item| matches!(item, SyntaxNode::OperatorSegment(_)))
        .count();
    let (Some(SyntaxNode::OperatorSegment(lhs_first)), Some(SyntaxNode::OperatorSegment(rhs))) = (
        bin.items.first(),
        bin.items[op_index + 1..]
            .iter()
            .find(|item| matches!(item, SyntaxNode::OperatorSegment(_))),
    ) else {
        return Err(throw_err(
            env,
            Some(op.pos.clone()),
            "expected an operand on each side of the operator",
            None,
            None,
        ));
    };
    let lhs = if lhs_segments == 1 {
        lhs_first.clone()
    } else {
        OperatorSegmentToken {
            pos: lhs_first.pos.clone(),
            items: vec![SyntaxNode::BinaryExpression(Box::new(
                BinaryExpressionToken {
                    pos: bin.pos.clone(),
                    prec: bin.prec,
                    tag: bin.tag,
                    items: bin.items[..op_index].to_vec(),
                },
            ))],
        }
    };

    let slot_key = crate::std_keys::operator_symbol(crate::std_keys::OperatorKind::Slot, &op.op);
    if let Some(slot_op) = slot.type_symbol(env, slot_key)? {
        let args = SyntaxNode::Block(Box::new(BlockToken {
            pos: op.pos.clone(),
            start: "(".to_string(),
            end: ")".to_string(),
            tag: BracketTag::List,
            items: vec![SyntaxNode::BinaryExpression(Box::new(
                BinaryExpressionToken {
                    pos: op.pos.clone(),
                    prec: 0,
                    tag: OpTag::Sep,
                    items: vec![
                        SyntaxNode::OperatorSegment(lhs),
                        SyntaxNode::Operator(OperatorToken {
                            pos: op.pos.clone(),
                            op: ",".to_string(),
                            op_tag: OpTag::Sep,
                        }),
                        SyntaxNode::OperatorSegment(rhs.clone()),
                    ],
                },
            ))],
        }));
        return analyze_call(
            env,
            slot,
            op.pos.clone(),
            slot_op,
            CallArg {
                pos: op.pos.clone(),
                ast: std::slice::from_ref(&args),
            },
            block,
        );
    }

    let lhs = analyze(
        env,
        Type::Unknown(TypeUnknown),
        lhs.pos.clone(),
        &lhs.items,
        block,
    )?;
    let lhs_key = crate::std_keys::operator_symbol(crate::std_keys::OperatorKind::Lhs, &op.op);
    if !lhs.ty.has_value_symbol(env, lhs_key)? {
        return Err(throw_err(
            env,
            Some(op.pos.clone()),
            format!(
                "operator {op} is not supported: {} has no std.operator.slot(\"{op}\") and {} has no std.operator.lhs(\"{op}\")",
                slot.dump(),
                lhs.ty.dump(),
                op = op.op,
            ),
            None,
            None,
        ));
    }
    let bound = AnalysisResult {
        ty: Type::Bound(TypeBound {
            receiver: Box::new(lhs.ty),
            key: lhs_key,
        }),
        value: lhs.value,
    };
    analyze_call(
        env,
        slot,
        op.pos.clone(),
        bound,
        CallArg {
            pos: rhs.pos.clone(),
            ast: &rhs.items,
        },
        block,
    )
}

pub fn analyze_call(
    env: &mut Env,
    slot: Type,
    pos: TokenPosition,
    method: AnalysisResult,
    arg_in: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let ty = method.ty.clone();
    ty.analyze_call(env, slot, pos, method, arg_in, block)
}

pub fn analyze_access(
    env: &mut Env,
    slot: Type,
    obj: AnalysisResult,
    pos: TokenPosition,
    prop: AnalysisResult,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let ty = obj.ty.clone();
    ty.analyze_access(env, slot, obj, pos, prop, block)
}

pub fn analyze_base(
    env: &mut Env,
    slot: Type,
    ast: &SyntaxNode,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    match ast {
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Builtin => {
            if id.str == "builtin" {
                descriptor_construct(&builtin_namespace_descriptor(), env, "#builtin")
            } else {
                Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("unexpected builtin: #{}", id.str),
                    None,
                    None,
                ))
            }
        }
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Normal => {
            let Some(value) = env.scope.bindings.borrow().get(&id.str).cloned() else {
                return Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("not defined in scope: {}", id.str),
                    None,
                    None,
                ));
            };
            match value {
                Binding::Error { consumed, .. } => Err(throw_consumed_err(consumed)),
                Binding::Removed { pos } => Err(throw_err(
                    env,
                    Some(id.pos.clone()),
                    format!("not defined in scope: {}", id.str),
                    Some(vec![(Some(pos), "removed here".to_string())]),
                    None,
                )),
                Binding::Valid { decl, .. } => {
                    let r = get_declaration(env, decl)?;
                    Ok(AnalysisResult {
                        ty: r.ty,
                        value: RuntimeValue::Comptime(r.value),
                    })
                }
                Binding::Runtime { runtime, .. } => {
                    if let RuntimeValue::Runtime(idx) = &runtime.value
                        && idx.1 != block.validate
                    {
                        return Err(throw_err(
                            env,
                            Some(id.pos.clone()),
                            "not accessible",
                            None,
                            None,
                        ));
                    }
                    Ok(runtime)
                }
            }
        }
        SyntaxNode::Block(b) if b.tag == BracketTag::String => crate::ct::analyze_literal(
            env,
            slot,
            crate::std_keys::LiteralKind::String,
            b.pos.clone(),
            ast,
            block,
        ),
        SyntaxNode::Raw(r) if r.tag == RawTag::Void => Ok(AnalysisResult {
            ty: Type::Void(TypeVoid),
            value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
        }),
        SyntaxNode::Block(b) if b.tag == BracketTag::Map => crate::ct::analyze_literal(
            env,
            slot,
            crate::std_keys::LiteralKind::Map,
            b.pos.clone(),
            ast,
            block,
        ),
        SyntaxNode::Block(b) if b.tag == BracketTag::List => crate::ct::analyze_literal(
            env,
            slot,
            crate::std_keys::LiteralKind::List,
            b.pos.clone(),
            ast,
            block,
        ),
        SyntaxNode::BinaryExpression(be)
            if matches!(be.tag, OpTag::Compare | OpTag::Add | OpTag::Mul) =>
        {
            analyze_binary_op(env, slot, be, block)
        }
        SyntaxNode::Block(b) if b.tag == BracketTag::Code => analyze_block(
            env,
            slot,
            b.pos.clone(),
            &b.items,
            block,
            |env, b2, _block| {
                let (_lhs, op, _rhs) = b2;
                Err(throw_err(
                    env,
                    Some(op.pos.clone()),
                    "TODO: implement bind in block",
                    None,
                    None,
                ))
            },
        ),
        SyntaxNode::Identifier(id) if id.ident_tag == IdentifierTag::Number => {
            crate::ct::analyze_literal(
                env,
                slot,
                crate::std_keys::LiteralKind::Number,
                id.pos.clone(),
                ast,
                block,
            )
        }
        SyntaxNode::BinaryExpression(be) if be.tag == OpTag::Assign && !assigns_to_discard(be) => {
            analyze_binary_op(env, slot, be, block)
        }
        SyntaxNode::BinaryExpression(be) if be.tag == OpTag::Assign => {
            let rbr = read_binary2(env, std::slice::from_ref(ast), OpTag::Assign)?;
            let Some((lhs, _op, rhs)) = rbr else {
                return Err(throw_err(
                    env,
                    Some(be.pos.clone()),
                    format!(
                        "Expected X = Y, got X = Y = Z? {}",
                        AST_NODE.dump_list(&be.items, crate::printers::UNLIMITED_DEPTH)
                    ),
                    None,
                    None,
                ));
            };
            let mut targets = Vec::new();
            let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
            let body = analyze(
                env,
                destructure.ty.clone(),
                rhs.pos.clone(),
                &rhs.items,
                block,
            )?;
            let bindings = analyze_destructure(env, &destructure, body, block)?;
            if !bindings.is_empty() {
                return Err(throw_err(
                    env,
                    Some(lhs.pos.clone()),
                    "TODO implement assignment operator",
                    None,
                    None,
                ));
            }
            Ok(AnalysisResult {
                ty: Type::Void(TypeVoid),
                value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
            })
        }
        _ => Err(throw_err(
            env,
            Some(syntax_node_pos(ast).clone()),
            format!(
                "TODO analyzeBase: {}{}",
                syntax_node_kind(ast),
                AST_NODE.dump_list(std::slice::from_ref(ast), 3)
            ),
            None,
            None,
        )),
    }
}

pub trait Descriptor: std::fmt::Debug {
    fn construct_impl(&self, env: &mut Env, route: &str)
    -> Result<AnalysisResult, PositionedError>;
}

fn descriptor_construct(
    d: &Rc<dyn Descriptor>,
    env: &mut Env,
    route: &str,
) -> Result<AnalysisResult, PositionedError> {
    let cache = env.builtin_cache.clone();
    let d2 = d.clone();
    let route2 = route.to_string();
    let comptime = ComptimeScopeMap::root(HashMap::new());
    cache.get_or_put(d, &comptime, env, move |env| {
        d2.construct_impl(env, &route2)
    })
}

impl CacheKey for Rc<dyn Descriptor> {
    fn cache_ptr(&self) -> usize {
        Rc::as_ptr(self) as *const () as usize
    }
}

#[derive(Debug)]
pub struct NamespaceDescriptor {
    pub entries: Vec<(String, Rc<dyn Descriptor>)>,
    pub call: Option<BuiltinFn>,
    pub pos: TokenPosition,
}

impl Descriptor for NamespaceDescriptor {
    fn construct_impl(
        &self,
        env: &mut Env,
        route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        let mut results: HashMap<String, AnalysisResult> = HashMap::new();
        for (key, value) in &self.entries {
            let sub_route = format!("{route}.{key}");
            results.insert(key.clone(), descriptor_construct(value, env, &sub_route)?);
        }
        let ns = BuiltinNamespaceImpl {
            results,
            call: self.call,
            pos: self.pos.clone(),
            route: route.to_string(),
        };
        Ok(AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new(ns))),
        })
    }
}

#[derive(Debug)]
struct BuiltinNamespaceImpl {
    results: HashMap<String, AnalysisResult>,
    call: Option<BuiltinFn>,
    pos: TokenPosition,
    route: String,
}

impl ComptimeNamespace for BuiltinNamespaceImpl {
    fn get_string(
        &self,
        env: &mut Env,
        pos: TokenPosition,
        field: &str,
        _block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        if let Some(v) = self.results.get(field) {
            return Ok(v.clone());
        }
        Err(throw_err(
            env,
            Some(pos),
            format!("namespace {} does not have field: {field}", self.route),
            Some(vec![(
                Some(self.pos.clone()),
                "namespace defined here".to_string(),
            )]),
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
        slot: Type,
        pos: TokenPosition,
        arg: CallArg<'_>,
        block: &mut AnalysisBlock,
    ) -> Result<AnalysisResult, PositionedError> {
        let Some(call) = self.call else {
            return Err(throw_err(
                env,
                Some(pos),
                format!("namespace {} does not support call", self.route),
                Some(vec![(Some(self.pos.clone()), "defined here".to_string())]),
                None,
            ));
        };
        call(env, slot, pos, arg, block)
    }

    fn pos(&self) -> &TokenPosition {
        &self.pos
    }
}

#[derive(Debug)]
struct PreludeDescriptor(fn() -> crate::user_type::LazyPrelude);

impl Descriptor for PreludeDescriptor {
    fn construct_impl(
        &self,
        _env: &mut Env,
        _route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        Ok(AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(Rc::new((self.0)()))),
        })
    }
}

#[derive(Debug)]
struct ReflectPreludeDescriptor;

impl Descriptor for ReflectPreludeDescriptor {
    fn construct_impl(
        &self,
        _env: &mut Env,
        _route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        Ok(AnalysisResult {
            ty: Type::CtNamespace(CtNamespace),
            value: RuntimeValue::Comptime(ComptimeValue::Namespace(
                crate::user_type::LazyPrelude::reflect(),
            )),
        })
    }
}

#[derive(Debug)]
pub struct CustomDescriptor(pub AnalysisResult);

impl Descriptor for CustomDescriptor {
    fn construct_impl(
        &self,
        _env: &mut Env,
        _route: &str,
    ) -> Result<AnalysisResult, PositionedError> {
        Ok(self.0.clone())
    }
}

fn d_raw(result: AnalysisResult) -> Rc<dyn Descriptor> {
    Rc::new(CustomDescriptor(result))
}

fn d_type(ty: Type) -> Rc<dyn Descriptor> {
    d_raw(AnalysisResult {
        ty: Type::CtType(CtType),
        value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType { ty })),
    })
}

fn d_std_key(key: crate::std_keys::StdKey) -> Rc<dyn Descriptor> {
    d_raw(AnalysisResult {
        ty: Type::CtKey(CtKey),
        value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::Symbol {
            key: crate::std_keys::std_key(key),
            child: Type::Unknown(TypeUnknown),
        })),
    })
}

fn d_ns(entries: Vec<(&str, Rc<dyn Descriptor>)>, call: Option<BuiltinFn>) -> Rc<dyn Descriptor> {
    Rc::new(NamespaceDescriptor {
        entries: entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        call,
        pos: compiler_pos(),
    })
}

fn std_folder_or_file_type() -> Type {
    Type::CtBuildArtifact(CtBuildArtifact { narrow: None })
}

fn build_symbol() -> Symbol {
    static BUILD_SYMBOL: OnceLock<Symbol> = OnceLock::new();
    *BUILD_SYMBOL.get_or_init(Symbol::new)
}

fn build_symbol_child_type() -> Type {
    Type::Fn(TypeFn {
        pos: compiler_pos(),
        arg: Box::new(Type::Void(TypeVoid)),
        ret: Box::new(std_folder_or_file_type()),
    })
}

fn build_symbol_value() -> ComptimeValueKey {
    ComptimeValueKey::Symbol {
        key: build_symbol(),
        child: build_symbol_child_type(),
    }
}

fn builtin_operator_key(
    env: &mut Env,
    kind: crate::std_keys::OperatorKind,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let pos = arg_ast.pos.clone();
    let name = analyze(
        env,
        Type::OperatorName(OperatorName),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    let name = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::OperatorName),
        name.value,
        pos,
    )?;
    let ComptimeValue::OperatorName(name) = name else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::CtKey(CtKey),
        value: RuntimeValue::Comptime(ComptimeValue::Key(ComptimeValueKey::Symbol {
            key: crate::std_keys::operator_symbol(kind, &name.value),
            child: Type::Unknown(TypeUnknown),
        })),
    })
}

fn builtin_operator_slot_call(
    env: &mut Env,
    _slot: Type,
    _pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    builtin_operator_key(env, crate::std_keys::OperatorKind::Slot, arg_ast, block)
}

fn builtin_operator_lhs_call(
    env: &mut Env,
    _slot: Type,
    _pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    builtin_operator_key(env, crate::std_keys::OperatorKind::Lhs, arg_ast, block)
}

pub(crate) fn builtin_list_args<'a>(
    env: &mut Env,
    pos: &TokenPosition,
    arg_ast: &CallArg<'a>,
    what: &str,
    count: usize,
) -> Result<Vec<OperatorSegmentToken>, PositionedError> {
    match crate::ct::call_list_items(env, arg_ast)? {
        Some(items) if items.len() == count => Ok(items),
        _ => Err(throw_err(
            env,
            Some(pos.clone()),
            format!("{what} takes {count} arguments in parentheses"),
            None,
            None,
        )),
    }
}

fn builtin_type_wrap_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = builtin_list_args(env, &pos, &arg_ast, "std.type.wrap", 2)?;
    let ty = analyze(
        env,
        Type::CtType(CtType),
        items[0].pos.clone(),
        &items[0].items,
        block,
    )?;
    let ComptimeValue::Type(ty) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::Type),
        ty.value,
        items[0].pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    let Type::User(user) = &ty.ty else {
        return Err(throw_err(
            env,
            Some(items[0].pos.clone()),
            format!("std.type.wrap needs a std.Type, got {}", ty.ty.dump()),
            None,
            None,
        ));
    };
    let repr = user.repr(env, &pos)?;
    let value = analyze(
        env,
        repr.clone(),
        items[1].pos.clone(),
        &items[1].items,
        block,
    )?;
    let value = repr.cast_into(env, block, value, items[1].pos.clone())?;
    Ok(AnalysisResult {
        ty: ty.ty,
        value: value.value,
    })
}

fn builtin_type_unwrap_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let value = analyze(
        env,
        Type::Unknown(TypeUnknown),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    let Type::User(user) = &value.ty else {
        return Err(throw_err(
            env,
            Some(pos),
            format!(
                "std.type.unwrap needs a value of a std.Type, got {}",
                value.ty.dump()
            ),
            None,
            None,
        ));
    };
    Ok(AnalysisResult {
        ty: user.repr(env, &pos)?,
        value: value.value,
    })
}

fn builtin_c_binary_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let name = analyze(
        env,
        Type::OperatorName(OperatorName),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    let ComptimeValue::OperatorName(name) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::OperatorName),
        name.value,
        pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    let Some(op) = crate::backend::c::CBinaryOp::from_token(&name.value) else {
        return Err(throw_err(
            env,
            Some(pos),
            format!("C has no operator {}", name.value),
            None,
            None,
        ));
    };
    Ok(crate::ct::slot_operator_value(Type::CInt(CInt), op))
}

fn builtin_c_int_from_kw_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let kw_int = Type::KwInt(crate::ct::KwInt);
    let value = analyze(env, kw_int.clone(), arg_ast.pos, arg_ast.ast, block)?;
    let value = kw_int.cast_into(env, block, value, pos.clone())?;
    let ComptimeValue::KwInt(value) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::KwInt),
        value.value,
        pos.clone(),
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    let Ok(value) = i32::try_from(value.value) else {
        return Err(throw_err(
            env,
            Some(pos),
            format!("{} does not fit in a C int", value.value),
            None,
            None,
        ));
    };
    Ok(AnalysisResult {
        ty: Type::CInt(CInt),
        value: RuntimeValue::Comptime(ComptimeValue::CInt(ComptimeValueCInt { value })),
    })
}

fn builtin_kw_map_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let items = builtin_list_args(env, &pos, &arg_ast, "std.kw.map", 2)?;
    let mut types = Vec::new();
    for item in &items {
        let ty = analyze(
            env,
            Type::CtType(CtType),
            item.pos.clone(),
            &item.items,
            block,
        )?;
        let ComptimeValue::Type(ty) = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::Type),
            ty.value,
            item.pos.clone(),
        )?
        else {
            unreachable!("get_comptime guarantees a matching kind")
        };
        types.push(ty.ty);
    }
    let value = types.pop().expect("two arguments");
    let key = types.pop().expect("two arguments");
    Ok(AnalysisResult {
        ty: Type::CtType(CtType),
        value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
            ty: Type::KwMap(crate::ct::KwMap {
                key: Box::new(key),
                value: Box::new(value),
            }),
        })),
    })
}

fn builtin_kw_list_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let elem = analyze(env, Type::CtType(CtType), arg_ast.pos, arg_ast.ast, block)?;
    let ComptimeValue::Type(elem) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::Type),
        elem.value,
        pos,
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::CtType(CtType),
        value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
            ty: Type::KwList(crate::ct::KwList {
                elem: Box::new(elem.ty),
            }),
        })),
    })
}

fn builtin_declare_call(
    env: &mut Env,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    kind: crate::user_type::DeclKind,
    what: &str,
) -> Result<AnalysisResult, PositionedError> {
    let items = trim_ws(arg_ast.ast);
    let [SyntaxNode::Block(map)] = items.as_slice() else {
        return Err(throw_err(
            env,
            Some(pos),
            format!("{what} takes a [ ... ] map"),
            None,
            None,
        ));
    };
    if map.tag != BracketTag::Map {
        return Err(throw_err(
            env,
            Some(pos),
            format!("{what} takes a [ ... ] map"),
            None,
            None,
        ));
    }
    Ok(crate::user_type::declare(env, map, kind))
}

fn builtin_struct_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    _block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    builtin_declare_call(
        env,
        pos,
        arg_ast,
        crate::user_type::DeclKind::Struct,
        "std.Struct",
    )
}

fn builtin_enum_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    _block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    builtin_declare_call(
        env,
        pos,
        arg_ast,
        crate::user_type::DeclKind::Enum,
        "std.Enum",
    )
}

fn builtin_option_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let child = analyze(env, Type::CtType(CtType), arg_ast.pos, arg_ast.ast, block)?;
    let ComptimeValue::Type(child) = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::Type),
        child.value,
        pos,
    )?
    else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(AnalysisResult {
        ty: Type::CtType(CtType),
        value: RuntimeValue::Comptime(ComptimeValue::Type(ComptimeValueType {
            ty: Type::Optional(crate::ct::TypeOptional {
                child: Box::new(child.ty),
            }),
        })),
    })
}

fn builtin_kw_mut_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let init = analyze(
        env,
        Type::Unknown(TypeUnknown),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    if let Type::Unknown(_) = init.ty {
        return Err(throw_err(
            env,
            Some(pos),
            "std.kw.mut needs a value of a known type",
            None,
            None,
        ));
    }
    let cell = block_append(
        block,
        AnalysisLine::MutNew {
            pos,
            init: init.value,
        },
    );
    Ok(AnalysisResult {
        ty: Type::KwMut(crate::ct::KwMut {
            inner: Box::new(init.ty),
        }),
        value: RuntimeValue::Runtime(cell),
    })
}

fn builtin_kw_loop_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    block_append(
        block,
        AnalysisLine::RegionBegin {
            pos: pos.clone(),
            region: Region::KwLoop,
        },
    );
    analyze(env, Type::Void(TypeVoid), arg_ast.pos, arg_ast.ast, block)?;
    block_append(block, AnalysisLine::RegionEnd { pos });
    Ok(AnalysisResult {
        ty: Type::Never(crate::ct::TypeNever),
        value: RuntimeValue::Comptime(ComptimeValue::Void(ComptimeValueVoid)),
    })
}

fn kw_if_binding(
    env: &mut Env,
    arg_ast: &CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<Option<AnalysisResult>, PositionedError> {
    let Some(items) = crate::ct::call_list_items(env, arg_ast)? else {
        return Ok(None);
    };
    let [item] = items.as_slice() else {
        return Ok(None);
    };
    let Some((lhs, op, rhs)) = read_binary2(env, &item.items, OpTag::Var)? else {
        return Ok(None);
    };
    let mut targets = Vec::new();
    let destructure = read_destructure(env, lhs.pos.clone(), &lhs.items, &mut targets)?;
    let bind = match &destructure.extract {
        DestructureExtract::SingleItem { target, pos } => Some(Box::new(crate::ct::KwIfBinding {
            name: targets[*target].name.clone(),
            pos: pos.clone(),
            ty: destructure.ty.clone(),
        })),
        DestructureExtract::Discard { .. } => None,
        other => {
            return Err(throw_err(
                env,
                Some(destructure_extract_pos(other).clone()),
                "std.kw.if (v := opt) binds a single name",
                None,
                None,
            ));
        }
    };
    let value = analyze(
        env,
        Type::Unknown(TypeUnknown),
        rhs.pos.clone(),
        &rhs.items,
        block,
    )?;
    let Type::Optional(optional) = value.ty else {
        return Err(throw_err(
            env,
            Some(op.pos.clone()),
            format!(
                "std.kw.if (v := x) needs an optional, got {}",
                value.ty.dump()
            ),
            None,
            None,
        ));
    };
    Ok(Some(AnalysisResult {
        ty: Type::KwIfOptional(crate::ct::TypeKwIfOptional {
            child: optional.child,
            bind,
        }),
        value: value.value,
    }))
}

fn builtin_kw_if_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    if let Some(binding) = kw_if_binding(env, &arg_ast, block)? {
        return Ok(binding);
    }
    let bool_ty = Type::KwBool(crate::ct::KwBool);
    let cond = analyze(env, bool_ty.clone(), arg_ast.pos, arg_ast.ast, block)?;
    if let Type::Optional(optional) = cond.ty {
        return Ok(AnalysisResult {
            ty: Type::KwIfOptional(crate::ct::TypeKwIfOptional {
                bind: None,
                child: optional.child,
            }),
            value: cond.value,
        });
    }
    let cond = bool_ty.cast_into(env, block, cond, pos)?;
    Ok(AnalysisResult {
        ty: Type::KwIf(crate::ct::KwIf {
            region: crate::ct::KwIfRegion::If,
        }),
        value: cond.value,
    })
}

fn builtin_c_if_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    let int = Type::CInt(CInt);
    let cond = analyze(
        env,
        Type::Unknown(TypeUnknown),
        arg_ast.pos,
        arg_ast.ast,
        block,
    )?;
    let cond = crate::user_type::unwrap_to(env, cond, &pos)?;
    let cond = int.cast_into(env, block, cond, pos)?;
    Ok(AnalysisResult {
        ty: Type::CIf(crate::ct::CIf),
        value: cond.value,
    })
}

fn builtin_c_compile_call(
    env: &mut Env,
    _slot: Type,
    pos: TokenPosition,
    arg_ast: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<AnalysisResult, PositionedError> {
    with_target_env(env, TargetEnv::C, |env| {
        let arg_res = analyze(
            env,
            Type::CtExportList(CtExportList {
                key: Box::new(Type::CExportName(CExportName)),
            }),
            arg_ast.pos,
            arg_ast.ast,
            block,
        )?;
        let arg_ct = crate::comptime::get_comptime(
            env,
            Some(crate::comptime::ComptimeValueKind::ExportList),
            arg_res.value,
            pos.clone(),
        )?;
        let ComptimeValue::ExportList(arg_ct) = arg_ct else {
            unreachable!("get_comptime guarantees a matching kind")
        };

        let mut ctx = crate::backend::c::CCodegenCtx::default();
        for entry in &arg_ct.exports {
            let name = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::CExportName),
                RuntimeValue::Comptime(entry.key.clone()),
                entry.key_pos.clone(),
            )?;
            let ComptimeValue::CExportName(name) = name else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            let body = analyze(
                env,
                Type::Unknown(TypeUnknown),
                entry.value.pos.clone(),
                &entry.value.ast,
                block,
            )?;
            let Type::Fn(_) = &body.ty else {
                return Err(throw_err(
                    env,
                    Some(entry.value.pos.clone()),
                    format!("TODO export {} to C", body.ty.dump()),
                    None,
                    None,
                ));
            };
            let content = crate::comptime::get_comptime(
                env,
                Some(crate::comptime::ComptimeValueKind::Fn),
                body.value,
                entry.value.pos.clone(),
            )?;
            let ComptimeValue::Fn(content) = content else {
                unreachable!("get_comptime guarantees a matching kind")
            };
            if !ctx.export(content, name.value.as_str()) {
                return Err(throw_err(
                    env,
                    Some(entry.key_pos.clone()),
                    "duplicate export",
                    None,
                    None,
                ));
            }
        }

        let source = crate::backend::c::codegen_c(env, &mut ctx)?;
        Ok(AnalysisResult {
            ty: Type::CtBuildArtifact(CtBuildArtifact {
                narrow: Some(CtBuildArtifactNarrow::File),
            }),
            value: RuntimeValue::Comptime(ComptimeValue::BuildArtifact(
                ComptimeValueBuildArtifact::File(ComptimeFile {
                    value: source.into_bytes(),
                }),
            )),
        })
    })
}

fn build_builtin_namespace_descriptor() -> Rc<dyn Descriptor> {
    d_ns(
        vec![
            (
                "build",
                d_raw(AnalysisResult {
                    ty: Type::CtKey(CtKey),
                    value: RuntimeValue::Comptime(ComptimeValue::Key(build_symbol_value())),
                }),
            ),
            (
                "c",
                d_ns(
                    vec![
                        ("compile", d_ns(vec![], Some(builtin_c_compile_call))),
                        ("if", d_ns(vec![], Some(builtin_c_if_call))),
                        ("binary", d_ns(vec![], Some(builtin_c_binary_call))),
                        (
                            "int_from_kw",
                            d_ns(vec![], Some(builtin_c_int_from_kw_call)),
                        ),
                        (
                            "int",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CInt(CInt),
                                    },
                                )),
                            }),
                        ),
                    ],
                    None,
                ),
            ),
            (
                "reflect",
                d_ns(
                    vec![
                        (
                            "function",
                            d_ns(vec![], Some(crate::reflect::builtin_reflect_function_call)),
                        ),
                        (
                            "fail",
                            d_ns(vec![], Some(crate::reflect::builtin_reflect_fail_call)),
                        ),
                        (
                            "Value",
                            d_type(Type::ReflectValue(crate::ct::TypeReflectValue)),
                        ),
                        (
                            "Constant",
                            d_type(Type::ReflectConstant(crate::ct::TypeReflectConstant)),
                        ),
                        (
                            "Data",
                            d_type(Type::ReflectData(crate::ct::TypeReflectData)),
                        ),
                        ("Fn", d_type(Type::ReflectFn(crate::ct::TypeReflectFn))),
                    ],
                    None,
                ),
            ),
            (
                "std",
                d_ns(
                    vec![
                        ("Target", d_type(Type::Target(crate::ct::TypeTarget))),
                        (
                            "emit",
                            d_ns(vec![], Some(crate::reflect::builtin_emit_call)),
                        ),
                        ("reflect", Rc::new(ReflectPreludeDescriptor)),
                        (
                            "File",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CtBuildArtifact(CtBuildArtifact {
                                            narrow: Some(CtBuildArtifactNarrow::File),
                                        }),
                                    },
                                )),
                            }),
                        ),
                        (
                            "Folder",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CtBuildArtifact(CtBuildArtifact {
                                            narrow: Some(CtBuildArtifactNarrow::Folder),
                                        }),
                                    },
                                )),
                            }),
                        ),
                        (
                            "mc",
                            Rc::new(PreludeDescriptor(crate::user_type::LazyPrelude::mc)),
                        ),
                        (
                            "kw",
                            d_ns(
                                vec![
                                    (
                                        "int",
                                        d_raw(AnalysisResult {
                                            ty: Type::CtType(CtType),
                                            value: RuntimeValue::Comptime(ComptimeValue::Type(
                                                ComptimeValueType {
                                                    ty: Type::KwInt(crate::ct::KwInt),
                                                },
                                            )),
                                        }),
                                    ),
                                    (
                                        "string",
                                        d_raw(AnalysisResult {
                                            ty: Type::CtType(CtType),
                                            value: RuntimeValue::Comptime(ComptimeValue::Type(
                                                ComptimeValueType {
                                                    ty: Type::KwString(crate::ct::KwString),
                                                },
                                            )),
                                        }),
                                    ),
                                    ("list", d_ns(vec![], Some(builtin_kw_list_call))),
                                    ("map", d_ns(vec![], Some(builtin_kw_map_call))),
                                    (
                                        "text",
                                        d_raw(AnalysisResult {
                                            ty: Type::CtType(CtType),
                                            value: RuntimeValue::Comptime(ComptimeValue::Type(
                                                ComptimeValueType {
                                                    ty: Type::KwText(crate::ct::KwText),
                                                },
                                            )),
                                        }),
                                    ),
                                    (
                                        "null",
                                        d_raw(AnalysisResult {
                                            ty: Type::Null(crate::ct::TypeNull),
                                            value: RuntimeValue::Comptime(ComptimeValue::Optional(
                                                ComptimeValueOptional { some: None },
                                            )),
                                        }),
                                    ),
                                    (
                                        "bool",
                                        d_raw(AnalysisResult {
                                            ty: Type::CtType(CtType),
                                            value: RuntimeValue::Comptime(ComptimeValue::Type(
                                                ComptimeValueType {
                                                    ty: Type::KwBool(crate::ct::KwBool),
                                                },
                                            )),
                                        }),
                                    ),
                                    ("if", d_ns(vec![], Some(builtin_kw_if_call))),
                                    ("loop", d_ns(vec![], Some(builtin_kw_loop_call))),
                                    ("mut", d_ns(vec![], Some(builtin_kw_mut_call))),
                                ],
                                None,
                            ),
                        ),
                        (
                            "operator",
                            d_ns(
                                vec![
                                    ("slot", d_ns(vec![], Some(builtin_operator_slot_call))),
                                    ("lhs", d_ns(vec![], Some(builtin_operator_lhs_call))),
                                    ("call", d_std_key(crate::std_keys::StdKey::Call)),
                                ],
                                None,
                            ),
                        ),
                        (
                            "literal",
                            d_ns(
                                vec![
                                    (
                                        "string",
                                        d_std_key(crate::std_keys::StdKey::Literal(
                                            crate::std_keys::LiteralKind::String,
                                        )),
                                    ),
                                    (
                                        "number",
                                        d_std_key(crate::std_keys::StdKey::Literal(
                                            crate::std_keys::LiteralKind::Number,
                                        )),
                                    ),
                                    (
                                        "list",
                                        d_std_key(crate::std_keys::StdKey::Literal(
                                            crate::std_keys::LiteralKind::List,
                                        )),
                                    ),
                                    (
                                        "map",
                                        d_std_key(crate::std_keys::StdKey::Literal(
                                            crate::std_keys::LiteralKind::Map,
                                        )),
                                    ),
                                ],
                                None,
                            ),
                        ),
                        (
                            "c",
                            Rc::new(PreludeDescriptor(crate::user_type::LazyPrelude::c)),
                        ),
                        ("Option", d_ns(vec![], Some(builtin_option_call))),
                        ("Struct", d_ns(vec![], Some(builtin_struct_call))),
                        ("Enum", d_ns(vec![], Some(builtin_enum_call))),
                        (
                            "Type",
                            d_raw(AnalysisResult {
                                ty: Type::CtType(CtType),
                                value: RuntimeValue::Comptime(ComptimeValue::Type(
                                    ComptimeValueType {
                                        ty: Type::CtType(CtType),
                                    },
                                )),
                            }),
                        ),
                        (
                            "type",
                            d_ns(
                                vec![
                                    ("wrap", d_ns(vec![], Some(builtin_type_wrap_call))),
                                    ("unwrap", d_ns(vec![], Some(builtin_type_unwrap_call))),
                                    ("repr", d_std_key(crate::std_keys::StdKey::Repr)),
                                    (
                                        "name",
                                        d_raw(crate::kw::static_fn(
                                            "std.type.name",
                                            crate::kw::KwBuiltinOp::TypeName,
                                            Type::CtType(CtType),
                                            Type::KwString(crate::ct::KwString),
                                        )),
                                    ),
                                    (
                                        "base",
                                        d_raw(crate::kw::static_fn(
                                            "std.type.base",
                                            crate::kw::KwBuiltinOp::TypeBase,
                                            Type::CtType(CtType),
                                            Type::CtType(CtType),
                                        )),
                                    ),
                                    (
                                        "fields",
                                        d_std_key(crate::std_keys::StdKey::Section(
                                            crate::std_keys::Section::Fields,
                                        )),
                                    ),
                                    (
                                        "cases",
                                        d_std_key(crate::std_keys::StdKey::Section(
                                            crate::std_keys::Section::Cases,
                                        )),
                                    ),
                                    (
                                        "statics",
                                        d_std_key(crate::std_keys::StdKey::Section(
                                            crate::std_keys::Section::Statics,
                                        )),
                                    ),
                                    (
                                        "methods",
                                        d_std_key(crate::std_keys::StdKey::Section(
                                            crate::std_keys::Section::Methods,
                                        )),
                                    ),
                                ],
                                None,
                            ),
                        ),
                    ],
                    None,
                ),
            ),
        ],
        None,
    )
}

thread_local! {
    static BUILTIN_NAMESPACE: Rc<dyn Descriptor> = build_builtin_namespace_descriptor();
}

fn builtin_namespace_descriptor() -> Rc<dyn Descriptor> {
    BUILTIN_NAMESPACE.with(|d| d.clone())
}

fn import_file_body(
    env: &mut Env,
    filename: &str,
    root_pos: TokenPosition,
    tokenized_result: &[SyntaxNode],
) -> Result<ComptimeValueBuildArtifact, PositionedError> {
    let mut block = empty_block();
    let ns = analyze_namespace(
        env,
        TokenPosition {
            fyl: filename.to_string(),
            lyn: 0,
            col: 0,
            idx: 0,
        },
        tokenized_result,
    )?;
    let main_fn = ns.get_symbol(
        env,
        root_pos.clone(),
        build_symbol_child_type(),
        build_symbol(),
        &mut block,
    )?;
    let Some(main_fn) = main_fn else {
        return Err(throw_err(
            env,
            Some(root_pos),
            "expected main fn",
            None,
            None,
        ));
    };
    let void_raw = SyntaxNode::Raw(RawToken {
        pos: compiler_pos(),
        raw: String::new(),
        tag: RawTag::Void,
    });
    let call_result = analyze_call(
        env,
        std_folder_or_file_type(),
        root_pos.clone(),
        main_fn,
        CallArg {
            pos: compiler_pos(),
            ast: std::slice::from_ref(&void_raw),
        },
        &mut block,
    )?;
    let evaluated =
        crate::comptime::comptime_eval(env, &block, call_result.value, root_pos.clone())?;
    let result = crate::comptime::get_comptime(
        env,
        Some(crate::comptime::ComptimeValueKind::BuildArtifact),
        RuntimeValue::Comptime(evaluated),
        root_pos,
    )?;
    let ComptimeValue::BuildArtifact(result) = result else {
        unreachable!("get_comptime guarantees a matching kind")
    };
    Ok(result)
}

const IMPORT_STACK_SIZE: usize = 1 << 30;

pub fn import_file(
    filename: &str,
    contents: &str,
) -> Result<ComptimeValueBuildArtifact, Vec<TokenizationError>> {
    import_file_with_step_limit(filename, contents, crate::comptime::DEFAULT_STEP_LIMIT)
}

pub fn import_file_with_step_limit(
    filename: &str,
    contents: &str,
    step_limit: usize,
) -> Result<ComptimeValueBuildArtifact, Vec<TokenizationError>> {
    let filename = filename.to_string();
    let contents = contents.to_string();
    std::thread::Builder::new()
        .name("cvl2 import".to_string())
        .stack_size(IMPORT_STACK_SIZE)
        .spawn(move || {
            crate::comptime::set_step_limit(step_limit);
            import_file_on_this_thread(&filename, &contents)
        })
        .expect("spawning the import thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn import_file_on_this_thread(
    filename: &str,
    contents: &str,
) -> Result<ComptimeValueBuildArtifact, Vec<TokenizationError>> {
    let mut source = Source::new(filename, contents);
    let tokenized = tokenize(&mut source);
    let root_pos = TokenPosition {
        fyl: filename.to_string(),
        lyn: 0,
        col: 0,
        idx: 0,
    };

    let mut changes = HashMap::new();
    changes.insert(target_env_symbol(), TargetEnv::Build);
    let mut env = Env {
        trace: Vec::new(),
        errors: tokenized.errors.clone(),
        scope: Scope {
            comptime: ComptimeScopeMap::root(changes),
            bindings: Rc::new(RefCell::new(HashMap::new())),
        },
        fn_cache: Rc::new(PerComptimeScopeCache::new()),
        decl_cache: Rc::new(PerComptimeScopeCache::new()),
        builtin_cache: Rc::new(PerComptimeScopeCache::new()),
    };

    let result = import_file_body(&mut env, filename, root_pos, &tokenized.result);
    match result {
        Ok(artifact) if env.errors.is_empty() => Ok(artifact),
        Ok(_) => Err(env.errors),
        Err(e) => {
            handle_err(&mut env, e);
            Err(env.errors)
        }
    }
}

fn renumber_extract(
    extract: &DestructureExtract,
    old: &[DestructureTarget],
    new: &mut Vec<DestructureTarget>,
) -> DestructureExtract {
    match extract {
        DestructureExtract::SingleItem { target, pos } => {
            new.push(old[*target].clone());
            DestructureExtract::SingleItem {
                target: new.len() - 1,
                pos: pos.clone(),
            }
        }
        DestructureExtract::Comptime { target, pos } => {
            new.push(old[*target].clone());
            DestructureExtract::Comptime {
                target: new.len() - 1,
                pos: pos.clone(),
            }
        }
        DestructureExtract::List { items, pos } => DestructureExtract::List {
            items: items
                .iter()
                .map(|item| renumber_extract(item, old, new))
                .collect(),
            pos: pos.clone(),
        },
        DestructureExtract::Map { items, pos } => DestructureExtract::Map {
            items: items
                .iter()
                .map(|(key, item)| (key.clone(), renumber_extract(item, old, new)))
                .collect(),
            pos: pos.clone(),
        },
        DestructureExtract::Discard { pos } => DestructureExtract::Discard { pos: pos.clone() },
    }
}

const MAX_SPECIALIZATIONS: usize = 256;

pub fn specialize_call(
    env: &mut Env,
    callee: &ComptimeValueFn,
    pos: &TokenPosition,
    arg_in: CallArg<'_>,
    block: &mut AnalysisBlock,
) -> Result<(ComptimeValueFn, RuntimeValue), PositionedError> {
    let args = callee.args();
    let (
        Type::Tuple(types),
        DestructureExtract::List {
            items: extracts,
            pos: list_pos,
        },
    ) = (&args.ty, &args.extract)
    else {
        unreachable!("compile-time parameters only appear in a parameter list")
    };
    let items: Vec<OperatorSegmentToken> = match crate::ct::call_list_items(env, &arg_in)? {
        Some(items) => items,
        None if extracts.len() == 1 => vec![OperatorSegmentToken {
            pos: arg_in.pos.clone(),
            items: arg_in.ast.to_vec(),
        }],
        None => Vec::new(),
    };
    if items.len() != extracts.len() {
        return Err(throw_err(
            env,
            Some(pos.clone()),
            format!("expected {} arguments, got {}", extracts.len(), items.len()),
            None,
            None,
        ));
    }
    let mut known = Vec::new();
    let mut bindings = Vec::new();
    let mut runtime_values = Vec::new();
    let mut runtime_types = Vec::new();
    let mut runtime_extracts = Vec::new();
    for ((item, ty), extract) in items.iter().zip(&types.children).zip(extracts) {
        let value = analyze(env, ty.clone(), item.pos.clone(), &item.items, block)?;
        let value = ty.cast_into(env, block, value, item.pos.clone())?;
        if let DestructureExtract::Comptime {
            target,
            pos: param_pos,
        } = extract
        {
            let name = &args.targets[*target].name;
            let RuntimeValue::Comptime(constant) = value.value else {
                return Err(throw_err(
                    env,
                    Some(item.pos.clone()),
                    format!("{name} must be known at compile time"),
                    Some(vec![(Some(param_pos.clone()), "declared here".to_string())]),
                    None,
                ));
            };
            if !crate::kw::can_compare(&constant) {
                return Err(throw_err(
                    env,
                    Some(item.pos.clone()),
                    format!("a {} can't be a compile-time argument", value.ty.dump()),
                    None,
                    None,
                ));
            }
            known.push(constant.clone());
            bindings.push((
                name.clone(),
                Binding::Runtime {
                    pos: param_pos.clone(),
                    runtime: AnalysisResult {
                        ty: value.ty,
                        value: RuntimeValue::Comptime(constant),
                    },
                },
            ));
        } else {
            runtime_values.push(value.value);
            runtime_types.push(ty.clone());
            runtime_extracts.push(extract);
        }
    }
    let existing = callee
        .0
        .specializations
        .borrow()
        .iter()
        .find(|(values, _)| {
            values.len() == known.len()
                && values
                    .iter()
                    .zip(&known)
                    .all(|(a, b)| crate::kw::values_equal(a, b))
        })
        .map(|(_, special)| special.clone());
    let special = match existing {
        Some(special) => special,
        None if callee.0.specializations.borrow().len() >= MAX_SPECIALIZATIONS => {
            return Err(throw_err(
                env,
                Some(pos.clone()),
                format!(
                    "this function was copied for {MAX_SPECIALIZATIONS} sets of compile-time arguments; does a recursive call change one every time?"
                ),
                Some(vec![(
                    Some(callee.pos().clone()),
                    "function defined here".to_string(),
                )]),
                None,
            ));
        }
        None => {
            let mut targets = Vec::new();
            let items = runtime_extracts
                .into_iter()
                .map(|extract| renumber_extract(extract, &args.targets, &mut targets))
                .collect();
            let mut scope_bindings = callee.body().scope.bindings.borrow().clone();
            scope_bindings.extend(bindings);
            let special = ComptimeValueFn::new(
                Destructure {
                    targets,
                    extract: DestructureExtract::List {
                        items,
                        pos: list_pos.clone(),
                    },
                    ty: Type::Tuple(TypeTuple {
                        children: runtime_types,
                    }),
                    tags: Vec::new(),
                },
                ComptimeValueAst {
                    ast: callee.body().ast.clone(),
                    pos: callee.body().pos.clone(),
                    scope: Scope {
                        comptime: callee.body().scope.comptime.clone(),
                        bindings: Rc::new(RefCell::new(scope_bindings)),
                    },
                },
                callee.pos().clone(),
            );
            callee
                .0
                .specializations
                .borrow_mut()
                .push((known, special.clone()));
            special
        }
    };
    let arg = block_append(
        block,
        AnalysisLine::Tuple {
            pos: pos.clone(),
            items: runtime_values,
        },
    );
    Ok((special, RuntimeValue::Runtime(arg)))
}
