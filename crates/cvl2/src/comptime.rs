use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const MAX_STEPS: usize = 10_000_000;
const MAX_CALL_DEPTH: usize = 1000;

thread_local! {
    static CALL_DEPTH: Cell<usize> = const { Cell::new(0) };
}

use crate::compiler::{
    AnalysisBlock, AnalysisLine, ComptimeFile, ComptimeValue, ComptimeValueBuildArtifact,
    ComptimeValueTuple, Env, NsFields, NsFieldsEntry, PositionedError, Region, RuntimeValue,
    Symbol, analyze_function, throw_consumed_err, throw_err,
};
use crate::ct::fold_kw_binary;
use crate::parser::{ErrorStyle, TokenPosition};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComptimeValueKind {
    Key,
    Namespace,
    Type,
    Ast,
    Void,
    KvFields,
    Fn,
    Optional,
    BuildArtifact,
    Uint8Array,
    ExportList,
    CExportName,
    McIdentifier,
    McResult,
    CInt,
    OperatorName,
    KwInt,
    KwBool,
    Tuple,
    KwMut,
    McNbtRef,
    Error,
    Mc,
}

impl ComptimeValueKind {
    fn of(value: &ComptimeValue) -> Self {
        match value {
            ComptimeValue::Key(_) => ComptimeValueKind::Key,
            ComptimeValue::Namespace(_) => ComptimeValueKind::Namespace,
            ComptimeValue::Type(_) => ComptimeValueKind::Type,
            ComptimeValue::Ast(_) => ComptimeValueKind::Ast,
            ComptimeValue::Void(_) => ComptimeValueKind::Void,
            ComptimeValue::KvFields(_) => ComptimeValueKind::KvFields,
            ComptimeValue::Fn(_) => ComptimeValueKind::Fn,
            ComptimeValue::Optional(_) => ComptimeValueKind::Optional,
            ComptimeValue::BuildArtifact(_) => ComptimeValueKind::BuildArtifact,
            ComptimeValue::Uint8Array(_) => ComptimeValueKind::Uint8Array,
            ComptimeValue::ExportList(_) => ComptimeValueKind::ExportList,
            ComptimeValue::CExportName(_) => ComptimeValueKind::CExportName,
            ComptimeValue::McIdentifier(_) => ComptimeValueKind::McIdentifier,
            ComptimeValue::McResult(_) => ComptimeValueKind::McResult,
            ComptimeValue::CInt(_) => ComptimeValueKind::CInt,
            ComptimeValue::OperatorName(_) => ComptimeValueKind::OperatorName,
            ComptimeValue::KwInt(_) => ComptimeValueKind::KwInt,
            ComptimeValue::KwBool(_) => ComptimeValueKind::KwBool,
            ComptimeValue::Tuple(_) => ComptimeValueKind::Tuple,
            ComptimeValue::KwMut(_) => ComptimeValueKind::KwMut,
            ComptimeValue::McNbtRef(_) => ComptimeValueKind::McNbtRef,
            ComptimeValue::Error(_) => ComptimeValueKind::Error,
            ComptimeValue::Mc(_) => ComptimeValueKind::Mc,
        }
    }
}

struct RuntimeData<'a> {
    block: &'a AnalysisBlock,
    results: &'a [Option<ComptimeValue>],
}

fn narrow_mismatch(
    env: &Env,
    pos: TokenPosition,
    expected: ComptimeValueKind,
    actual: ComptimeValueKind,
) -> PositionedError {
    throw_err(
        env,
        Some(pos),
        format!("Expected value of type {expected:?}, got {actual:?}"),
        None,
        None,
    )
}

fn get_comptime_impl(
    env: &mut Env,
    k: Option<ComptimeValueKind>,
    v: RuntimeValue,
    pos: TokenPosition,
    runtime: Option<&RuntimeData<'_>>,
) -> Result<ComptimeValue, PositionedError> {
    match v {
        RuntimeValue::Runtime(idx) => {
            let Some(runtime) = runtime else {
                return Err(throw_err(
                    env,
                    Some(pos),
                    "Value must be known at comptime.",
                    None,
                    None,
                ));
            };
            if idx.1 != runtime.block.validate {
                return Err(throw_err(
                    env,
                    Some(pos),
                    "Assertion failure: Ex",
                    None,
                    None,
                ));
            }
            let resolved = runtime.results[idx.0]
                .clone()
                .expect("runtime result must be populated before being referenced");
            get_comptime_impl(env, k, RuntimeValue::Comptime(resolved), pos, None)
        }
        RuntimeValue::Comptime(ComptimeValue::Error(err))
            if k.is_some_and(|k| k != ComptimeValueKind::Error) =>
        {
            Err(throw_consumed_err(err.etok))
        }
        RuntimeValue::Comptime(value) => match k {
            None => Ok(value),
            Some(k) if ComptimeValueKind::of(&value) == k => Ok(value),
            Some(k) => Err(narrow_mismatch(env, pos, k, ComptimeValueKind::of(&value))),
        },
    }
}

