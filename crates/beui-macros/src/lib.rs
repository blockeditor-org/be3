use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::braced;
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Expr, ExprCall, ExprLit, ExprPath, ExprReference, ExprUnary, FnArg,
    GenericArgument, GenericParam, Ident, Item, ItemFn, Lit, Pat, PatType, Path, PathArguments,
    ReturnType, Signature, Token, Type, UnOp, Visibility, WherePredicate, parse_macro_input,
};

mod associated;

struct Prop {
    ident: Ident,
    ty: Type,
    default: Option<Expr>,
    is_children_slot: bool,
    /// Typed by one of the component's own type parameters: it takes what is passed as it
    /// is, and a tag that leaves it out infers that parameter as `MissingProp`.
    bare: bool,
    /// Its last type argument is one of the component's own type parameters (`Prop<V>`,
    /// `RenderFn<H, B>`) that no other prop names: what is passed decides that parameter, so
    /// it is taken without converting it (`Exact`).
    exact: bool,
    /// An `Option<X>` whose `X` is `exact`'s kind of type (`Option<RenderFn<H, B>>`): it takes
    /// an `Option<X>`, or what an exact `X` takes (`ExactWithin`).
    within: bool,
    /// `#[prop(required)]`: given even though its type has a value when left out.
    required: bool,
}

struct Setter {
    method: Ident,
    generics: Vec<proc_macro2::TokenStream>,
    args: proc_macro2::TokenStream,
    where_clause: Vec<proc_macro2::TokenStream>,
    value: proc_macro2::TokenStream,
}

