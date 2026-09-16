use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::braced;
use syn::parenthesized;
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Expr, ExprCall, ExprLit, ExprPath, ExprReference, ExprUnary, FnArg, GenericArgument,
    Ident, ItemFn, Lit, Pat, PatType, Path, PathArguments, Token, Type, UnOp, parse_macro_input,
};

struct Prop {
    ident: Ident,
    ty: Type,
    inner_ty: Option<Type>,
    optional_reactive_inner_ty: Option<Type>,
    reactive_inner_ty: Option<Type>,
    render: Option<Render>,
    optional_render: Option<Render>,
    func_args: Option<Vec<Type>>,
    optional_func_args: Option<Vec<Type>>,
    is_children: bool,
    is_child: bool,
    is_optional_child: bool,
    callback_args: Option<Vec<Type>>,
    is_click_callback: bool,
    default: Option<Expr>,
    is_children_slot: bool,
}

struct Render {
    once: bool,
    handle: Option<Type>,
}

fn render_kind(ty: &Type) -> Option<Render> {
    if is_named_type(ty, "Render") {
        Some(Render {
            once: true,
            handle: generic_inner(ty, "Render"),
        })
    } else if is_named_type(ty, "RenderFn") {
        Some(Render {
            once: false,
            handle: generic_inner(ty, "RenderFn"),
        })
    } else {
        None
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
        args: quote! { value: impl ::beui::reactive::IntoFunc<#value, #result> },
        where_clause: quote! {},
        value: wrap(quote! { ::beui::reactive::IntoFunc::into_func(value) }),
    }
}

fn render_setter(
    method: &Ident,
    render: &Render,
    wrap: impl Fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> Setter {
    let (signature, build) = match (&render.handle, render.once) {
        (Some(handle), true) => (
            quote! { ::beui::reactive::IntoRender<#handle> },
            quote! { ::beui::reactive::IntoRender::into_render(value) },
        ),
        (Some(handle), false) => (
            quote! { ::beui::reactive::IntoRenderFn<#handle> },
            quote! { ::beui::reactive::IntoRenderFn::into_render_fn(value) },
        ),
        (None, true) => (
            quote! { ::core::ops::FnOnce() -> ::beui::NodeId + 'static },
            quote! { ::beui::reactive::Render::new(move |()| value()) },
        ),
        (None, false) => (
            quote! { ::core::ops::Fn() -> ::beui::NodeId + 'static },
            quote! { ::beui::reactive::RenderFn::new(move |()| value()) },
        ),
    };
    Setter {
        method: method.clone(),
        generics: quote! {},
        args: quote! { value: impl #signature },
        where_clause: quote! {},
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
    let (bound, build) = if render.once {
        (
            quote! { ::core::ops::FnOnce() -> ChildrenBlock + 'static },
            quote! { ::beui::reactive::Render::new(
                move |_handle: #handle| ::beui::reactive::OneChild::one_child(children()),
            ) },
        )
    } else {
        (
            quote! { ::core::ops::Fn() -> ChildrenBlock + 'static },
            quote! { ::beui::reactive::RenderFn::new(
                move |_handle: #handle| ::beui::reactive::OneChild::one_child(children()),
            ) },
        )
    };
    Setter {
        method: format_ident!("children_block"),
        generics: quote! { <ChildrenFn, ChildrenBlock> },
        args: quote! { children: ChildrenFn },
        where_clause: quote! {
            where
                ChildrenFn: #bound + ::beui::reactive::UnitHandle<#handle>,
                ChildrenBlock: ::beui::reactive::OneChild + 'static,
        },
        value: wrap(build),
    }
}

fn children_setters(prop: &Prop) -> Vec<Setter> {
    let block = format_ident!("children_block");
    if prop.is_children {
        return vec![Setter {
            method: block,
            generics: quote! { <ChildrenBlock> },
            args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
            where_clause: quote! { where ChildrenBlock: Into<::beui::reactive::Children> },
            value: quote! { children().into() },
        }];
    }
    if prop.is_optional_child {
        return vec![Setter {
            method: block,
            generics: quote! { <ChildrenBlock> },
            args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
            where_clause: quote! { where ChildrenBlock: Into<::beui::reactive::Children> },
            value: quote! { {
                let children: ::beui::reactive::Children = children().into();
                children.only()
            } },
        }];
    }
    if prop.is_child {
        return vec![Setter {
            method: block,
            generics: quote! { <ChildrenBlock> },
            args: quote! { children: impl ::core::ops::FnOnce() -> ChildrenBlock },
            where_clause: quote! { where ChildrenBlock: ::beui::reactive::OneChild },
            value: quote! { ::beui::reactive::OneChild::one_child(children()) },
        }];
    }
    let children_render = format_ident!("children_render");
    match &prop.optional_render {
        Some(render) => vec![
            render_setter(&children_render, render, |build| quote! { Some(#build) }),
            render_children_block(render, |build| quote! { Some(#build) }),
        ],
        None => {
            let render = prop.render.as_ref().expect("checked by the caller");
            vec![
                render_setter(&children_render, render, |build| build),
                render_children_block(render, |build| build),
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
        plain(
            quote! { value: impl Into<::beui::reactive::Children> },
            quote! { value.into() },
        )
    } else if prop.is_optional_child {
        plain(
            quote! { value: impl Into<::beui::reactive::Children> },
            quote! { {
                let children: ::beui::reactive::Children = value.into();
                children.only()
            } },
        )
    } else if let Some(render) = &prop.optional_render {
        render_setter(ident, render, |build| quote! { Some(#build) })
    } else if let Some(render) = &prop.render {
        render_setter(ident, render, |build| build)
    } else if let Some(args) = &prop.optional_func_args {
        func_setter(ident, args, |build| quote! { Some(#build) })
    } else if let Some(args) = &prop.func_args {
        func_setter(ident, args, |build| build)
    } else if let Some(inner_ty) = &prop.optional_reactive_inner_ty {
        plain(
            quote! { value: impl ::beui::reactive::IntoProp<#inner_ty> },
            quote! { Some(::beui::reactive::IntoProp::into_prop(value)) },
        )
    } else if let Some(inner_ty) = &prop.inner_ty {
        if is_string_type(inner_ty) {
            plain(
                quote! { value: impl Into<String> },
                quote! { Some(value.into()) },
            )
        } else {
            plain(quote! { value: #inner_ty }, quote! { Some(value) })
        }
    } else if prop.is_click_callback {
        plain(
            quote! { value: impl ::core::ops::FnMut() + 'static },
            quote! { ::beui::reactive::ClickCallback::new(value) },
        )
    } else if let Some(args) = &prop.callback_args {
        let signature = match args.as_slice() {
            [value] => quote! { ::core::ops::FnMut(#value) },
            [value, result] => quote! { ::core::ops::FnMut(#value) -> #result },
            _ => panic!("`Callback` props take one or two type arguments"),
        };
        plain(
            quote! { value: impl #signature + 'static },
            quote! { ::beui::reactive::Callback::new(value) },
        )
    } else if let Some(inner_ty) = &prop.reactive_inner_ty {
        plain(
            quote! { value: impl ::beui::reactive::IntoProp<#inner_ty> },
            quote! { ::beui::reactive::IntoProp::into_prop(value) },
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
    prop.default.is_none() && prop.callback_args.is_none() && !prop.is_click_callback
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

fn take_prop_attr(attrs: &mut Vec<Attribute>) -> PropAttr {
    let mut parsed = PropAttr {
        default: None,
        children: false,
    };
    let Some(position) = attrs.iter().position(|attr| attr.path().is_ident("prop")) else {
        return parsed;
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
    })
    .expect("#[prop(...)] expects `children`, `default = <expr>`, or both");
    parsed
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
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = parse_macro_input!(item as ItemFn);
    let name = sig.ident.to_string();
    let component_ident = sig.ident.clone();
    let builder_ident = format_ident!("{}Builder", name);
    let output = sig.output.clone();

    let props: Vec<Prop> = sig
        .inputs
        .iter_mut()
        .map(|arg| {
            let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg else {
                panic!("#[component] functions cannot take `self`");
            };
            let ident = match pat.as_ref() {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("#[component] props must be simple identifiers"),
            };
            let PropAttr { default, children } = take_prop_attr(attrs);
            let inner_ty = generic_inner(ty, "Option");
            let optional_reactive_inner_ty = inner_ty
                .as_ref()
                .and_then(|inner| generic_inner(inner, "Prop"));
            let reactive_inner_ty = generic_inner(ty, "Prop");
            let optional_render = inner_ty.as_ref().and_then(render_kind);
            let optional_func_args = inner_ty.as_ref().and_then(func_args);
            let is_children = is_named_type(ty, "Children");
            let is_child = is_named_type(ty, "Child");
            let is_optional_child = inner_ty
                .as_ref()
                .is_some_and(|inner| is_named_type(inner, "Child"));
            Prop {
                ident,
                ty: (**ty).clone(),
                inner_ty,
                optional_reactive_inner_ty,
                reactive_inner_ty,
                render: render_kind(ty),
                optional_render,
                func_args: func_args(ty),
                optional_func_args,
                is_children,
                is_child,
                is_optional_child,
                callback_args: generic_args(ty, "Callback"),
                is_click_callback: is_named_type(ty, "ClickCallback"),
                default,
                is_children_slot: children,
            }
        })
        .collect();

    let designated: Vec<usize> = props
        .iter()
        .enumerate()
        .filter(|(_, prop)| prop.is_children_slot)
        .map(|(index, _)| index)
        .collect();
    if designated.len() > 1 {
        panic!("component `{name}` marks more than one prop `#[prop(children)]`");
    }
    let named_children = props.iter().position(|prop| prop.ident == "children");
    if let (Some(slot), Some(children)) = (designated.first().copied(), named_children)
        && slot != children
    {
        panic!(
            "component `{name}` already takes its children through `children`, so `{}` cannot be `#[prop(children)]`",
            props[slot].ident,
        );
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
            panic!(
                "prop `{}` of component `{name}` is `#[prop(children)]`, so it must be typed `Children`, `Child`, `Option<Child>`, `Render<_>`, or `RenderFn<_>`",
                prop.ident,
            );
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
        .map(|index| format_ident!("__{}_needs_{}", component_ident, props[*index].ident))
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

    let lifetimes: Vec<_> = sig
        .generics
        .lifetimes()
        .map(|param| &param.lifetime)
        .collect();
    let type_params: Vec<_> = sig
        .generics
        .type_params()
        .map(|param| &param.ident)
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
        .chain([
            format_ident!("with_test_id"),
            format_ident!("with_node_ref"),
        ])
        .chain(phantom_field.is_some().then(|| phantom_ident.clone()))
        .collect();

    let comp_params: Vec<&syn::GenericParam> = sig.generics.params.iter().collect();
    let comp_args: Vec<proc_macro2::TokenStream> = sig
        .generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
            syn::GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                quote! { #lifetime }
            }
            syn::GenericParam::Const(param) => {
                let ident = &param.ident;
                quote! { #ident }
            }
        })
        .collect();
    let where_clause = sig.generics.where_clause.clone();

    let decl_params: Vec<proc_macro2::TokenStream> = comp_params
        .iter()
        .map(|param| quote! { #param })
        .chain(
            req_params
                .iter()
                .map(|param| quote! { #param = ::beui::reactive::MissingProp }),
        )
        .collect();
    let decl_generics = angle_bracketed(&decl_params);
    let component_generics = angle_bracketed(
        &comp_params
            .iter()
            .map(|param| quote! { #param })
            .collect::<Vec<_>>(),
    );
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
    let default_args = angle_bracketed(&comp_args);
    let builder_default = quote! { #builder_ident #default_args };

    let init_fields = props.iter().enumerate().map(|(index, prop)| {
        let ident = &prop.ident;
        match req_slot_of[index] {
            Some(_) => quote! { #ident: ::beui::reactive::MissingProp },
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
                    quote! { ::beui::reactive::MissingProp }
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
            None if prop.inner_ty.is_some() => quote! { self.#ident.unwrap_or(None) },
            None => match &prop.default {
                Some(default) if prop.reactive_inner_ty.is_some() => quote! {
                    self.#ident
                        .unwrap_or_else(|| ::beui::reactive::IntoProp::into_prop(#default))
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

    let finish = quote! { ::beui::reactive::component(move || #block) };

    quote! {
        #(#req_trait_defs)*

        #(#attrs)*
        #vis struct #builder_ident #decl_generics #where_clause {
            #(#fields,)*
            with_test_id: Option<String>,
            with_node_ref: Option<::beui::reactive::NodeRef>,
            #phantom_field
        }

        #[allow(non_snake_case)]
        #vis fn #component_ident #component_generics () -> #builder_default #where_clause {
            #builder_ident {
                #(#init_fields,)*
                with_test_id: None,
                with_node_ref: None,
                #phantom_init
            }
        }

        impl #all_generics #builder_all #where_clause {
            #(#optional_methods)*

            pub fn with_test_id(mut self, value: impl Into<String>) -> Self {
                self.with_test_id = Some(value.into());
                self
            }

            pub fn with_node_ref(mut self, value: &::beui::reactive::NodeRef) -> Self {
                self.with_node_ref = Some(value.clone());
                self
            }
        }

        #(#required_impls)*

        impl #all_generics #builder_all #where_clause {
            #[track_caller]
            pub fn build(self) #output #build_where {
                let test_id = self.with_test_id;
                let node_ref = self.with_node_ref;
                #(#field_lets)*
                let node = #finish;
                if let Some(test_id) = test_id {
                    ::beui::reactive::with_document(|document| document.set_test_id(node, test_id));
                }
                if let Some(node_ref) = node_ref {
                    node_ref.fill(node);
                }
                node
            }
        }
    }
    .into()
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
        let value = if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            content.parse::<Expr>()?
        } else {
            parse_unbraced_value(input)?
        };
        Ok(ViewAttr { key, value })
    }
}

const SPECIAL_NAMES: &str = "`@sizing`, `@node_ref`, `@test_id`";

enum Special {
    Sizing,
    Setter(&'static str),
}

fn special_kind(name: &Ident) -> syn::Result<Special> {
    match name.to_string().as_str() {
        "sizing" => Ok(Special::Sizing),
        "node_ref" => Ok(Special::Setter("with_node_ref")),
        "test_id" => Ok(Special::Setter("with_test_id")),
        other => Err(syn::Error::new(
            name.span(),
            format!("unknown special prop `@{other}`, expected one of {SPECIAL_NAMES}"),
        )),
    }
}

fn reserved_plain_prop(name: &Ident) -> syn::Result<()> {
    let name_str = name.to_string();
    if matches!(name_str.as_str(), "node_ref" | "test_id") {
        return Err(syn::Error::new(
            name.span(),
            format!("`{name_str}` is a framework slot rather than a prop, write `@{name_str}=...`"),
        ));
    }
    Ok(())
}

struct ViewSizing {
    span: proc_macro2::Span,
    value: Expr,
}

enum ViewChildKind {
    Node(ViewNode),
    Expr(Expr),
}

struct ViewChild {
    kind: ViewChildKind,
}

impl Parse for ViewChild {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kind = if input.peek(Token![<]) {
            ViewChildKind::Node(input.parse()?)
        } else {
            let content;
            braced!(content in input);
            ViewChildKind::Expr(content.parse()?)
        };

        Ok(ViewChild { kind })
    }
}

struct ViewNode {
    tag_path: Vec<Ident>,
    props: Vec<ViewAttr>,
    specials: Vec<ViewAttr>,
    sizing: Option<ViewSizing>,
    children: Option<Vec<ViewChild>>,
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
        let mut specials = Vec::new();
        let mut special_names: Vec<String> = Vec::new();
        let mut sizing = None;
        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            if input.peek(Token![@]) {
                let at = input.parse::<Token![@]>()?;
                let attr = input.parse::<ViewAttr>()?;
                match special_kind(&attr.key)? {
                    Special::Sizing => {
                        if sizing.is_some() {
                            return Err(syn::Error::new(at.span, "duplicate `@sizing`"));
                        }
                        sizing = Some(ViewSizing {
                            span: at.span,
                            value: attr.value,
                        });
                    }
                    Special::Setter(method) => {
                        let name = attr.key.to_string();
                        if special_names.contains(&name) {
                            return Err(syn::Error::new(
                                attr.key.span(),
                                format!("`@{name}` is set more than once on this tag"),
                            ));
                        }
                        special_names.push(name);
                        specials.push(ViewAttr {
                            key: format_ident!("{}", method, span = attr.key.span()),
                            value: attr.value,
                        });
                    }
                }
                continue;
            }
            let attr = input.parse::<ViewAttr>()?;
            reserved_plain_prop(&attr.key)?;
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
                specials,
                sizing,
                children: None,
            });
        }
        input.parse::<Token![>]>()?;

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

        Ok(ViewNode {
            tag_path,
            props,
            specials,
            sizing,
            children: Some(children),
        })
    }
}

fn expand_child_items(children: &[ViewChild]) -> Vec<proc_macro2::TokenStream> {
    children
        .iter()
        .map(|child| {
            let (node, sizing) = match &child.kind {
                ViewChildKind::Node(node) => (expand_view_node(node), node.sizing.as_ref()),
                ViewChildKind::Expr(expr) => (quote! { #expr }, None),
            };
            match sizing {
                None => quote! { ::beui::reactive::intrinsic(#node) },
                Some(sizing) => {
                    let value = &sizing.value;
                    quote! { ::beui::reactive::size(#node, #value) }
                }
            }
        })
        .collect()
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
        .chain(node.specials.iter())
        .map(|prop| {
            let key = &prop.key;
            let value = &prop.value;
            quote! { .#key(#value) }
        })
        .collect::<Vec<_>>();

    let tag = last.span();
    let build = format_ident!("build", span = tag);

    match &node.children {
        None => quote_spanned! { tag => #component_path() #(#setters)* .#build() },
        Some(children) => {
            if let [
                ViewChild {
                    kind: ViewChildKind::Expr(Expr::Closure(closure)),
                },
            ] = children.as_slice()
            {
                let children_render = format_ident!("children_render", span = tag);
                return quote_spanned! { tag =>
                    #component_path() #(#setters)* .#children_render(#closure) .#build()
                };
            }
            let items = expand_child_items(children);
            let block = quote_spanned! { tag => move || [#(#items),*] };
            let children_block = format_ident!("children_block", span = tag);
            quote_spanned! { tag =>
                #component_path() #(#setters)* .#children_block(#block) .#build()
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
        if let ViewChildKind::Node(ViewNode {
            sizing: Some(sizing),
            ..
        }) = &root.kind
        {
            return syn::Error::new(
                sizing.span,
                "`@sizing` gives a child its share of its siblings' space, and a `view!` with one root builds that node on its own; write the sizing where the node is used as a child",
            )
            .to_compile_error();
        }
        return match &root.kind {
            ViewChildKind::Node(node) => expand_view_node(node),
            ViewChildKind::Expr(expr) => quote! { #expr },
        };
    }
    if view.roots.is_empty() {
        return quote! { ::beui::reactive::Children::default() };
    }
    let items = expand_child_items(&view.roots);
    quote! { ::beui::reactive::Children::from([#(#items),*]) }
}

#[cfg(test)]
mod tests;

#[proc_macro]
pub fn view(item: TokenStream) -> TokenStream {
    let view = parse_macro_input!(item as View);
    let expanded = expand_view(&view);
    quote! { { #expanded } }.into()
}