pub fn get_comptime(
    env: &mut Env,
    k: Option<ComptimeValueKind>,
    v: RuntimeValue,
    pos: TokenPosition,
) -> Result<ComptimeValue, PositionedError> {
    get_comptime_impl(env, k, v, pos, None)
}

pub fn comptime_eval(
    env: &mut Env,
    block: &AnalysisBlock,
    result: RuntimeValue,
    pos: TokenPosition,
) -> Result<ComptimeValue, PositionedError> {
    comptime_eval_with_args(env, block, result, pos, None)
}

fn comptime_eval_with_args(
    env: &mut Env,
    block: &AnalysisBlock,
    result: RuntimeValue,
    pos: TokenPosition,
    args: Option<RuntimeValue>,
) -> Result<ComptimeValue, PositionedError> {
    let mut results: Vec<Option<ComptimeValue>> = block.lines.iter().map(|_| None).collect();
    let mut region_end: Vec<Option<usize>> = vec![None; block.lines.len()];
    let mut region_begin: Vec<Option<usize>> = vec![None; block.lines.len()];
    let mut label_end: HashMap<Symbol, usize> = HashMap::new();
    let mut open = Vec::new();
    for (i, instr) in block.lines.iter().enumerate() {
        match instr {
            AnalysisLine::RegionBegin { .. } | AnalysisLine::LabelBegin { .. } => open.push(i),
            AnalysisLine::RegionEnd { .. } | AnalysisLine::LabelEnd { .. } => {
                let begin = open.pop().expect("analysis closes every region it opens");
                region_end[begin] = Some(i);
                region_begin[i] = Some(begin);
                if let AnalysisLine::LabelEnd { label, .. } = instr {
                    label_end.insert(*label, i);
                }
            }
            _ => {}
        }
    }

    let mut i = 0;
    let mut steps: usize = 0;
    while i < block.lines.len() {
        let instr = &block.lines[i];
        let mut next = i + 1;
        steps += 1;
        if steps > MAX_STEPS {
            return Err(throw_err(
                env,
                Some(crate::printers::analysis_line_pos(instr).clone()),
                "compile-time evaluation took too many steps; is a std.kw.loop missing a break?",
                None,
                None,
            ));
        }
        match instr {
            AnalysisLine::ComptimeKvListInit { .. } => {
                results[i] = Some(ComptimeValue::KvFields(NsFields {
                    locked: false,
                    entries: Vec::new(),
                }));
            }
            AnalysisLine::ComptimeKvListAppend {
                pos: ipos,
                key,
                list,
                value,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let fields_check = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::KvFields),
                    list.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::KvFields(fields_check) = fields_check else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                if fields_check.locked {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "assertion failed",
                        None,
                        None,
                    ));
                }
                let key_val =
                    get_comptime_impl(env, None, key.clone(), ipos.clone(), Some(&runtime))?;
                let value_val =
                    get_comptime_impl(env, None, value.clone(), ipos.clone(), Some(&runtime))?;

                let RuntimeValue::Runtime(list_idx) = list else {
                    unreachable!("get_comptime already validated this resolves to a runtime slot")
                };
                let Some(ComptimeValue::KvFields(fields)) = &mut results[list_idx.0] else {
                    unreachable!("validated above via get_comptime")
                };
                fields.entries.push(NsFieldsEntry {
                    pos: ipos.clone(),
                    key: key_val,
                    value: value_val,
                });
            }
            AnalysisLine::Call {
                pos: ipos,
                method,
                arg,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let method_val = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::Fn),
                    method.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::Fn(method_val) = method_val else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let arg_val =
                    get_comptime_impl(env, None, arg.clone(), ipos.clone(), Some(&runtime))?;
                let body = analyze_function(env, &method_val)?;
                let depth = CALL_DEPTH.with(|d| d.get());
                if depth >= MAX_CALL_DEPTH {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "compile-time calls are nested too deeply",
                        None,
                        None,
                    ));
                }
                CALL_DEPTH.with(|d| d.set(depth + 1));
                let value = comptime_eval_with_args(
                    env,
                    &body.block,
                    body.value,
                    ipos.clone(),
                    Some(RuntimeValue::Comptime(arg_val)),
                );
                CALL_DEPTH.with(|d| d.set(depth));
                let value = value?;
                results[i] = Some(value);
            }
            AnalysisLine::Args { pos: ipos } => {
                let Some(args_val) = args.clone() else {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "cannot get args when executing without args",
                        Some(vec![(Some(pos.clone()), "called here".to_string())]),
                        Some(ErrorStyle::Unreachable),
                    ));
                };
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let value = get_comptime_impl(env, None, args_val, ipos.clone(), Some(&runtime))?;
                results[i] = Some(value);
            }
            AnalysisLine::ComptimeFileCreate { pos: ipos, value } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let arg_val = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::Uint8Array),
                    value.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::Uint8Array(arg_val) = arg_val else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                results[i] = Some(ComptimeValue::BuildArtifact(
                    ComptimeValueBuildArtifact::File(ComptimeFile {
                        value: arg_val.value,
                    }),
                ));
            }
            AnalysisLine::Tuple { pos: ipos, items } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let mut values = Vec::new();
                for item in items {
                    values.push(get_comptime_impl(
                        env,
                        None,
                        item.clone(),
                        ipos.clone(),
                        Some(&runtime),
                    )?);
                }
                results[i] = Some(ComptimeValue::Tuple(ComptimeValueTuple { items: values }));
            }
            AnalysisLine::TupleGet {
                pos: ipos,
                tuple,
                index,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let tuple = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::Tuple),
                    tuple.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?;
                let ComptimeValue::Tuple(tuple) = tuple else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                results[i] = Some(tuple.items[*index].clone());
            }
            AnalysisLine::KwBinary {
                pos: ipos,
                op,
                lhs,
                rhs,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let lhs = get_comptime_impl(env, None, lhs.clone(), ipos.clone(), Some(&runtime))?;
                let rhs = get_comptime_impl(env, None, rhs.clone(), ipos.clone(), Some(&runtime))?;
                results[i] = Some(fold_kw_binary(env, ipos, *op, &lhs, &rhs)?);
            }
            AnalysisLine::RegionBegin { pos: ipos, region } => {
                let end = region_end[i].expect("every region_begin has a region_end");
                match region {
                    Region::KwIf { cond } => {
                        let runtime = RuntimeData {
                            block,
                            results: &results,
                        };
                        let taken = get_comptime_impl(
                            env,
                            Some(ComptimeValueKind::KwBool),
                            cond.clone(),
                            ipos.clone(),
                            Some(&runtime),
                        )?;
                        let ComptimeValue::KwBool(taken) = taken else {
                            unreachable!("get_comptime guarantees a matching kind")
                        };
                        results[end] = Some(ComptimeValue::KwBool(taken.clone()));
                        if !taken.value {
                            next = end + 1;
                        }
                    }
                    Region::KwElse { if_end } => {
                        let if_taken = matches!(
                            &results[if_end.0],
                            Some(ComptimeValue::KwBool(taken)) if taken.value
                        );
                        if if_taken {
                            next = end + 1;
                        }
                    }
                    Region::KwLoop => {}
                    Region::CIf { .. } => {
                        return Err(throw_err(
                            env,
                            Some(ipos.clone()),
                            "std.c.if can't run at compile time",
                            None,
                            None,
                        ));
                    }
                }
            }
            AnalysisLine::RegionEnd { .. } => {
                let begin = region_begin[i].expect("every region_end has a region_begin");
                if let AnalysisLine::RegionBegin {
                    region: Region::KwLoop,
                    ..
                } = &block.lines[begin]
                {
                    next = begin + 1;
                }
            }
            AnalysisLine::LabelBegin { .. } => {}
            AnalysisLine::MutNew { pos: ipos, init } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let init =
                    get_comptime_impl(env, None, init.clone(), ipos.clone(), Some(&runtime))?;
                results[i] = Some(ComptimeValue::KwMut(Rc::new(RefCell::new(init))));
            }
            AnalysisLine::MutGet { pos: ipos, cell } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let ComptimeValue::KwMut(cell) = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::KwMut),
                    cell.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?
                else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                results[i] = Some(cell.borrow().clone());
            }
            AnalysisLine::MutSet {
                pos: ipos,
                cell,
                value,
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                let ComptimeValue::KwMut(cell) = get_comptime_impl(
                    env,
                    Some(ComptimeValueKind::KwMut),
                    cell.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?
                else {
                    unreachable!("get_comptime guarantees a matching kind")
                };
                let value =
                    get_comptime_impl(env, None, value.clone(), ipos.clone(), Some(&runtime))?;
                *cell.borrow_mut() = value;
            }
            AnalysisLine::LabelEnd {
                pos: ipos, value, ..
            } => {
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                results[i] = Some(get_comptime_impl(
                    env,
                    None,
                    value.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?);
            }
            AnalysisLine::Break {
                pos: ipos,
                label,
                value,
            } => {
                let Some(end) = label_end.get(label).copied() else {
                    return Err(throw_err(
                        env,
                        Some(ipos.clone()),
                        "break to a label that isn't open here",
                        None,
                        None,
                    ));
                };
                let runtime = RuntimeData {
                    block,
                    results: &results,
                };
                results[end] = Some(get_comptime_impl(
                    env,
                    None,
                    value.clone(),
                    ipos.clone(),
                    Some(&runtime),
                )?);
                next = end + 1;
            }
            AnalysisLine::CBinary { pos: ipos, .. } => {
                return Err(throw_err(
                    env,
                    Some(ipos.clone()),
                    "std.c.int operators can't run at compile time",
                    None,
                    None,
                ));
            }
            AnalysisLine::McExecRaw { pos: ipos, .. } => {
                return Err(throw_err(
                    env,
                    Some(ipos.clone()),
                    "todo: comptime eval expr: mc:exec_raw",
                    None,
                    None,
                ));
            }
        }
        i = next;
    }

    let runtime = RuntimeData {
        block,
        results: &results,
    };
    get_comptime_impl(env, None, result, pos, Some(&runtime))
}