/// The type a children slot's `Slot<..>` names: the declared type, or `Exact<..>` or
/// `ExactWithin<..>` around it, whose slot traits take what is passed without converting it.
fn prop_target(prop: &Prop) -> proc_macro2::TokenStream {
    let ty = &prop.ty;
    if prop.exact {
        quote! { ::__beui_tree::reactive::macro_support::Exact<#ty> }
    } else if prop.within {
        quote! { ::__beui_tree::reactive::macro_support::ExactWithin<#ty> }
    } else {
        quote! { #ty }
    }
}

/// The trait a setter asks of what it is passed, with the marker `by`: `IntoPropValue` of the
/// declared type, or `IntoExactProp` of it for a prop whose type comes from what is passed.
/// Either way the declared type is what a message names.
fn conversion(prop: &Prop, by: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let ty = &prop.ty;
    if !prop.exact && !prop.within {
        return quote! { ::__beui_tree::reactive::IntoPropValue<#ty, #by> };
    }
    let kind = slot_kind(prop);
    quote! { ::__beui_tree::reactive::macro_support::IntoExactProp<#ty, #by, #kind> }
}

/// `IntoExactProp`'s and `IntoSlot`'s `K`: which slot traits take what is passed.
fn slot_kind(prop: &Prop) -> proc_macro2::TokenStream {
    if prop.exact {
        quote! { ::__beui_tree::reactive::macro_support::Exact<()> }
    } else if prop.within {
        quote! { ::__beui_tree::reactive::macro_support::ExactWithin<()> }
    } else {
        quote! { () }
    }
}

/// `value` converted by `conversion`.
fn convert(
    prop: &Prop,
    by: proc_macro2::TokenStream,
    value: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let conversion = conversion(prop, by);
    let method = if prop.exact || prop.within {
        quote! { into_exact_prop }
    } else {
        quote! { into_prop_value }
    };
    quote! { <_ as #conversion>::#method(#value) }
}

/// A prop's one setter. `view!` calls it by the prop's name, and the children slot's also as
/// `__children`, with the children wrapped by the shape they are written in.
fn prop_setter(prop: &Prop, method: Ident) -> Setter {
    let ty = &prop.ty;
    if prop.bare {
        return Setter {
            method,
            generics: Vec::new(),
            args: quote! { value: #ty },
            where_clause: Vec::new(),
            value: quote! { value },
        };
    }
    let conversion = conversion(prop, quote! { __By });
    Setter {
        method,
        generics: vec![quote! { __Passed }, quote! { __By }],
        args: quote! { value: __Passed },
        where_clause: vec![quote! { __Passed: #conversion }],
        value: convert(prop, quote! { __By }, quote! { value }),
    }
}

fn prop_setters(prop: &Prop, children_slot: bool) -> Vec<Setter> {
    let mut setters = vec![prop_setter(prop, prop.ident.clone())];
    if children_slot {
        let mut children = prop_setter(prop, format_ident!("__children"));
        if !prop.bare {
            let target = prop_target(prop);
            let ty = &prop.ty;
            let kind = slot_kind(prop);
            let takes = quote! { ::__beui_tree::reactive::macro_support::TakesChildren<#ty, __Passed, #kind> };
            children.generics = vec![quote! { __Make }, quote! { __Passed }];
            children.args = quote! { make: __Make };
            children.where_clause = vec![
                quote! { __Make: ::core::ops::FnOnce(::__beui_tree::reactive::macro_support::Slot<#target>) -> __Passed },
                quote! { __Make: #takes },
            ];
            children.value = quote! {
                <__Make as #takes>::take_children(make(::__beui_tree::reactive::macro_support::Slot::new()))
            };
        }
        setters.push(children);
    }
    setters
}

/// A default written in `#[prop(default = ..)]` converts the way a passed value does.
fn default_value(prop: &Prop, default: &Expr) -> proc_macro2::TokenStream {
    if prop.bare {
        return quote! { #default };
    }
    convert(prop, quote! { _ }, quote! { #default })
}

fn own_param_ident(ty: &Type) -> Option<Ident> {
    match ty {
        Type::Path(path) if path.qself.is_none() => path.path.get_ident().cloned(),
        _ => None,
    }
}

/// Whether `ty` names `param` as a type of its own, not only through a projection such as
/// `V::Handed` or `<V as Trait>::Item`.
fn mentions_bare(ty: &Type, param: &Ident) -> bool {
    let tokens: Vec<proc_macro2::TokenTree> = quote! { #ty }.into_iter().collect();
    fn walk(tokens: &[proc_macro2::TokenTree], param: &Ident) -> bool {
        tokens.iter().enumerate().any(|(index, token)| match token {
            proc_macro2::TokenTree::Ident(ident) if ident == param => {
                !matches!(
                    tokens.get(index + 1),
                    Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == ':'
                ) && !matches!(
                    tokens.get(index + 1),
                    Some(proc_macro2::TokenTree::Ident(next)) if next == "as"
                )
            }
            proc_macro2::TokenTree::Group(group) => {
                let inner: Vec<_> = group.stream().into_iter().collect();
                walk(&inner, param)
            }
            _ => false,
        })
    }
    walk(&tokens, param)
}

fn is_own_param(ty: &Type, params: &[Ident]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none()
        && path.path.get_ident().is_some_and(|ident| params.contains(ident)))
}

/// `X` in `Option<X>`. The rule for props whose type comes from what is passed looks through
/// `Option`, the one other name the macro reads: an optional prop of a type it would take
/// exactly (`Option<RenderFn<H, B>>`) takes what that type does, as `Some`.
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.iter().collect::<Vec<_>>().as_slice() {
        [GenericArgument::Type(inner)] => Some(inner),
        _ => None,
    }
}

/// The last type argument of `ty`'s last path segment: `V` in `Prop<V>`, `B` in
/// `RenderFn<H, B>`.
fn last_type_arg(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let PathArguments::AngleBracketed(args) = &path.path.segments.last()?.arguments else {
        return None;
    };
    args.args.iter().rev().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

/// A render handed nothing is a `Build<C>` or `BuildFn<C>`, never a `Render<(), C>` or
/// `RenderFn<(), C>`, so that a closure child naming a value is an error there. Rust cannot
/// leave `()` out of a type parameter, so this one check reads the written type: the only
/// place the macro looks at a type's name.
fn reject_unit_handle(ty: &Type) -> syn::Result<()> {
    let Type::Path(path) = ty else {
        return Ok(());
    };
    for segment in &path.path.segments {
        let PathArguments::AngleBracketed(args) = &segment.arguments else {
            continue;
        };
        let types: Vec<&Type> = args
            .args
            .iter()
            .filter_map(|arg| match arg {
                GenericArgument::Type(ty) => Some(ty),
                _ => None,
            })
            .collect();
        let renders = segment.ident == "Render" || segment.ident == "RenderFn";
        if renders
            && let Some(Type::Tuple(handle)) = types.first()
            && handle.elems.is_empty()
        {
            let build = if segment.ident == "Render" {
                "Build"
            } else {
                "BuildFn"
            };
            return Err(syn::Error::new_spanned(
                handle,
                format!(
                    "a render with no handle is a `{build}<C>`: write `{build}<..>` for this `{}<(), ..>`",
                    segment.ident,
                ),
            ));
        }
        for ty in types {
            reject_unit_handle(ty)?;
        }
    }
    Ok(())
}

fn angle_bracketed(items: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    if items.is_empty() {
        quote! {}
    } else {
        quote! { < #(#items),* > }
    }
}

fn camel_case(ident: &Ident) -> String {
    let mut camel = String::new();
    let mut capitalise = true;
    for character in syn::ext::IdentExt::unraw(ident).to_string().chars() {
        if character == '_' {
            capitalise = true;
            continue;
        }
        if capitalise {
            camel.extend(character.to_uppercase());
            capitalise = false;
        } else {
            camel.push(character);
        }
    }
    camel
}

struct PropAttr {
    default: Option<Expr>,
    children: bool,
    required: bool,
}

/// Reads a prop's `#[prop(..)]` attributes, however many it is written with: their keys are
/// merged, and a key given twice is an error.
fn take_prop_attr(attrs: &mut Vec<Attribute>) -> syn::Result<PropAttr> {
    let mut parsed = PropAttr {
        default: None,
        children: false,
        required: false,
    };
    let (prop_attrs, others): (Vec<Attribute>, Vec<Attribute>) = std::mem::take(attrs)
        .into_iter()
        .partition(|attr| attr.path().is_ident("prop"));
    *attrs = others;
    for attr in prop_attrs {
        attr.parse_nested_meta(|meta| {
            let twice = || meta.error("this prop already says this in a `#[prop(..)]`");
            if meta.path.is_ident("children") {
                if parsed.children {
                    return Err(twice());
                }
                parsed.children = true;
                return Ok(());
            }
            if meta.path.is_ident("default") {
                if parsed.default.is_some() {
                    return Err(twice());
                }
                parsed.default = Some(meta.value()?.parse::<Expr>()?);
                return Ok(());
            }
            if meta.path.is_ident("required") {
                if parsed.required {
                    return Err(twice());
                }
                parsed.required = true;
                return Ok(());
            }
            Err(meta.error("expected `children`, `required` or `default = <expr>`"))
        })?;
    }
    Ok(parsed)
}

/// Makes a function a component, used as a tag in `view!`.
///
/// Each prop gets one setter, which takes `impl IntoPropValue<P, M>` for the prop's declared
/// type `P`: each prop type says what it takes (`beui_tree::prop_value`), and the macro reads
/// no type names. A `Prop<T>` takes a `T`, a signal or memo of one, or a closure returning one,
/// read as a getter; an `Option<X>` what an `X` takes, or an `Option<X>` passed on as it is; a
/// `Callback<T>` a closure or a signal; a render prop a closure taking its handle, and a
/// `Build<C>` or `BuildFn<C>` one taking nothing.
///
/// A prop left out of a tag takes its `#[prop(default = <expr>)]`, converted as a passed value
/// would be, or else its type's `AbsentProp` value: `None` for an `Option`, empty for a
/// `Callback` or `Children`, a signal in the component's scope holding `T::default()` for a
/// `Binding<T>`. A prop of any other type with no default is required, and leaving it out is
/// a compile error at the tag. `#[prop(required)]` is the one manual tag: it requires a prop
/// whose type has an absent value (a `Binding` with no sensible default, such as the tab
/// `Tabs` shows), and is an error on a prop that has a default or whose type has none.
/// `Option<Prop<T>>` tells "unset" apart from any value but settles at build time, so a prop
/// that must go back to nothing while alive is `Prop<Option<T>>` with
/// `#[prop(default = None)]`.
///
/// The prop named `children`, or the one marked `#[prop(children)]`, is filled by the
/// block between the tags; a component has at most one, and `children` wins. Its type says
/// which shapes of children it takes: a child type tags that build one, `Children<C>` a run,
/// a render handed a value (`Render<H, C>`, `RenderFn<H, C>`) a closure naming it, and a
/// render handed nothing (`Build<C>`, `BuildFn<C>`) tags or `|| { .. }`. A render handed
/// nothing is never written `Render<(), C>`; the macro rejects that.
///
/// A prop whose last type argument is one of the component's own type parameters, named by
/// no other prop, takes what is passed without converting it, so the parameter follows it
/// (`Show`'s `value` and `then`: its branch count). An `Option` of such a type
/// (`Option<RenderFn<H, C>>`) takes what that type does, as `Some`. A prop typed by a bare type
/// parameter
/// takes its value as it is and, with no default, may be left out: that parameter is then
/// inferred as `MissingProp` (`Show`'s `fallback`); it cannot be the children slot. The
/// component's bounds are checked on
/// each setter and on `build`, not on the builder type, so a bound a tag does not meet is
/// reported as that bound.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    parse_macro_input!(attr as syn::parse::Nothing);
    let item = parse_macro_input!(item as Item);
    let expanded = match item {
        Item::Fn(item) => expand_component(item),
        Item::Trait(item) => associated::expand_trait(item),
        Item::Impl(item) => associated::expand_impl(item),
        other => Err(syn::Error::new_spanned(
            other,
            "`#[component]` goes on a function, or on a trait or impl block whose component methods are also marked `#[component]`",
        )),
    };
    match expanded {
        Ok(expanded) => resolve_tree(expanded).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_component(item: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = item;
    let ident = sig.ident.clone();
    let generics = sig.generics.clone();
    let where_clause = &generics.where_clause;
    let ExpandedComponent {
        items,
        constructor_ty,
        constructor_body,
    } = expand_builder(ComponentSpec {
        attrs,
        vis: vis.clone(),
        label: ident.to_string(),
        base: ident.to_string(),
        sig,
        outer: Vec::new(),
        outer_where: Vec::new(),
        finish: Finish::Body(block),
    })?;
    Ok(quote! {
        #items

        #[allow(non_snake_case)]
        #vis fn #ident #generics () -> #constructor_ty #where_clause {
            #constructor_body
        }
    })
}

struct ComponentSpec {
    attrs: Vec<Attribute>,
    vis: Visibility,
    sig: Signature,
    label: String,
    base: String,
    outer: Vec<(GenericParam, proc_macro2::TokenStream)>,
    outer_where: Vec<WherePredicate>,
    finish: Finish,
}

enum Finish {
    Body(Box<Block>),
    Call(proc_macro2::TokenStream),
}

struct ExpandedComponent {
    items: proc_macro2::TokenStream,
    constructor_ty: proc_macro2::TokenStream,
    constructor_body: proc_macro2::TokenStream,
}

fn generic_arg(param: &GenericParam) -> proc_macro2::TokenStream {
    match param {
        GenericParam::Type(param) => {
            let ident = &param.ident;
            quote! { #ident }
        }
        GenericParam::Lifetime(param) => {
            let lifetime = &param.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Const(param) => {
            let ident = &param.ident;
            quote! { #ident }
        }
    }
}

fn expand_builder(spec: ComponentSpec) -> syn::Result<ExpandedComponent> {
    let ComponentSpec {
        attrs,
        vis,
        mut sig,
        label: name,
        base,
        outer,
        outer_where,
        finish,
    } = spec;
    let builder_ident = format_ident!("__{}_Builder", base);
    let output_ty = match &sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    let own_params: Vec<Ident> = sig
        .generics
        .type_params()
        .map(|param| param.ident.clone())
        .collect();
    let mut props: Vec<Prop> = sig
        .inputs
        .iter_mut()
        .map(|arg| {
            let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "a `#[component]` function cannot take `self`",
                ));
            };
            let Pat::Ident(pat_ident) = pat.as_ref() else {
                return Err(syn::Error::new_spanned(
                    pat,
                    "a `#[component]` prop must be a plain name rather than a pattern",
                ));
            };
            let ident = pat_ident.ident.clone();
            let PropAttr {
                default,
                children,
                required,
            } = take_prop_attr(attrs)?;
            if required && default.is_some() {
                return Err(syn::Error::new_spanned(
                    ty,
                    "`#[prop(required)]` is for a prop with no default, whose type has a value when it is left out (a `Binding<T>`)",
                ));
            }
            reject_unit_handle(ty)?;
            let bare = is_own_param(ty, &own_params);
            let exact = !bare
                && last_type_arg(ty).is_some_and(|inner| is_own_param(inner, &own_params));
            let within = !bare
                && !exact
                && option_inner(ty)
                    .and_then(last_type_arg)
                    .is_some_and(|inner| is_own_param(inner, &own_params));
            Ok(Prop {
                ident,
                ty: (**ty).clone(),
                default,
                is_children_slot: children,
                bare,
                exact,
                within,
                required,
            })
        })
        .collect::<syn::Result<_>>()?;
    // A prop whose last type argument is a type parameter no other prop names takes its type
    // only from what is passed; one another prop names converts as usual.
    let mentioned: Vec<bool> = props
        .iter()
        .enumerate()
        .map(|(index, prop)| {
            let decided = if prop.exact {
                last_type_arg(&prop.ty)
            } else if prop.within {
                option_inner(&prop.ty).and_then(last_type_arg)
            } else {
                None
            };
            let Some(param) = decided.and_then(own_param_ident) else {
                return false;
            };
            props
                .iter()
                .enumerate()
                .any(|(other, prop)| other != index && mentions_bare(&prop.ty, &param))
        })
        .collect();
    for (prop, mentioned) in props.iter_mut().zip(mentioned) {
        prop.exact &= !mentioned;
        prop.within &= !mentioned;
    }

    let designated: Vec<usize> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop.is_children_slot)
        .map(|(index, _)| index)
        .collect();
    if let [_, second, ..] = designated.as_slice() {
        return Err(syn::Error::new_spanned(
            &props[*second].ident,
            format!("component `{name}` already marks another prop `#[prop(children)]`"),
        ));
    }
    let named_children = props.iter().position(|prop| prop.ident == "children");
    if let (Some(slot), Some(children)) = (designated.first().copied(), named_children)
        && slot != children
    {
        return Err(syn::Error::new_spanned(
            &props[slot].ident,
            format!(
                "component `{name}` already takes its children through `children`, so `{}` cannot be `#[prop(children)]`",
                props[slot].ident,
            ),
        ));
    }
    let children_slot = designated.first().copied().or(named_children);
    if let Some(slot) = children_slot
        && props[slot].bare
    {
        let prop = &props[slot];
        return Err(syn::Error::new_spanned(
            &prop.ty,
            format!(
                "`{}` takes this tag's children, so its type says which children it takes: give it a slot type such as `Children<C>`, a child type, or a render (`RenderFn<H, C>`, `BuildFn<C>`), not a bare type parameter",
                prop.ident,
            ),
        ));
    }
    // Every prop is a typestate parameter of the builder: `MissingProp` until a setter sets
    // it. The builder struct and its setter impls carry no bounds of the component's, which
    // go on each method instead, so a bound the tag does not meet is reported as that bound
    // (its own message) rather than as a method that is missing.
    let state_params: Vec<Ident> = props
        .iter()
        .map(|prop| format_ident!("Prop{}", camel_case(&prop.ident)))
        .collect();
    let req_traits: Vec<Ident> = props
        .iter()
        .map(|prop| {
            format_ident!(
                "__{}_needs_{}",
                base,
                syn::ext::IdentExt::unraw(&prop.ident)
            )
        })
        .collect();
    let out_param = format_ident!("__Out");

    let req_trait_defs = props
        .iter()
        .zip(&req_traits)
        .filter(|(prop, _)| prop.default.is_none())
        .map(|(given, trait_ident)| {
            let prop = given.ident.to_string();
            let message = format!("missing required prop `{prop}` on component `{name}`");
            let label = format!("this tag does not set `{prop}`");
            let note = format!(
                "write `{prop}=...` in the tag, or give the prop a `#[prop(default = ...)]`"
            );
            // A bare prop is its own typestate, so a tag that leaves it out infers its type
            // parameter as `MissingProp`. Any other is kept as a `SetProp` once set, and left
            // out it is its type's `AbsentProp`, if the type has one and the prop is not
            // `#[prop(required)]`.
            let impls = if given.bare {
                quote! {
                    #[doc(hidden)]
                    impl<T> #trait_ident<T> for T {
                        fn __prop(self) -> T {
                            self
                        }
                    }
                }
            } else {
                let absent = (!given.required).then(|| quote! {
                    #[doc(hidden)]
                    #[diagnostic::do_not_recommend]
                    impl<T: ::__beui_tree::reactive::AbsentProp> #trait_ident<T> for ::__beui_tree::reactive::macro_support::MissingProp {
                        fn __prop(self) -> T {
                            <T as ::__beui_tree::reactive::AbsentProp>::absent()
                        }
                    }
                });
                quote! {
                    #[doc(hidden)]
                    #[diagnostic::do_not_recommend]
                    impl<T> #trait_ident<T> for ::__beui_tree::reactive::macro_support::SetProp<T> {
                        fn __prop(self) -> T {
                            self.0
                        }
                    }
                    #absent
                }
            };
            quote! {
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                #[diagnostic::on_unimplemented(message = #message, label = #label, note = #note)]
                #vis trait #trait_ident<T> {
                    fn __prop(self) -> T;
                }

                #impls
            }
        });

    let missing = quote! { ::__beui_tree::reactive::macro_support::MissingProp };
    let fields = props.iter().zip(&state_params).map(|(prop, param)| {
        let ident = &prop.ident;
        quote! { #ident: #param }
    });

    // The component's parameters without their inline bounds, which join its where clause.
    let mut params: Vec<(GenericParam, proc_macro2::TokenStream)> = outer;
    params.extend(
        sig.generics
            .params
            .iter()
            .map(|param| (param.clone(), generic_arg(param))),
    );
    params.sort_by_key(|(param, _)| !matches!(param, GenericParam::Lifetime(_)));
    let mut inline_bounds: Vec<WherePredicate> = Vec::new();
    let comp_params: Vec<GenericParam> = params
        .iter()
        .map(|(param, _)| match param {
            GenericParam::Type(param) => {
                let ident = &param.ident;
                let bounds = &param.bounds;
                if !bounds.is_empty() {
                    inline_bounds.push(syn::parse_quote! { #ident: #bounds });
                }
                syn::parse_quote! { #ident }
            }
            GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                let bounds = &param.bounds;
                if !bounds.is_empty() {
                    inline_bounds.push(syn::parse_quote! { #lifetime: #bounds });
                }
                syn::parse_quote! { #lifetime }
            }
            GenericParam::Const(param) => GenericParam::Const(param.clone()),
        })
        .collect();
    let lifetimes: Vec<_> = comp_params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(param) => Some(&param.lifetime),
            _ => None,
        })
        .collect();
    let type_params: Vec<_> = comp_params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(param) => Some(&param.ident),
            _ => None,
        })
        .collect();
    let phantom_ident = format_ident!("__generics");
    let phantom_field = (!lifetimes.is_empty() || !type_params.is_empty()).then(|| {
        quote! {
            #[allow(dead_code)]
            #phantom_ident: ::core::marker::PhantomData<(
                #(&#lifetimes (),)*
                #(fn() -> #type_params,)*
            )>,
        }
    });
    let phantom_init = phantom_field
        .is_some()
        .then(|| quote! { #phantom_ident: ::core::marker::PhantomData, });

    let field_idents: Vec<Ident> = props
        .iter()
        .map(|prop| prop.ident.clone())
        .chain([format_ident!("with_built")])
        .chain(phantom_field.is_some().then(|| phantom_ident.clone()))
        .collect();

    let comp_args: Vec<proc_macro2::TokenStream> = comp_params.iter().map(generic_arg).collect();
    let constructor_args: Vec<proc_macro2::TokenStream> =
        params.iter().map(|(_, arg)| arg.clone()).collect();
    let where_predicates: Vec<WherePredicate> = inline_bounds
        .into_iter()
        .chain(
            sig.generics
                .where_clause
                .iter()
                .flat_map(|clause| clause.predicates.iter().cloned()),
        )
        .chain(outer_where)
        .collect();
    let where_clause =
        (!where_predicates.is_empty()).then(|| quote! { where #(#where_predicates),* });
    let decl_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain([quote! { #out_param }])
        .chain(
            state_params
                .iter()
                .map(|param| quote! { #param = #missing }),
        )
        .collect();
    let decl_generics = angle_bracketed(&decl_params);
    let all_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain([quote! { #out_param }])
        .chain(state_params.iter().map(|param| quote! { #param }))
        .collect();
    let all_generics = angle_bracketed(&all_params);
    let built_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain(state_params.iter().map(|param| quote! { #param }))
        .collect();
    let built_generics = angle_bracketed(&built_params);
    let state_args = |out: proc_macro2::TokenStream| -> proc_macro2::TokenStream {
        let args: Vec<proc_macro2::TokenStream> = comp_args
            .iter()
            .cloned()
            .chain([out])
            .chain(state_params.iter().map(|param| quote! { #param }))
            .collect();
        angle_bracketed(&args)
    };
    let all_args = state_args(quote! { #out_param });
    let built_args = state_args(output_ty.clone());
    let builder_all = quote! { #builder_ident #all_args };
    let builder_built = quote! { #builder_ident #built_args };
    let constructor_args: Vec<proc_macro2::TokenStream> = constructor_args
        .into_iter()
        .chain([output_ty.clone()])
        .collect();
    let constructor_args = angle_bracketed(&constructor_args);
    let constructor_ty = quote! { #builder_ident #constructor_args };
    // A method's own where clause: what it asks, then the component's bounds.
    let method_where = |own: Vec<proc_macro2::TokenStream>| -> proc_macro2::TokenStream {
        if own.is_empty() && where_predicates.is_empty() {
            quote! {}
        } else {
            quote! { where #(#own,)* #(#where_predicates,)* }
        }
    };

    let init_fields = props.iter().map(|prop| {
        let ident = &prop.ident;
        quote! { #ident: #missing }
    });

    let setter_impls = props
        .iter()
        .enumerate()
        .map(|(slot, prop)| {
            let ident = &prop.ident;
            let ty = &prop.ty;
            let others: Vec<&Ident> = field_idents
                .iter()
                .filter(|field| *field != ident)
                .collect();
            let impl_params: Vec<proc_macro2::TokenStream> = comp_params
                .iter()
                .map(|param| quote! { #param })
                .chain([quote! { #out_param }])
                .chain(
                    state_params
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != slot)
                        .map(|(_, param)| quote! { #param }),
                )
                .collect();
            let impl_generics = angle_bracketed(&impl_params);
            let with_state = |state: proc_macro2::TokenStream| -> proc_macro2::TokenStream {
                let args: Vec<proc_macro2::TokenStream> = comp_args
                    .iter()
                    .cloned()
                    .chain([quote! { #out_param }])
                    .chain(state_params.iter().enumerate().map(|(other, param)| {
                        if other == slot {
                            state.clone()
                        } else {
                            quote! { #param }
                        }
                    }))
                    .collect();
                angle_bracketed(&args)
            };
            let unset_args = with_state(missing.clone());
            // The set state names the prop's type through `__Target`, a parameter of the
            // setter's equal to it (`Is`), so the setter's signature holds no projection of the
            // component's parameters: one whose bound a tag does not meet would make rustc
            // report the setter as missing rather than the bound.
            let (set_state, set_value) = if prop.bare && prop.default.is_none() {
                (quote! { #ty }, quote! { __prop_value })
            } else if prop.bare {
                (
                    quote! { ::__beui_tree::reactive::macro_support::SetProp<#ty> },
                    quote! { ::__beui_tree::reactive::macro_support::SetProp(__prop_value) },
                )
            } else {
                (
                    quote! { ::__beui_tree::reactive::macro_support::SetProp<__Target> },
                    quote! { ::__beui_tree::reactive::macro_support::SetProp(::__beui_tree::reactive::macro_support::Is::is(__prop_value)) },
                )
            };
            let set_args = with_state(set_state);

            let methods = prop_setters(prop, children_slot == Some(slot))
                .into_iter()
                .map(|setter| {
                    let Setter {
                        method,
                        generics,
                        args,
                        where_clause,
                        value,
                    } = setter;
                    let (generics, where_clause) = if prop.bare {
                        (angle_bracketed(&generics), method_where(where_clause))
                    } else {
                        let mut generics = generics;
                        generics.push(quote! { __Target });
                        let mut where_clause = where_clause;
                        where_clause.push(quote! { #ty: ::__beui_tree::reactive::macro_support::Is<__Target> });
                        (angle_bracketed(&generics), method_where(where_clause))
                    };
                    let others = &others;
                    quote! {
                        pub fn #method #generics (self, #args)
                            -> #builder_ident #set_args
                        #where_clause
                        {
                            let __prop_value = #value;
                            let #builder_ident { #ident: _, #(#others,)* } = self;
                            #builder_ident { #ident: #set_value, #(#others,)* }
                        }
                    }
                });

            quote! {
                impl #impl_generics #builder_ident #unset_args {
                    #(#methods)*
                }
            }
        })
        .collect::<Vec<_>>();

    // A function component settles its props inside its scope, so what a missing one makes (a
    // binding's signal) belongs to the component. An associated component settles them in the
    // caller's scope, just before calling its hidden method, so that belongs to the caller.
    // A default runs untracked: a read in it is a one-time snapshot, not a tracked read in the
    // component's body.
    let field_takes = props.iter().map(|prop| {
        let ident = &prop.ident;
        quote! { let #ident = self.#ident; }
    });
    let field_lets: Vec<proc_macro2::TokenStream> = props
        .iter()
        .zip(&state_params)
        .zip(&req_traits)
        .map(|((prop, param), trait_ident)| {
            let ident = &prop.ident;
            let ty = &prop.ty;
            let value = match &prop.default {
                None => quote! { <#param as #trait_ident<#ty>>::__prop(#ident) },
                Some(default) => {
                    let default = default_value(prop, default);
                    quote! {
                        <#param as ::__beui_tree::reactive::macro_support::PropOr<#ty>>::prop_or(#ident, || ::__beui_tree::reactive::untrack(|| #default))
                    }
                }
            };
            quote! { let #ident = #value; }
        })
        .collect();

    let build_where = method_where(
        props
            .iter()
            .zip(&state_params)
            .zip(&req_traits)
            .map(|((prop, param), trait_ident)| {
                let ty = &prop.ty;
                match prop.default {
                    None => quote! { #param: #trait_ident<#ty> },
                    Some(_) => {
                        quote! { #param: ::__beui_tree::reactive::macro_support::PropOr<#ty> }
                    }
                }
            })
            .chain([
                quote! { #out_param: ::__beui_tree::reactive::macro_support::SameType<#output_ty> },
            ])
            .collect(),
    );

    let finish = match finish {
        Finish::Body(block) => quote! {
            ::__beui_tree::reactive::component(&::__beui_tree::reactive::ComponentInfo { name: #name, module: module_path!() }, move || {
                #(#field_lets)*
                ::__beui_tree::reactive::ComponentOutput::<#output_ty>::into_output(#block)
            })
        },
        Finish::Call(call) => {
            let idents = props.iter().map(|prop| &prop.ident);
            quote! {
                {
                    #(#field_lets)*
                    #call(#(#idents),*)
                }
            }
        }
    };

    let constructor_body = quote! {
        #builder_ident {
            #(#init_fields,)*
            with_built: ::std::vec::Vec::new(),
            #phantom_init
        }
    };

    let required_checks = props.iter().filter(|prop| prop.required).map(|prop| {
        let ty = &prop.ty;
        let check = format_ident!(
            "__{}_required_{}",
            base,
            syn::ext::IdentExt::unraw(&prop.ident)
        );
        let generics: Vec<proc_macro2::TokenStream> =
            comp_params.iter().map(|param| quote! { #param }).collect();
        let generics = angle_bracketed(&generics);
        let marked = quote_spanned! { ty.span() =>
            marked::<#ty>();
        };
        quote! {
            #[allow(non_snake_case, dead_code)]
            fn #check #generics () #where_clause {
                fn marked<T: ?Sized + ::__beui_tree::reactive::MarkedRequired>() {}
                #marked
            }
        }
    });

    // Children written between the tags of a component with no slot for them.
    let no_children = children_slot.is_none().then(|| {
        quote! {
            impl #all_generics #builder_all {
                #[doc(hidden)]
                pub fn __children<__Make>(self, _make: __Make) -> Self
                where
                    __Make: ::__beui_tree::reactive::macro_support::TakesNoChildren,
                {
                    self
                }
            }
        }
    });

    // Children written between the tags of a tag that set the children slot as a prop too:
    // `__children` on the state where the slot is set asks for a trait nothing implements, so
    // it is reported with this message rather than as a missing method.
    let children_twice = children_slot.map(|slot| {
        let prop = props[slot].ident.to_string();
        let trait_ident = format_ident!("__{}_children_set_twice", base);
        let message = format!(
            "`{prop}` is set both as a prop and as this tag's children, write it one way or the other"
        );
        let label = format!("these children set `{prop}` again");
        let impl_params: Vec<proc_macro2::TokenStream> = comp_params
            .iter()
            .map(|param| quote! { #param })
            .chain([quote! { #out_param }])
            .chain(
                state_params
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != slot)
                    .map(|(_, param)| quote! { #param }),
            )
            .chain([quote! { __Set }])
            .collect();
        let impl_generics = angle_bracketed(&impl_params);
        let args: Vec<proc_macro2::TokenStream> = comp_args
            .iter()
            .cloned()
            .chain([quote! { #out_param }])
            .chain(state_params.iter().enumerate().map(|(other, param)| {
                if other == slot {
                    quote! { ::__beui_tree::reactive::macro_support::SetProp<__Set> }
                } else {
                    quote! { #param }
                }
            }))
            .collect();
        let args = angle_bracketed(&args);
        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #[diagnostic::on_unimplemented(message = #message, label = #label)]
            #vis trait #trait_ident {}

            impl #impl_generics #builder_ident #args {
                #[doc(hidden)]
                pub fn __children<__Make>(self, _make: __Make) -> Self
                where
                    __Make: #trait_ident,
                {
                    self
                }
            }
        }
    });

    let items = quote! {
        #(#req_trait_defs)*

        #(#required_checks)*

        #(#attrs)*
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #builder_ident #decl_generics {
            #(#fields,)*
            with_built: ::std::vec::Vec<::std::boxed::Box<dyn ::core::ops::FnOnce(&#out_param)>>,
            #phantom_field
        }

        impl #built_generics ::__beui_tree::reactive::ComponentBuilder for #builder_built #where_clause {
            type Output = #output_ty;

            fn with_built(
                mut self,
                bind: impl ::core::ops::FnOnce(&#output_ty) + 'static,
            ) -> Self {
                self.with_built.push(::std::boxed::Box::new(bind));
                self
            }
        }

        #(#setter_impls)*

        #no_children

        #children_twice

        impl #all_generics #builder_all {
            #[track_caller]
            pub fn build(self) -> #out_param #build_where {
                let with_built = <#out_param as ::__beui_tree::reactive::macro_support::SameType<#output_ty>>::same(self.with_built);
                #(#field_takes)*
                let node: #output_ty = #finish;
                for bind in with_built {
                    bind(&node);
                }
                <#out_param as ::__beui_tree::reactive::macro_support::SameType<#output_ty>>::from_output(node)
            }
        }
    };
    Ok(ExpandedComponent {
        items,
        constructor_ty,
        constructor_body,
    })
}

struct ViewAttr {
    key: Ident,
    value: Expr,
}

fn parse_unbraced_value(input: ParseStream) -> syn::Result<Expr> {
    if input.peek(Token![&]) {
        let and_token = input.parse::<Token![&]>()?;
        let mutability: Option<Token![mut]> = input.parse()?;
        let expr = parse_unbraced_value(input)?;
        return Ok(Expr::Reference(ExprReference {
            attrs: Vec::new(),
            and_token,
            mutability,
            expr: Box::new(expr),
        }));
    }

    if input.peek(Token![-]) {
        let minus = input.parse::<Token![-]>()?;
        let expr = parse_unbraced_value(input)?;
        return Ok(Expr::Unary(ExprUnary {
            attrs: Vec::new(),
            op: UnOp::Neg(minus),
            expr: Box::new(expr),
        }));
    }

    if input.peek(Token![!]) {
        let bang = input.parse::<Token![!]>()?;
        let expr = parse_unbraced_value(input)?;
        return Ok(Expr::Unary(ExprUnary {
            attrs: Vec::new(),
            op: UnOp::Not(bang),
            expr: Box::new(expr),
        }));
    }

    if input.peek(syn::token::Paren) {
        let unit;
        let paren_token = parenthesized!(unit in input);
        if unit.is_empty() {
            return Ok(Expr::Tuple(syn::ExprTuple {
                attrs: Vec::new(),
                paren_token,
                elems: Default::default(),
            }));
        }
        return Err(unit.error("write an expression in parentheses as a braced value: `={(...)}`"));
    }

    if input.peek(Lit) || input.peek(syn::LitBool) {
        let lit: Lit = input.parse()?;
        return Ok(Expr::Lit(ExprLit {
            attrs: Vec::new(),
            lit,
        }));
    }

    if input.peek(Ident)
        || input.peek(Token![::])
        || input.peek(Token![crate])
        || input.peek(Token![self])
        || input.peek(Token![Self])
        || input.peek(Token![super])
    {
        let path = Path::parse_mod_style(input)?;
        let func = Expr::Path(ExprPath {
            attrs: Vec::new(),
            qself: None,
            path,
        });
        if input.peek(syn::token::Paren) {
            let args;
            let paren_token = parenthesized!(args in input);
            return Ok(Expr::Call(ExprCall {
                attrs: Vec::new(),
                func: Box::new(func),
                paren_token,
                args: args.parse_terminated(Expr::parse, Token![,])?,
            }));
        }
        return Ok(func);
    }

    Err(input.error(
        "expected a literal, a path, a path call, `()`, or a braced expression `={...}` for an attribute value",
    ))
}

impl Parse for ViewAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if !input.peek(Token![=]) {
            let value = Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: None,
                path: Path::from(key.clone()),
            });
            return Ok(ViewAttr { key, value });
        }
        input.parse::<Token![=]>()?;
        let value = if input.peek(Token![<]) {
            let child = input.parse::<ViewChild>()?;
            syn::parse2(expand_child(&child))?
        } else if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            content.parse::<Expr>()?
        } else {
            parse_unbraced_value(input)?
        };
        if input.peek(Token![.]) || input.peek(Token![?]) || input.peek(syn::token::Bracket) {
            return Err(input.error(format!(
                "wrap a value that goes on past a literal, a path or a call in braces: `{key}={{...}}`"
            )));
        }
        Ok(ViewAttr { key, value })
    }
}

struct ViewModifier {
    path: Path,
    value: Expr,
}

impl Parse for ViewModifier {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![@]>()?;
        let path = input.call(Path::parse_mod_style)?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            if input.peek(syn::token::Brace) {
                let content;
                braced!(content in input);
                content.parse::<Expr>()?
            } else {
                parse_unbraced_value(input)?
            }
        } else {
            Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: None,
                path: path.clone(),
            })
        };
        if input.peek(Token![.]) || input.peek(Token![?]) || input.peek(syn::token::Bracket) {
            return Err(input.error(
                "wrap a modifier value that goes on past a literal, a path or a call in braces",
            ));
        }
        Ok(Self { path, value })
    }
}

fn parse_modifiers(input: ParseStream) -> syn::Result<Vec<ViewModifier>> {
    let mut modifiers = Vec::new();
    while input.peek(Token![@]) {
        modifiers.push(input.parse()?);
    }
    Ok(modifiers)
}

fn apply_modifiers(
    mut built: proc_macro2::TokenStream,
    modifiers: &[ViewModifier],
) -> proc_macro2::TokenStream {
    for modifier in modifiers {
        let path = &modifier.path;
        let value = &modifier.value;
        built = quote_spanned! { path.span() => #path(#built, #value) };
    }
    built
}

enum ViewChildKind {
    Fragment(Vec<ViewChild>),
    Node(ViewNode),
    Expr(Expr, Vec<ViewModifier>),
}

struct ViewChild {
    kind: ViewChildKind,
}

impl Parse for ViewChild {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kind = if input.peek(Token![<]) && input.peek2(Token![>]) {
            input.parse::<Token![<]>()?;
            input.parse::<Token![>]>()?;
            let mut children = Vec::new();
            while !(input.peek(Token![<]) && input.peek2(Token![/])) {
                children.push(input.parse::<ViewChild>()?);
            }
            input.parse::<Token![<]>()?;
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            ViewChildKind::Fragment(children)
        } else if input.peek(Token![<]) {
            ViewChildKind::Node(input.parse()?)
        } else if input.peek(syn::LitStr) {
            let lit: syn::LitStr = input.parse()?;
            ViewChildKind::Expr(
                Expr::Lit(ExprLit {
                    attrs: Vec::new(),
                    lit: Lit::Str(lit),
                }),
                parse_modifiers(input)?,
            )
        } else {
            let content;
            braced!(content in input);
            let expr = content.parse()?;
            ViewChildKind::Expr(expr, parse_modifiers(input)?)
        };

        Ok(ViewChild { kind })
    }
}

struct ViewNode {
    tag_path: Vec<Ident>,
    props: Vec<ViewAttr>,
    modifiers: Vec<ViewModifier>,
    head: Option<ViewHead>,
    children: Option<Vec<ViewChild>>,
}

struct ViewHead {
    span: proc_macro2::Span,
    pattern: Option<Pat>,
    block: Option<syn::Block>,
}

fn parse_view_head(input: ParseStream) -> syn::Result<Option<ViewHead>> {
    let (span, pattern) = if input.peek(Token![||]) {
        (input.parse::<Token![||]>()?.spans[0], None)
    } else if input.peek(Token![|]) {
        let open = input.parse::<Token![|]>()?;
        let mut params = vec![parse_head_param(input)?];
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.peek(Token![|]) {
                break;
            }
            params.push(parse_head_param(input)?);
        }
        input.parse::<Token![|]>()?;
        let pattern = match params.len() {
            1 => params.pop().expect("one parameter").0,
            _ => head_tuple(params, open.span)?,
        };
        (open.span, Some(pattern))
    } else {
        return Ok(None);
    };
    let block = if input.peek(syn::token::Brace) {
        let fork = input.fork();
        fork.parse::<syn::Block>()?;
        if fork.peek(Token![<]) && fork.peek2(Token![/]) {
            Some(input.parse::<syn::Block>()?)
        } else {
            None
        }
    } else {
        None
    };
    Ok(Some(ViewHead {
        span,
        pattern,
        block,
    }))
}

/// One parameter of a closure child, with its type when it is written.
fn parse_head_param(input: ParseStream) -> syn::Result<(Pat, Option<Type>)> {
    let pattern = Pat::parse_single(input)?;
    if !input.peek(Token![:]) {
        return Ok((pattern, None));
    }
    let colon_token = input.parse::<Token![:]>()?;
    let ty: Type = input.parse()?;
    Ok((
        Pat::Type(PatType {
            attrs: Vec::new(),
            pat: Box::new(pattern),
            colon_token,
            ty: Box::new(ty.clone()),
        }),
        Some(ty),
    ))
}

/// A closure child of several parameters (`|item: T, index: ReadSignal<usize>|`) takes what
/// it is handed as one tuple: `|(item, index): (T, ReadSignal<usize>)|`.
fn head_tuple(params: Vec<(Pat, Option<Type>)>, span: proc_macro2::Span) -> syn::Result<Pat> {
    let typed = params.iter().all(|(_, ty)| ty.is_some());
    if !typed && params.iter().any(|(_, ty)| ty.is_some()) {
        return Err(syn::Error::new(
            span,
            "give every parameter of this closure child its type, or none of them",
        ));
    }
    let mut pats = syn::punctuated::Punctuated::<Pat, Token![,]>::new();
    let mut types = syn::punctuated::Punctuated::<Type, Token![,]>::new();
    for (pat, ty) in params {
        let pat = match pat {
            Pat::Type(typed) => *typed.pat,
            pat => pat,
        };
        pats.push(pat);
        if let Some(ty) = ty {
            types.push(ty);
        }
    }
    let tuple = Pat::Tuple(syn::PatTuple {
        attrs: Vec::new(),
        paren_token: Default::default(),
        elems: pats,
    });
    if !typed {
        return Ok(tuple);
    }
    Ok(Pat::Type(PatType {
        attrs: Vec::new(),
        pat: Box::new(tuple),
        colon_token: Default::default(),
        ty: Box::new(Type::Tuple(syn::TypeTuple {
            paren_token: Default::default(),
            elems: types,
        })),
    }))
}

fn parse_tag_path(input: ParseStream) -> syn::Result<Vec<Ident>> {
    let mut segments = vec![input.parse::<Ident>()?];
    while input.peek(Token![::]) {
        input.parse::<Token![::]>()?;
        segments.push(input.parse::<Ident>()?);
    }
    Ok(segments)
}

fn tag_path_string(tag_path: &[Ident]) -> String {
    tag_path
        .iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

impl Parse for ViewNode {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![<]>()?;
        let tag_path = parse_tag_path(input)?;

        let mut props: Vec<ViewAttr> = Vec::new();
        let mut modifiers = Vec::new();
        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            if input.peek(Token![@]) {
                modifiers.push(input.parse::<ViewModifier>()?);
                continue;
            }
            let attr = input.parse::<ViewAttr>()?;
            if props.iter().any(|earlier| earlier.key == attr.key) {
                return Err(syn::Error::new(
                    attr.key.span(),
                    format!("prop `{}` is set more than once on this tag", attr.key),
                ));
            }
            props.push(attr);
        }

        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(ViewNode {
                tag_path,
                props,
                modifiers,
                head: None,
                children: None,
            });
        }
        input.parse::<Token![>]>()?;

        let head = parse_view_head(input)?;
        let mut children = Vec::new();
        while !(input.peek(Token![<]) && input.peek2(Token![/])) {
            children.push(input.parse::<ViewChild>()?);
        }

        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let close_tag_path = parse_tag_path(input)?;
        if tag_path_string(&close_tag_path) != tag_path_string(&tag_path) {
            let close_tag = tag_path_string(&close_tag_path);
            let tag = tag_path_string(&tag_path);
            return Err(syn::Error::new(
                close_tag_path.last().unwrap().span(),
                format!("mismatched closing tag `</{close_tag}>`, expected `</{tag}>`"),
            ));
        }
        input.parse::<Token![>]>()?;

        if let Some(attr) = props.iter().find(|attr| attr.key == "children") {
            return Err(syn::Error::new(
                attr.key.span(),
                "`children` is set both as a prop and as this tag's children, write it one way or the other",
            ));
        }

        if let [
            ViewChild {
                kind: ViewChildKind::Expr(Expr::Closure(closure), _),
            },
        ] = children.as_slice()
        {
            return Err(syn::Error::new_spanned(
                closure,
                "write a closure child without its braces and `move`, as `|value| <Tag />` or `|value| { ... }`",
            ));
        }

        Ok(ViewNode {
            tag_path,
            props,
            modifiers,
            head,
            children: Some(children),
        })
    }
}

fn expand_child(child: &ViewChild) -> proc_macro2::TokenStream {
    match &child.kind {
        ViewChildKind::Fragment(children) => {
            if children.is_empty() {
                quote! { ::__beui_tree::reactive::Children::default() }
            } else {
                let items = expand_child_items(children);
                quote! { ::__beui_tree::reactive::Children::from([#(#items),*]) }
            }
        }
        ViewChildKind::Node(node) => expand_view_node(node),
        ViewChildKind::Expr(expr, modifiers) => apply_modifiers(quote! { #expr }, modifiers),
    }
}

fn expand_child_items(children: &[ViewChild]) -> Vec<proc_macro2::TokenStream> {
    children
        .iter()
        .map(|child| {
            let built = expand_child(child);
            quote! { ::__beui_tree::reactive::into_segment(#built) }
        })
        .collect()
}

fn expand_child_block(children: &[ViewChild]) -> proc_macro2::TokenStream {
    let [child] = children else {
        let items = expand_child_items(children);
        return quote! { [#(#items),*] };
    };
    let built = expand_child(child);
    quote! { ::__beui_tree::reactive::SingleChild(#built) }
}

fn expand_view_node(node: &ViewNode) -> proc_macro2::TokenStream {
    let mut segments = node.tag_path.clone();
    let last = segments.pop().expect("tag path always has one segment");
    let component_path = if segments.is_empty() {
        quote! { #last }
    } else {
        quote! { #(#segments::)* #last }
    };
    let setters = node
        .props
        .iter()
        .map(|prop| {
            let key = &prop.key;
            let value = &prop.value;
            quote_spanned! { key.span() => .#key(#value) }
        })
        .collect::<Vec<_>>();

    let tag = last.span();
    let build = format_ident!("build", span = tag);
    let builder = quote_spanned! { tag => #component_path() #(#setters)* };
    apply_modifiers(expand_view_body(node, builder, build, tag), &node.modifiers)
}

/// A tag's children reach its children slot's one setter, `__children`, wrapped by the shape
/// they are written in; the slot's type says which shapes it takes.
fn expand_view_body(
    node: &ViewNode,
    builder: proc_macro2::TokenStream,
    build: Ident,
    tag: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    let Some(children) = &node.children else {
        return quote_spanned! { tag => #builder .#build() };
    };
    let (span, wrapped) = match &node.head {
        Some(head) => {
            let span = head.span;
            let params = match &head.pattern {
                Some(pattern) => quote! { |#pattern| },
                None => quote! { || },
            };
            match (&head.block, &head.pattern) {
                (Some(block), Some(_)) => (
                    span,
                    quote_spanned! { span => __slot.closure(move #params #block) },
                ),
                (Some(block), None) => (
                    span,
                    quote_spanned! { span => ::__beui_tree::reactive::macro_support::ChildUnitClosure(move #params #block) },
                ),
                (None, Some(_)) => {
                    let items = expand_child_block(children);
                    (
                        span,
                        quote_spanned! { span => __slot.with(move #params #items) },
                    )
                }
                (None, None) => {
                    let items = expand_child_block(children);
                    (
                        tag,
                        quote_spanned! { tag => ::__beui_tree::reactive::macro_support::ChildBlock(move || #items) },
                    )
                }
            }
        }
        None => {
            let items = expand_child_block(children);
            (
                tag,
                quote_spanned! { tag => ::__beui_tree::reactive::macro_support::ChildBlock(move || #items) },
            )
        }
    };
    let set = format_ident!("__children", span = span);
    quote_spanned! { span =>
        #builder .#set(|__slot| {
            #[allow(unused_imports)]
            use ::__beui_tree::reactive::macro_support::{FnOnceSlot as _, FnSlot as _, NoHandedSlot as _};
            #wrapped
        }) .#build()
    }
}

struct View {
    roots: Vec<ViewChild>,
}

impl Parse for View {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut roots = Vec::new();
        while !input.is_empty() {
            roots.push(input.parse::<ViewChild>()?);
        }
        Ok(View { roots })
    }
}

fn expand_view(view: &View) -> proc_macro2::TokenStream {
    if let [root] = view.roots.as_slice() {
        return expand_child(root);
    }
    if view.roots.is_empty() {
        return quote! { ::__beui_tree::reactive::Children::default() };
    }
    let items = expand_child_items(&view.roots);
    quote! { ::__beui_tree::reactive::Children::from([#(#items),*]) }
}

#[proc_macro_attribute]
pub fn sample(attr: TokenStream, item: TokenStream) -> TokenStream {
    parse_macro_input!(attr as syn::parse::Nothing);
    let source = written_source(item.clone())
        .unwrap_or_else(|| "the compiler gave no source text for this sample".to_owned());
    let tokens = proc_macro2::TokenStream::from(item.clone());
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let vis = &function.vis;
    quote! {
        #tokens

        #[allow(non_snake_case)]
        #vis mod #name {
            pub const SOURCE: &str = #source;
        }
    }
    .into()
}

fn written_source(item: TokenStream) -> Option<String> {
    let mut pieces = Vec::new();
    for token in item {
        let span = token.span();
        let start = span.start();
        let end = span.end();
        pieces.push((
            (start.line(), start.column()),
            (end.line(), end.column()),
            span.source_text()?,
        ));
    }
    let first_column = pieces.first()?.0.1;
    let shift = pieces
        .last()
        .map_or(0, |(start, end, text)| match end.0 > start.0 {
            true => text
                .lines()
                .last()
                .map_or(0, |line| line.len() - line.trim_start().len()),
            false => 0,
        });
    let mut written = String::new();
    let mut previous: Option<(usize, usize)> = None;
    for (start, end, text) in pieces {
        match previous {
            Some((line, _)) if start.0 > line => {
                written.push_str(&"\n".repeat(start.0 - line));
                let indent = shift + start.1.saturating_sub(first_column);
                written.push_str(&" ".repeat(indent));
            }
            Some((_, column)) => written.push_str(&" ".repeat(start.1.saturating_sub(column))),
            None => {}
        }
        written.push_str(&text);
        previous = Some(end);
    }
    let dedented: Vec<&str> = written
        .lines()
        .map(|line| {
            let indent = line.len() - line.trim_start().len();
            &line[indent.min(shift)..]
        })
        .collect();
    Some(dedented.join("\n"))
}

#[cfg(test)]
mod tests;

/// Builds a tree once, the moment it runs.
///
/// - An attribute value in braces is any expression. Literals, paths, a call on a path, a
///   unary expression and a reference to a path may drop the braces. A bare name is
///   shorthand for `name={name}`; there is no boolean shorthand. A `String` prop takes a
///   string literal.
/// - `@path=value` is a modifier: `path(built, value)` runs after all props and children
///   are built, in source order, and may change the type; its value is evaluated then too,
///   so compute one before `view!` if a child closure will move what it needs. Without
///   `=` the path is its own value. A modifier works on the node; how a parent places a child
///   (a list child's length, a grid cell's span) is a wrapping tag, `<ListItem grow=1.0>` or
///   `<GridCell span=2>`, which builds the child its parent takes.
/// - A slot that hands over a value names it after the opening tag, as a closure does
///   (`<Container>|size| <Card size /></Container>`, or `|size| { ...; view! {..} }` for
///   statements first). Tags without `|value|` ignore it. The closure is always `move`, and
///   a `RenderFn` slot may run it again, so clone what it hands on. The body is exactly one
///   child unless the slot returns `Children<T>`.
/// - More than one root makes a `Children`; `view! {}` is the empty one.
#[proc_macro]
pub fn view(item: TokenStream) -> TokenStream {
    let view = parse_macro_input!(item as View);
    let expanded = expand_view(&view);
    resolve_tree(quote! { { #expanded } }).into()
}

fn resolve_tree(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro_crate::{FoundCrate, crate_name};
    // beui-documents is found by its package name and answers with the name it is depended on
    // under (`beui`), as the app runtime does, rather than through the fallback below.
    let target = [
        "beui",
        "beui-documents",
        "beui-view",
        "beui-html",
        "beui-tree",
    ]
    .into_iter()
    .find_map(|name| crate_name(name).ok().map(|found| (name, found)))
    .map(|(name, found)| match found {
        FoundCrate::Itself
            if std::env::var("CARGO_CRATE_NAME").ok().as_deref()
                == Some(name.replace('-', "_").as_str()) =>
        {
            "crate".to_string()
        }
        FoundCrate::Itself => name.replace('-', "_"),
        FoundCrate::Name(name) => name,
    });
    let Some(target) = target else {
        return rewrite_tree(tokens, "beui");
    };
    rewrite_tree(tokens, &target)
}

fn rewrite_tree(tokens: proc_macro2::TokenStream, target: &str) -> proc_macro2::TokenStream {
    use proc_macro2::{Group, Ident, TokenTree};
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut output = proc_macro2::TokenStream::new();
    let mut index = 0;
    while index < tokens.len() {
        if index + 4 < tokens.len()
            && tokens[index].to_string() == ":"
            && tokens[index + 1].to_string() == ":"
            && tokens[index + 2].to_string() == "__beui_tree"
            && tokens[index + 3].to_string() == ":"
            && tokens[index + 4].to_string() == ":"
        {
            if target != "crate" {
                output.extend(tokens[index..index + 2].iter().cloned());
            }
            output.extend([TokenTree::Ident(Ident::new(
                target,
                tokens[index + 2].span(),
            ))]);
            index += 3;
            continue;
        }
        let token = match &tokens[index] {
            TokenTree::Group(group) => {
                let mut new = Group::new(group.delimiter(), rewrite_tree(group.stream(), target));
                new.set_span(group.span());
                TokenTree::Group(new)
            }
            other => other.clone(),
        };
        output.extend([token]);
        index += 1;
    }
    output
}
