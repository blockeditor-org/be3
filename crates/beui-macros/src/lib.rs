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
    inner_ty: Option<Type>,
    optional_reactive_inner_ty: Option<Type>,
    reactive_inner_ty: Option<Type>,
    binding_inner_ty: Option<Type>,
    render: Option<Render>,
    optional_render: Option<Render>,
    func_args: Option<Vec<Type>>,
    optional_func_args: Option<Vec<Type>>,
    is_children: bool,
    is_child: bool,
    is_optional_child: bool,
    callback_signature: Option<proc_macro2::TokenStream>,
    optional_callback_signature: Option<proc_macro2::TokenStream>,
    is_click_callback: bool,
    is_optional_click_callback: bool,
    default: Option<Expr>,
    is_children_slot: bool,
}

struct Render {
    once: bool,
    handle: Option<Type>,
    child: Option<Type>,
}

fn render_kind(ty: &Type) -> Option<Render> {
    let once = if is_named_type(ty, "Render") {
        true
    } else if is_named_type(ty, "RenderFn") {
        false
    } else {
        return None;
    };
    let name = if once { "Render" } else { "RenderFn" };
    let args = generic_args(ty, name).unwrap_or_default();
    let handle = args.first().cloned().filter(|handle| !is_unit_type(handle));
    Some(Render {
        once,
        handle,
        child: args.get(1).cloned(),
    })
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn render_child(render: &Render) -> proc_macro2::TokenStream {
    match &render.child {
        Some(child) => quote! { #child },
        None => quote! { ::__beui_tree::reactive::Child },
    }
}

fn callback_signature(ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    let args = generic_args(ty, "Callback").unwrap_or_default();
    match args.as_slice() {
        [value] => Ok(quote! { ::__beui_tree::reactive::IntoCallback<#value, ()> }),
        [value, result] => Ok(quote! { ::__beui_tree::reactive::IntoCallback<#value, #result> }),
        _ => Err(syn::Error::new_spanned(
            ty,
            "a `Callback` prop takes one or two type arguments, as `Callback<Value>` or `Callback<Value, Result>`",
        )),
    }
}

fn func_args(ty: &Type) -> Option<Vec<Type>> {
    if !is_named_type(ty, "Func") {
        return None;
    }
    let args = generic_args(ty, "Func")?;
    (args.len() == 2).then_some(args)
}

struct Setter {
    method: Ident,
    generics: proc_macro2::TokenStream,
    args: proc_macro2::TokenStream,
    where_clause: proc_macro2::TokenStream,
    value: proc_macro2::TokenStream,
}

fn func_setter(
    method: &Ident,
    args: &[Type],
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let (value, result) = (&args[0], &args[1]);
    Setter {
        method: method.clone(),
        generics: quote! {},
        args: quote! { value: impl ::__beui_tree::reactive::IntoFunc<#value, #result> },
        where_clause: quote! {},
        value: wrap(quote! { ::__beui_tree::reactive::IntoFunc::into_func(value) }),
    }
}

struct RenderOutput {
    child: proc_macro2::TokenStream,
    item: Option<proc_macro2::TokenStream>,
}

impl RenderOutput {
    fn built_bound(&self) -> proc_macro2::TokenStream {
        let child = &self.child;
        match &self.item {
            Some(item) => quote! { ::__beui_tree::reactive::IntoSegment<#item> },
            None => quote! { ::__beui_tree::reactive::IntoChild<#child> },
        }
    }

    fn block_bound(&self) -> proc_macro2::TokenStream {
        let child = &self.child;
        match &self.item {
            Some(item) => quote! { ::__beui_tree::reactive::IntoSegments<#item> },
            None => quote! { ::__beui_tree::reactive::OneChild<#child> },
        }
    }

    fn wrap_built(&self, built: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let child = &self.child;
        match &self.item {
            Some(_) => {
                quote! { ::__beui_tree::reactive::Children::from(::__beui_tree::reactive::into_segment(#built)) }
            }
            None => quote! { ::__beui_tree::reactive::into_child::<#child>(#built) },
        }
    }

    fn wrap_block(&self, block: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let child = &self.child;
        match &self.item {
            Some(_) => quote! { ::__beui_tree::reactive::Children::from_block(#block) },
            None => quote! { <_ as ::__beui_tree::reactive::OneChild<#child>>::one_child(#block) },
        }
    }
}

fn render_output(render: &Render) -> RenderOutput {
    let child = render_child(render);
    let item = render
        .child
        .as_ref()
        .filter(|ty| is_named_type(ty, "Children"))
        .map(|_| quote! { <#child as ::__beui_tree::reactive::ChildrenSlot>::Child });
    RenderOutput { child, item }
}

fn render_new(render: &Render) -> proc_macro2::TokenStream {
    if render.once {
        quote! { ::__beui_tree::reactive::Render::new }
    } else {
        quote! { ::__beui_tree::reactive::RenderFn::new }
    }
}

fn render_fn_bound(render: &Render, args: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if render.once {
        quote! { ::core::ops::FnOnce(#args) }
    } else {
        quote! { ::core::ops::Fn(#args) }
    }
}

fn render_setter(
    method: &Ident,
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let child = render_child(render);
    let (generics, signature, where_clause, build) = match (&render.handle, render.once) {
        (Some(handle), true) => (
            quote! {},
            quote! { ::__beui_tree::reactive::IntoRender<#handle, #child> },
            quote! {},
            quote! { ::__beui_tree::reactive::IntoRender::into_render(value) },
        ),
        (Some(handle), false) => (
            quote! {},
            quote! { ::__beui_tree::reactive::IntoRenderFn<#handle, #child> },
            quote! {},
            quote! { ::__beui_tree::reactive::IntoRenderFn::into_render_fn(value) },
        ),
        (None, _) => {
            let output = render_output(render);
            let bound = render_fn_bound(render, quote! {});
            let built_bound = output.built_bound();
            let new = render_new(render);
            let built = output.wrap_built(quote! { value() });
            (
                quote! { <Built> },
                quote! { #bound -> Built + 'static },
                quote! { where Built: #built_bound },
                quote! { #new(move |()| #built) },
            )
        }
    };
    Setter {
        method: method.clone(),
        generics,
        args: quote! { value: impl #signature },
        where_clause,
        value: wrap(build),
    }
}

fn render_children_closure(
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let output = render_output(render);
    let new = render_new(render);
    let (bound, where_clause, build) = match &render.handle {
        Some(handle) if output.item.is_none() => {
            let child = &output.child;
            let (into, convert) = if render.once {
                (
                    quote! { ::__beui_tree::reactive::IntoRender<#handle, #child> },
                    quote! { ::__beui_tree::reactive::IntoRender::into_render(value) },
                )
            } else {
                (
                    quote! { ::__beui_tree::reactive::IntoRenderFn<#handle, #child> },
                    quote! { ::__beui_tree::reactive::IntoRenderFn::into_render_fn(value) },
                )
            };
            let bound = render_fn_bound(render, quote! { #handle });
            (
                quote! { #bound -> Built + 'static + #into },
                quote! {},
                convert,
            )
        }
        Some(handle) => {
            let bound = render_fn_bound(render, quote! { #handle });
            let built_bound = output.built_bound();
            let built = output.wrap_built(quote! { value(handle) });
            (
                quote! { #bound -> Built + 'static },
                quote! { Built: #built_bound, },
                quote! { #new(move |handle: #handle| #built) },
            )
        }
        None => {
            let bound = render_fn_bound(render, quote! {});
            let built_bound = output.built_bound();
            let built = output.wrap_built(quote! { value() });
            (
                quote! { #bound -> Built + 'static },
                quote! { Built: #built_bound, },
                quote! { #new(move |()| #built) },
            )
        }
    };
    Setter {
        method: format_ident!("children_render"),
        generics: quote! { <ChildrenFn, Built> },
        args: quote! { value: ChildrenFn },
        where_clause: quote! {
            where
                ChildrenFn: #bound,
                #where_clause
        },
        value: wrap(build),
    }
}

fn render_children_block(
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let handle = render
        .handle
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));
    let output = render_output(render);
    let bound = render_fn_bound(render, quote! {});
    let block_bound = output.block_bound();
    let new = render_new(render);
    let built = output.wrap_block(quote! { children() });
    Setter {
        method: format_ident!("children_block"),
        generics: quote! { <ChildrenFn, ChildrenBlock> },
        args: quote! { children: ChildrenFn },
        where_clause: quote! {
            where
                ChildrenFn: #bound -> ChildrenBlock + 'static,
                ChildrenBlock: #block_bound,
        },
        value: wrap(quote! { #new(move |_handle: #handle| #built) }),
    }
}

fn render_children_with(
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let Some(handle) = &render.handle else {
        return no_children_with();
    };
    let output = render_output(render);
    let bound = render_fn_bound(render, quote! { #handle });
    let block_bound = output.block_bound();
    let new = render_new(render);
    let built = output.wrap_block(quote! { children(handle) });
    Setter {
        method: format_ident!("children_with"),
        generics: quote! { <ChildrenFn, ChildrenBlock> },
        args: quote! { children: ChildrenFn },
        where_clause: quote! {
            where
                ChildrenFn: #bound -> ChildrenBlock + 'static,
                ChildrenBlock: #block_bound,
        },
        value: wrap(quote! { #new(move |handle: #handle| #built) }),
    }
}

fn no_children_with() -> Setter {
    Setter {
        method: format_ident!("children_with"),
        generics: quote! { <ChildrenFn> },
        args: quote! { children: ChildrenFn },
        where_clause: quote! { where ChildrenFn: ::__beui_tree::reactive::HandsAValue },
        value: quote! { ::__beui_tree::reactive::HandsAValue::bind(children) },
    }
}

fn no_children_render() -> Setter {
    Setter {
        method: format_ident!("children_render"),
        generics: quote! { <ChildrenFn> },
        args: quote! { children: ChildrenFn },
        where_clause: quote! { where ChildrenFn: ::__beui_tree::reactive::BuildsItsChildren },
        value: quote! { ::__beui_tree::reactive::BuildsItsChildren::build(children) },
    }
}

fn children_setters(prop: &Prop) -> Vec<Setter> {
    let block = format_ident!("children_block");
    if prop.is_children {
        let ty = &prop.ty;
        let child = quote! { <#ty as ::__beui_tree::reactive::ChildrenSlot>::Child };
        return vec![
            Setter {
                method: block,
                generics: quote! { <ChildrenBlock> },
                args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
                where_clause: quote! { where ChildrenBlock: ::__beui_tree::reactive::IntoSegments<#child> },
                value: quote! { ::__beui_tree::reactive::Children::from_block(children()) },
            },
            no_children_with(),
            no_children_render(),
        ];
    }
    if prop.is_optional_child {
        let item = prop
            .inner_ty
            .as_ref()
            .expect("an optional child has an inner type");
        return vec![
            Setter {
                method: block,
                generics: quote! { <ChildrenBlock> },
                args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
                where_clause: quote! { where ChildrenBlock: ::__beui_tree::reactive::AtMostOneChild<#item> },
                value: quote! { ::__beui_tree::reactive::AtMostOneChild::at_most_one_child(children()) },
            },
            no_children_with(),
            no_children_render(),
        ];
    }
    if prop.is_child {
        let ty = &prop.ty;
        return vec![
            Setter {
                method: block,
                generics: quote! { <ChildrenBlock> },
                args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
                where_clause: quote! { where ChildrenBlock: ::__beui_tree::reactive::OneChild<#ty> },
                value: quote! { ::__beui_tree::reactive::OneChild::one_child(children()) },
            },
            no_children_with(),
            no_children_render(),
        ];
    }
    match &prop.optional_render {
        Some(render) => vec![
            render_children_closure(render, |build| quote! { Some(#build) }),
            render_children_block(render, |build| quote! { Some(#build) }),
            render_children_with(render, |build| quote! { Some(#build) }),
        ],
        None => {
            let render = prop.render.as_ref().expect("checked by the caller");
            vec![
                render_children_closure(render, |build| build),
                render_children_block(render, |build| build),
                render_children_with(render, |build| build),
            ]
        }
    }
}

fn named_setter(prop: &Prop) -> Setter {
    let ident = &prop.ident;
    let plain = |args: proc_macro2::TokenStream, value: proc_macro2::TokenStream| Setter {
        method: ident.clone(),
        generics: quote! {},
        args,
        where_clause: quote! {},
        value,
    };
    if prop.is_children {
        let ty = &prop.ty;
        plain(quote! { value: impl Into<#ty> }, quote! { value.into() })
    } else if prop.is_optional_child {
        let item = prop
            .inner_ty
            .as_ref()
            .expect("an optional child has an inner type");
        plain(
            quote! { value: impl Into<Option<#item>> },
            quote! { value.into() },
        )
    } else if let Some(render) = &prop.optional_render {
        render_setter(ident, render, |build| quote! { Some(#build) })
    } else if let Some(render) = &prop.render {
        render_setter(ident, render, |build| build)
    } else if let Some(args) = &prop.optional_func_args {
        func_setter(ident, args, |build| quote! { Some(#build) })
    } else if let Some(args) = &prop.func_args {
        func_setter(ident, args, |build| build)
    } else if prop.is_optional_click_callback {
        plain(
            quote! { value: impl ::__beui_tree::reactive::IntoClickCallback },
            quote! { Some(::__beui_tree::reactive::IntoClickCallback::into_click_callback(value)) },
        )
    } else if let Some(signature) = &prop.optional_callback_signature {
        plain(
            quote! { value: impl #signature },
            quote! { Some(::__beui_tree::reactive::IntoCallback::into_callback(value)) },
        )
    } else if let Some(inner_ty) = &prop.optional_reactive_inner_ty {
        plain(
            quote! { value: impl ::__beui_tree::reactive::IntoProp<#inner_ty> },
            quote! { Some(::__beui_tree::reactive::IntoProp::into_prop(value)) },
        )
    } else if let Some(inner_ty) = &prop.inner_ty {
        if is_string_type(inner_ty) {
            plain(
                quote! { value: impl Into<String> },
                quote! { Some(value.into()) },
            )
        } else if is_primitive_type(inner_ty) {
            plain(quote! { value: #inner_ty }, quote! { Some(value) })
        } else {
            plain(
                quote! { value: impl Into<Option<#inner_ty>> },
                quote! { value.into() },
            )
        }
    } else if prop.is_click_callback {
        plain(
            quote! { value: impl ::__beui_tree::reactive::IntoClickCallback },
            quote! { ::__beui_tree::reactive::IntoClickCallback::into_click_callback(value) },
        )
    } else if let Some(signature) = &prop.callback_signature {
        plain(
            quote! { value: impl #signature },
            quote! { ::__beui_tree::reactive::IntoCallback::into_callback(value) },
        )
    } else if let Some(inner_ty) = &prop.reactive_inner_ty {
        plain(
            quote! { value: impl ::__beui_tree::reactive::IntoProp<#inner_ty> },
            quote! { ::__beui_tree::reactive::IntoProp::into_prop(value) },
        )
    } else if let Some(inner_ty) = &prop.binding_inner_ty {
        plain(
            quote! { value: impl ::__beui_tree::reactive::IntoBinding<#inner_ty> },
            quote! { ::__beui_tree::reactive::IntoBinding::into_binding(value) },
        )
    } else if is_string_type(&prop.ty) {
        plain(quote! { value: impl Into<String> }, quote! { value.into() })
    } else {
        let ty = &prop.ty;
        plain(quote! { value: #ty }, quote! { value })
    }
}

fn prop_is_required(prop: &Prop) -> bool {
    if prop.inner_ty.is_some() || prop.is_children {
        return false;
    }
    if prop.is_child {
        return true;
    }
    prop.default.is_none()
        && prop.callback_signature.is_none()
        && !prop.is_click_callback
        && prop.binding_inner_ty.is_none()
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
    for character in ident.to_string().chars() {
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
}

fn take_prop_attr(attrs: &mut Vec<Attribute>) -> syn::Result<PropAttr> {
    let mut parsed = PropAttr {
        default: None,
        children: false,
    };
    let Some(position) = attrs.iter().position(|attr| attr.path().is_ident("prop")) else {
        return Ok(parsed);
    };
    let attr = attrs.remove(position);
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("children") {
            parsed.children = true;
            return Ok(());
        }
        if meta.path.is_ident("default") {
            parsed.default = Some(meta.value()?.parse::<Expr>()?);
            return Ok(());
        }
        Err(meta.error("expected `children` or `default = <expr>`"))
    })?;
    Ok(parsed)
}

fn generic_inner(ty: &Type, name: &str) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty.clone()),
        _ => None,
    })
}

fn generic_args(ty: &Type, name: &str) -> Option<Vec<Type>> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Some(Vec::new());
    };
    Some(
        args.args
            .iter()
            .filter_map(|arg| match arg {
                GenericArgument::Type(ty) => Some(ty.clone()),
                _ => None,
            })
            .collect(),
    )
}

fn is_string_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    segment.ident == "String" && matches!(segment.arguments, PathArguments::None)
}

fn is_primitive_type(ty: &Type) -> bool {
    const PRIMITIVES: [&str; 15] = [
        "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
        "u32", "u64", "usize",
    ];
    let Type::Path(path) = ty else {
        return false;
    };
    path.path
        .get_ident()
        .is_some_and(|ident| PRIMITIVES.iter().any(|primitive| ident == primitive))
}

fn is_named_type(ty: &Type, name: &str) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

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
    let output = sig.output.clone();
    let output_ty = match &sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    let props: Vec<Prop> = sig
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
            let PropAttr { default, children } = take_prop_attr(attrs)?;
            let callback_signature = is_named_type(ty, "Callback")
                .then(|| callback_signature(ty))
                .transpose()?;
            let inner_ty = generic_inner(ty, "Option");
            let is_optional_click_callback = inner_ty
                .as_ref()
                .is_some_and(|inner| is_named_type(inner, "ClickCallback"));
            let optional_callback_signature = inner_ty
                .as_ref()
                .filter(|inner| is_named_type(inner, "Callback"))
                .map(crate::callback_signature)
                .transpose()?;
            let optional_reactive_inner_ty = inner_ty
                .as_ref()
                .and_then(|inner| generic_inner(inner, "Prop"));
            let reactive_inner_ty = generic_inner(ty, "Prop");
            let binding_inner_ty = generic_inner(ty, "Binding");
            let optional_render = inner_ty.as_ref().and_then(render_kind);
            let optional_func_args = inner_ty.as_ref().and_then(func_args);
            let is_children = is_named_type(ty, "Children");
            let is_child = is_named_type(ty, "Child");
            let is_optional_child = inner_ty
                .as_ref()
                .is_some_and(|inner| is_named_type(inner, "Child"));
            Ok(Prop {
                ident,
                ty: (**ty).clone(),
                inner_ty,
                optional_reactive_inner_ty,
                reactive_inner_ty,
                binding_inner_ty,
                render: render_kind(ty),
                optional_render,
                func_args: func_args(ty),
                optional_func_args,
                is_children,
                is_child,
                is_optional_child,
                callback_signature,
                optional_callback_signature,
                is_click_callback: is_named_type(ty, "ClickCallback"),
                is_optional_click_callback,
                default,
                is_children_slot: children,
            })
        })
        .collect::<syn::Result<_>>()?;

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
    if let Some(slot) = children_slot {
        let prop = &props[slot];
        let known = prop.is_children
            || prop.is_child
            || prop.is_optional_child
            || prop.render.is_some()
            || prop.optional_render.is_some();
        if !known {
            return Err(syn::Error::new_spanned(
                &prop.ty,
                format!(
                    "prop `{}` of component `{name}` takes its children, so it must be typed `Children<_>`, `Child`, `Option<Child>`, `Render<_>`, or `RenderFn<_>`",
                    prop.ident,
                ),
            ));
        }
    }

    let required: Vec<usize> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop_is_required(prop))
        .map(|(index, _)| index)
        .collect();
    let req_params: Vec<Ident> = required
        .iter()
        .map(|index| format_ident!("Prop{}", camel_case(&props[*index].ident)))
        .collect();
    let req_traits: Vec<Ident> = required
        .iter()
        .map(|index| format_ident!("__{}_needs_{}", base, props[*index].ident))
        .collect();
    let req_slot_of: Vec<Option<usize>> = (0..props.len())
        .map(|index| required.iter().position(|slot| *slot == index))
        .collect();

    let req_trait_defs = required
        .iter()
        .zip(&req_traits)
        .map(|(index, trait_ident)| {
            let prop = props[*index].ident.to_string();
            let message = format!("missing required prop `{prop}` on component `{name}`");
            let label = format!("this tag does not set `{prop}`");
            let note = format!(
                "write `{prop}=...` in the tag, or give the prop a `#[prop(default = ...)]`"
            );
            quote! {
                #[doc(hidden)]
                #[allow(non_camel_case_types)]
                #[diagnostic::on_unimplemented(message = #message, label = #label, note = #note)]
                #vis trait #trait_ident<T> {
                    fn __prop(self) -> T;
                }

                #[doc(hidden)]
                impl<T> #trait_ident<T> for T {
                    fn __prop(self) -> T {
                        self
                    }
                }
            }
        });

    let fields = props.iter().enumerate().map(|(index, prop)| {
        let ident = &prop.ident;
        match req_slot_of[index] {
            Some(slot) => {
                let param = &req_params[slot];
                quote! { #ident: #param }
            }
            None => {
                let ty = &prop.ty;
                quote! { #ident: Option<#ty> }
            }
        }
    });

    let mut params: Vec<(GenericParam, proc_macro2::TokenStream)> = outer;
    params.extend(
        sig.generics
            .params
            .iter()
            .map(|param| (param.clone(), generic_arg(param))),
    );
    params.sort_by_key(|(param, _)| !matches!(param, GenericParam::Lifetime(_)));
    let lifetimes: Vec<_> = params
        .iter()
        .filter_map(|(param, _)| match param {
            GenericParam::Lifetime(param) => Some(&param.lifetime),
            _ => None,
        })
        .collect();
    let type_params: Vec<_> = params
        .iter()
        .filter_map(|(param, _)| match param {
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

    let comp_params: Vec<&GenericParam> = params.iter().map(|(param, _)| param).collect();
    let comp_args: Vec<proc_macro2::TokenStream> =
        params.iter().map(|(param, _)| generic_arg(param)).collect();
    let constructor_args: Vec<proc_macro2::TokenStream> =
        params.iter().map(|(_, arg)| arg.clone()).collect();
    let where_predicates: Vec<WherePredicate> = sig
        .generics
        .where_clause
        .iter()
        .flat_map(|clause| clause.predicates.iter().cloned())
        .chain(outer_where)
        .collect();
    let where_clause =
        (!where_predicates.is_empty()).then(|| quote! { where #(#where_predicates),* });
    let decl_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain(
            req_params
                .iter()
                .map(|param| quote! { #param = ::__beui_tree::reactive::MissingProp }),
        )
        .collect();
    let decl_generics = angle_bracketed(&decl_params);
    let all_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain(req_params.iter().map(|param| quote! { #param }))
        .collect();
    let all_generics = angle_bracketed(&all_params);
    let all_args: Vec<proc_macro2::TokenStream> = comp_args
        .iter()
        .cloned()
        .chain(req_params.iter().map(|param| quote! { #param }))
        .collect();
    let all_args = angle_bracketed(&all_args);
    let builder_all = quote! { #builder_ident #all_args };
    let constructor_args = angle_bracketed(&constructor_args);
    let constructor_ty = quote! { #builder_ident #constructor_args };

    let init_fields = props.iter().enumerate().map(|(index, prop)| {
        let ident = &prop.ident;
        match req_slot_of[index] {
            Some(_) => quote! { #ident: ::__beui_tree::reactive::MissingProp },
            None => quote! { #ident: ::core::default::Default::default() },
        }
    });

    let optional_methods: Vec<proc_macro2::TokenStream> = props
        .iter()
        .enumerate()
        .filter(|(index, _)| req_slot_of[*index].is_none())
        .flat_map(|(index, prop)| {
            let ident = prop.ident.clone();
            let mut setters = vec![named_setter(prop)];
            if children_slot == Some(index) {
                setters.extend(children_setters(prop));
            }
            setters
                .into_iter()
                .map(|setter| {
                    let Setter {
                        method,
                        generics,
                        args,
                        where_clause,
                        value,
                    } = setter;
                    let ident = &ident;
                    quote! {
                        pub fn #method #generics (mut self, #args) -> Self #where_clause {
                            self.#ident = Some(#value);
                            self
                        }
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let required_impls = required.iter().enumerate().map(|(slot, index)| {
        let prop = &props[*index];
        let ident = &prop.ident;
        let ty = &prop.ty;
        let others: Vec<&Ident> = field_idents
            .iter()
            .filter(|field| *field != ident)
            .collect();
        let impl_params: Vec<proc_macro2::TokenStream> = comp_params
            .iter()
            .map(|param| quote! { #param })
            .chain(
                req_params
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != slot)
                    .map(|(_, param)| quote! { #param }),
            )
            .collect();
        let impl_generics = angle_bracketed(&impl_params);
        let unset_args: Vec<proc_macro2::TokenStream> = comp_args
            .iter()
            .cloned()
            .chain(req_params.iter().enumerate().map(|(other, param)| {
                if other == slot {
                    quote! { ::__beui_tree::reactive::MissingProp }
                } else {
                    quote! { #param }
                }
            }))
            .collect();
        let unset_args = angle_bracketed(&unset_args);
        let set_args: Vec<proc_macro2::TokenStream> = comp_args
            .iter()
            .cloned()
            .chain(req_params.iter().enumerate().map(|(other, param)| {
                if other == slot {
                    quote! { #ty }
                } else {
                    quote! { #param }
                }
            }))
            .collect();
        let set_args = angle_bracketed(&set_args);

        let mut setters = vec![named_setter(prop)];
        if children_slot == Some(*index) {
            setters.extend(children_setters(prop));
        }
        let methods = setters.into_iter().map(|setter| {
            let Setter {
                method,
                generics,
                args,
                where_clause,
                value,
            } = setter;
            let others = &others;
            quote! {
                pub fn #method #generics (self, #args)
                    -> #builder_ident #set_args
                #where_clause
                {
                    let __prop_value = #value;
                    let #builder_ident { #ident: _, #(#others,)* } = self;
                    #builder_ident { #ident: __prop_value, #(#others,)* }
                }
            }
        });

        quote! {
            impl #impl_generics #builder_ident #unset_args #where_clause {
                #(#methods)*
            }
        }
    });

    let field_lets = props.iter().enumerate().map(|(index, prop)| {
        let ident = &prop.ident;
        let value = match req_slot_of[index] {
            Some(slot) => {
                let param = &req_params[slot];
                let trait_ident = &req_traits[slot];
                let ty = &prop.ty;
                quote! { <#param as #trait_ident<#ty>>::__prop(self.#ident) }
            }
            None if prop.binding_inner_ty.is_some() => quote! { self.#ident },
            None => match &prop.default {
                Some(default) if prop.reactive_inner_ty.is_some() => quote! {
                    self.#ident
                        .unwrap_or_else(|| ::__beui_tree::reactive::IntoProp::into_prop(#default))
                },
                Some(default) => quote! { self.#ident.unwrap_or_else(|| #default) },
                None => quote! { self.#ident.unwrap_or_default() },
            },
        };
        quote! { let #ident = #value; }
    });

    let build_where = {
        let bounds = required.iter().enumerate().map(|(slot, index)| {
            let param = &req_params[slot];
            let trait_ident = &req_traits[slot];
            let ty = &props[*index].ty;
            quote! { #param: #trait_ident<#ty> }
        });
        if required.is_empty() {
            quote! {}
        } else {
            quote! { where #(#bounds,)* }
        }
    };

    let owned_bindings = props
        .iter()
        .filter(|prop| prop.binding_inner_ty.is_some())
        .map(|prop| {
            let ident = &prop.ident;
            let initial = match &prop.default {
                Some(default) => quote! { #default },
                None => quote! { ::core::default::Default::default() },
            };
            quote! {
                let #ident = #ident.unwrap_or_else(|| {
                    ::__beui_tree::reactive::Binding::from(::__beui_tree::reactive::create_signal(#initial))
                });
            }
        });
    let finish = match finish {
        Finish::Body(block) => quote! {
            ::__beui_tree::reactive::component(&::__beui_tree::reactive::ComponentInfo { name: #name, module: module_path!() }, move || {
                #(#owned_bindings)*
                #block
            })
        },
        Finish::Call(call) => {
            let idents = props.iter().map(|prop| &prop.ident);
            quote! {
                {
                    #(#owned_bindings)*
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

    let items = quote! {
        #(#req_trait_defs)*

        #(#attrs)*
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #builder_ident #decl_generics #where_clause {
            #(#fields,)*
            with_built: ::std::vec::Vec<::std::boxed::Box<dyn ::core::ops::FnOnce(&#output_ty)>>,
            #phantom_field
        }

        impl #all_generics #builder_all #where_clause {
            #(#optional_methods)*

        }

        impl #all_generics ::__beui_tree::reactive::ComponentBuilder for #builder_all #where_clause {
            type Output = #output_ty;

            fn with_built(
                mut self,
                bind: impl ::core::ops::FnOnce(&#output_ty) + 'static,
            ) -> Self {
                self.with_built.push(::std::boxed::Box::new(bind));
                self
            }
        }

        #(#required_impls)*

        impl #all_generics #builder_all #where_clause {
            #[track_caller]
            pub fn build(self) #output #build_where {
                let with_built = self.with_built;
                #(#field_lets)*
                let node = #finish;
                for bind in with_built {
                    bind(&node);
                }
                node
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
        "expected a literal, a path, a path call, or a braced expression `={...}` for an attribute value",
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
        let pattern = Pat::parse_single(input)?;
        let pattern = if input.peek(Token![:]) {
            let colon_token = input.parse::<Token![:]>()?;
            Pat::Type(PatType {
                attrs: Vec::new(),
                pat: Box::new(pattern),
                colon_token,
                ty: Box::new(input.parse()?),
            })
        } else {
            pattern
        };
        input.parse::<Token![|]>()?;
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

fn expand_view_body(
    node: &ViewNode,
    builder: proc_macro2::TokenStream,
    build: Ident,
    tag: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    match &node.children {
        None => quote_spanned! { tag => #builder .#build() },
        Some(children) => {
            if let Some(head) = &node.head {
                let span = head.span;
                let params = match &head.pattern {
                    Some(pattern) => quote! { |#pattern| },
                    None => quote! { || },
                };
                if let Some(block) = &head.block {
                    let children_render = format_ident!("children_render", span = span);
                    return quote_spanned! { span =>
                        #builder .#children_render(move #params #block) .#build()
                    };
                }
                if head.pattern.is_some() {
                    let items = expand_child_block(children);
                    let children_with = format_ident!("children_with", span = span);
                    return quote_spanned! { span =>
                        #builder .#children_with(move #params #items) .#build()
                    };
                }
            }
            let items = expand_child_block(children);
            let block = quote_spanned! { tag => move || #items };
            let children_block = format_ident!("children_block", span = tag);
            quote_spanned! { tag =>
                #builder .#children_block(#block) .#build()
            }
        }
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

#[proc_macro]
pub fn view(item: TokenStream) -> TokenStream {
    let view = parse_macro_input!(item as View);
    let expanded = expand_view(&view);
    resolve_tree(quote! { { #expanded } }).into()
}

fn resolve_tree(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro_crate::{FoundCrate, crate_name};
    let target = ["beui", "beui-view", "beui-html", "beui-tree"]
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
