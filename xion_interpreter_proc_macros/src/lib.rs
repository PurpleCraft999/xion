use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
use std::collections::HashMap;
use syn::{
    Block,
    Expr::{self},
    Fields, GenericArgument, ImplItem, ImplItemFn, Item, ItemEnum, ItemFn, ItemImpl, ItemMod,
    ItemStruct, LitStr, Path, PathArguments, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

#[proc_macro_attribute]
pub fn lib(_attr: TokenStream, stream: TokenStream) -> TokenStream {
    let mut stream = parse_macro_input!(stream as ItemMod);

    let mut functions: Vec<(String, syn::Ident)> = Vec::new();
    let mut classes = Vec::new();
    {
        let (_, content) = stream.content.as_ref().expect("must have body");
        for item in content {
            match item {
                Item::Fn(f) => {
                    if let Some(a) = f
                        .attrs
                        .iter()
                        .find(|a| a.path().is_ident("native_function"))
                    {
                        let actual_fn_name = f.sig.ident.clone();
                        let fn_name = if let Ok(s) = a.parse_args::<NativeFunctionAttr>()
                            && let Some(name) = s.function_name
                        {
                            name.value()
                        } else {
                            actual_fn_name.clone().to_string()
                        };
                        functions.push((fn_name, actual_fn_name));
                    }
                }
                Item::Struct(s)
                    if let Some(derive) = s.attrs.iter().find(|a| a.path().is_ident("derive")) =>
                {
                    let result = derive.parse_args_with(|input: syn::parse::ParseStream| {
                        let paths = Punctuated::<Path, Token![,]>::parse_terminated(input)?;
                        Ok(paths)
                    });

                    if let Ok(paths) = result
                        && paths.iter().find(|p| p.is_ident("NativeClass")).is_some()
                    {
                        classes.push(s.ident.clone());
                    }
                }
                _ => (), // let s = format!("{:?}",f.attrs.clone().into_iter().map(|a|a.meta.into_token_stream().to_string()).collect::<Vec<_>>());
                         // return quote!{compile_error!(#s);}.into();
            }

            // if let Item::Struct(st) = item{

            // }
        }
    }
    let (_, content) = stream.content.as_mut().expect("must have body");
    content.push(Item::Use(parse_quote!(
        use std::sync::Arc;
    )));
    content.push(Item::Use(parse_quote!(
        use crate::{
            functions::NativeFunction,
            class::{NativeClass, NativeClassInstance, Static},
            runtime::{FromValue, RuntimeError, Variable},
            Value,
        };
    )));
    content.push(Item::Use(parse_quote!(
        use xion_interpreter_proc_macros::{native_function, NativeClass, NativeClassInstance};
    )));

    {
        let functions: Vec<_> = functions
            .into_iter()
            .map(|(n, id)| {
                (n, {
                    format_ident!("{}Fn", make_pascal_case_from_snake(id.to_string()))
                })
            })
            .collect();
        let functions = functions.iter().map(|(a, b)| {
            quote! { (#a.to_string(), Arc::new(#b) as Arc<dyn NativeFunction>) }
        });

        let get_fn: ItemFn = parse_quote! {
            pub fn get_std_lib_fn()->Vec<(String,Arc<dyn NativeFunction>)>{
                Vec::from([#(#functions),*])
            }
        };
        content.push(Item::Fn(get_fn));
    }

    {
        let classes = classes.iter().map(|c| {
            // let name = c.to_string();

            quote! {Arc::new(#c::__class_new_inner()) as Arc<dyn NativeClass>}
        });

        let get_class_func: ItemFn = parse_quote! {
            pub fn get_std_lib_classes()->Vec<Arc<dyn NativeClass>>{

                Vec::from([#(#classes),*])

            }
        };
        content.push(Item::Fn(get_class_func));
    }

    quote! {#stream}.into()
}

struct NativeFunctionAttr {
    // library_name:LitStr,
    function_name: Option<LitStr>,
}
impl Parse for NativeFunctionAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // let lib_name = input.parse()?;
        let func_name = if !input.is_empty() {
            Some(input.parse()?)
        } else {
            None
        };

        // let func_name = if input.peek(Token![,]) {
        //     let _: Token![,] = input.parse().expect("");

        //     if input.peek(LitStr) {
        //         Some(input.parse()?)
        //     } else {
        //         return Err(input.error("expected a string literal after comma"));
        //     }
        // } else {
        //     None
        // };

        Ok(Self {
            // library_name:lib_name,
            function_name: func_name,
        })
    }
}

fn get_inner_type(ty: &Type, name: &str) -> Option<Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == name
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(inner_ty)) = args.args.first()
    {
        Some(inner_ty.clone())
    } else {
        None
    }
}
fn get_inner_types(ty: &Type, name: &str) -> Option<Vec<Type>> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == name
        && let PathArguments::AngleBracketed(args) = &segment.arguments
    {
        let mut types = Vec::new();
        for arg in args.args.clone() {
            if let GenericArgument::Type(t) = arg {
                types.push(t);
            }
        }

        Some(types)
    } else {
        None
    }
}

struct ParsedFunction {
    args: Vec<TokenStream2>,
    length_check: TokenStream2,
    handle_return: TokenStream2,
    xion_name: TokenStream2,
}

fn parse_function(
    input_fn: &ItemFn,
    fn_name: &syn::Ident,
    attr: &NativeFunctionAttr,
    self_func: bool,
) -> ParsedFunction {
    let mut arg_extractions = Vec::new();
    let mut arg_names = Vec::new();
    let mut skip_length_check = false;
    let mut min_args = 0usize;
    let mut max_args = 0usize;

    let get_inner_type_from = get_inner_type;
    let fn_name_code = if let Some(name) = &attr.function_name {
        let name = format_ident!("{}", name.value());
        quote! {#name}
    } else {
        quote! {#fn_name}
    };
    // Check if there is exactly 1 parameter and it is a `Vec<T>`
    if input_fn.sig.inputs.len() == 1
        && let syn::FnArg::Typed(pat_type) = &input_fn.sig.inputs[0]
        && let Some(inner_ty) = get_inner_type_from(&pat_type.ty, "Vec")
        && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
    {
        let ident = &pat_ident.ident;
        arg_names.push(ident);
        skip_length_check = true;

        let inner_str = quote!(#inner_ty).to_string();
        if inner_str == "Value" {
            // If it's explicitly Vec<Value>, pass the entire args vector directly
            arg_extractions.push(quote! {
                let #ident = args;
            });
        } else {
            // If it's Vec<T>, map each Value in the vector using from_value
            arg_extractions.push(quote! {
                let #ident: Vec<#inner_ty> = args.into_iter()
                    .map(|v| <#inner_ty>::from_value(v)?)
                    .collect();
            });
        }
    }
    if !skip_length_check && !input_fn.sig.inputs.is_empty() {
        for (i, arg) in input_fn.sig.inputs.iter().enumerate() {
            if let syn::FnArg::Typed(pat_type) = arg {
                let ty = &pat_type.ty;
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let ident = &pat_ident.ident;
                    arg_names.push(ident);
                    max_args += 1;

                    if let Some(inner_ty) = get_inner_type_from(ty, "Option") {
                        // Optional argument: defaults to None if omitted or null
                        arg_extractions.push(quote! {
                            let #ident = match args.get(#i) {
                                Some(val) if !matches!(val, Value::Null) => {
                                    Some(<#inner_ty>::from_value(val.clone())?)
                                }
                                _ => None,
                            };
                        });
                    } else {
                        // Required argument
                        min_args += 1;
                        arg_extractions.push(quote! {
                            let #ident = <#ty>::from_value(
                                args.get(#i).ok_or(RuntimeError::Other(format!("Missing argument at index {}", #i)))?.clone()
                            )?;
                        });
                    }
                    // arg_extractions.push(quote! {
                    //     let #ident = <#ty>::from_value(
                    //         args.get(#i).ok_or(RuntimeError::Other(format!("Missing argument at index {}", #i)))?.clone()
                    //     )?;
                    // });
                }
            }
        }
    }

    let num_args = arg_names.len();

    let fn_name_str = fn_name.to_string().to_token_stream();
    let length_check = if skip_length_check {
        quote! {}
    } else {
        quote! {
            if args.len() < #min_args || args.len()>#max_args {
                return Err(RuntimeError::FnCallArgLengthMixMatch {function_name:#fn_name_str.to_string() ,expected: #num_args, found: args.len() });
            }
        }
    };
    let this = if self_func {
        quote! {self.}
    } else {
        quote! {}
    };
    let return_handling = match &input_fn.sig.output {
        ReturnType::Default => {
            quote! {
                #this #fn_name(#(#arg_names),*);
                Ok(None)
            }
        }
        ReturnType::Type(_, ty) if get_inner_type_from(ty, "Option").is_some() => {
            quote! {
                let res = #this #fn_name(#(#arg_names),*);
                match res {
                    Some(val) => Ok(Some(Value::from(val))),
                    None => Ok(Some(Value::Null)),
                }
            }
        }
        ReturnType::Type(_, ty) if let Some(re) = get_inner_types(ty, "Result") => {
            let ok = re.first().expect("has ok value");

            let ret_handle = if get_inner_type(ok, "Option").is_some() {
                quote! {match res{
                    Some(r)=>Some(Value::From(r)),
                    None=>Some(Value::Null),
                }}
            } else
            //check for unit
            if let Type::Tuple(tup) = ok
                && tup.elems.is_empty()
            {
                quote! {Some(Value::Null)}
            } else {
                quote! {Some(Value::from(res))}
            };

            quote! {
                let res = #this #fn_name(#(#arg_names),*);

                match res{
                    Ok(res) => Ok(#ret_handle),
                    Err(e)=>Err(e),
                }
            }
        }

        ReturnType::Type(_, _ty) => {
            quote! {
                let res = #this #fn_name(#(#arg_names),*);
                Ok(Some(Value::from(res)))
            }
        }
    };
    ParsedFunction {
        args: arg_extractions,
        length_check,
        handle_return: return_handling,
        xion_name: fn_name_code,
    }
}

//ai
#[proc_macro_attribute]
pub fn native_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    let attr = parse_macro_input!(attr as NativeFunctionAttr);

    // Generate a unique struct name, e.g., `add` -> `AddFn`
    let struct_name_str: String = format!("{}Fn", make_pascal_case_from_snake(fn_name.to_string()));
    let struct_name = syn::Ident::new(&struct_name_str, fn_name.span());

    let ParsedFunction {
        args,
        length_check,
        handle_return: return_handling,
        xion_name: _,
    } = parse_function(&input_fn, fn_name, &attr, false);

    let expanded = quote! {
        // Keep the original function intact
        #input_fn

        // Generate the invoker struct
        #[derive(Debug)]
        pub struct #struct_name;
        // impl #struct_name{
        //     const Name:&str = #fn_name_str;
        // }

        impl NativeFunction for #struct_name {
            fn call(&self, args: Vec<Value>) -> Result<Option<Value>, RuntimeError> {
                #length_check

                // 2. Extract and type-check arguments dynamically
                #(#args)*

                #return_handling
            }
        }
    };

    TokenStream::from(expanded)
}

fn make_pascal_case_from_snake(mut kebab: String) -> String {
    //replace the  `_`'s
    while let Some(dash_pos) = kebab.find("_") {
        kebab.remove(dash_pos);
        let lower = kebab.remove(dash_pos);
        kebab.insert(dash_pos, lower.to_ascii_uppercase());
    }
    capitalize_first_letter(kebab)
}

fn capitalize_first_letter(mut name: String) -> String {
    let first_letter = name.remove(0);
    name.insert(0, first_letter.to_ascii_uppercase());
    name
}

#[proc_macro_derive(NativeClass, attributes(value))]
pub fn native_class(stream: TokenStream) -> TokenStream {
    let class = parse_macro_input!(stream as ItemStruct);
    let class_name = class.ident.clone();
    let class_name_str = class_name.to_string();
    let mut fields = Vec::new();
    let mut static_fields = Vec::new();
    for field in class.fields.iter() {
        if let Some(id) = &field.ident
            && let Some(at) = field.attrs.iter().find(|a| a.path().is_ident("value"))
            && let Ok(meta) = at.meta.require_name_value()
        {
            let value = meta.value.clone();
            if let Some(ty) = get_inner_type(&field.ty, "Static") {
                static_fields.push((id.clone(), ty, value));
            } else {
                fields.push((id.clone(), field.ty.clone(), value));
            }
        }
    }
    let obj_struct_name = format_ident!("{class_name}Instance");

    let obj_fields = fields
        .iter()
        .map(|(name, ty, _)| quote! {#name:#ty})
        .collect::<Vec<_>>();
    let obj_fields_name = fields
        .iter()
        .map(|(name, _, _)| quote!(#name))
        .collect::<Vec<_>>();
    let obj_struct: ItemStruct = parse_quote!(
        #[derive(Debug,NativeClassInstance)]
        pub struct #obj_struct_name{
            #[allow(unused)]
            #(#obj_fields),*,
            class:Arc<dyn NativeClass>
        }
    );

    let default_field_values = fields
        .iter()
        .map(|(name, _, value)| quote! {let #name = #value.to_owned(); })
        .collect::<Vec<_>>();
    let static_class_fields = static_fields
        .iter()
        .map(|(name, _, expr)| {
            quote! {#name:Static::new(#expr)}
        })
        .collect::<Vec<_>>();

    let static_fields_match = static_fields
        .iter()
        .map(|(name, _ty, _expr)| {
            let name_str = name.to_string();

            quote! {#name_str=>Some(self.#name.to_variable())}
        })
        .collect::<Vec<_>>();

    let obj_create_field_values = fields
        .iter()
        .map(|(name, _, _)| quote! {let #name = self.#name.to_owned();})
        .collect::<Vec<_>>();

    quote! {

        impl #class_name{
            const Name:&str = #class_name_str;
            fn __class_new_inner()->Self{
                #(#default_field_values)*
                Self{

                    #(#obj_fields_name),*,
                    #(#static_class_fields),*
                }
            }
        }
        impl NativeClass for #class_name{
            fn instantiate(&self,this:Arc<dyn NativeClass>,values:Vec<Value>)->Result<Arc<std::sync::Mutex<dyn NativeClassInstance>>,RuntimeError>{

                #(#obj_create_field_values)*

                Ok(Arc::new(std::sync::Mutex::new(#obj_struct_name{#(#obj_fields_name),*,class:this })) as Arc<std::sync::Mutex<dyn NativeClassInstance>>)

            }
            fn name(&self)->&str{
                Self::Name
            }
            fn get_static_field(&self,name:&str)->Option<Variable>{
                match name{

                    #(#static_fields_match),*,
                    _=>None
                }
            }

        }
        #obj_struct
    }.into()

    // quote! {#class}.into()
}

#[proc_macro_derive(NativeClassInstance)]
pub fn native_class_instance(stream: TokenStream) -> TokenStream {
    let class_instance = parse_macro_input!(stream as ItemStruct);
    let class_name = class_instance.ident;

    let field_names = class_instance
        .fields
        .iter()
        .map(|f| f.ident.clone().expect("has field name"))
        .map(|i| {
            let name_str = i.to_string();
            quote! {#name_str=>Some(Variable::new(Value::from(self.#i.to_owned())))}
        })
        .collect::<Vec<_>>();

    quote! {
        impl NativeClassInstance for #class_name{
            fn call_method(&mut self,name:&str,args:Vec<Value>)->Result<Option<Value>, RuntimeError>{
                self.__method_call(name,args)
            }
            fn get_field(&self,name:&str)->Option<Variable>{
                match name{
                    #(#field_names),*,
                    _=>None


                }

            }
            fn class(&self)->Arc<dyn NativeClass>{
                self.class.clone()

            }
            fn get_field_mut(&mut self,name:&str)->Option<&mut Variable>{
                panic!("not impl yet")

            }
        }

        impl From<#class_name> for Value {
            fn from(val: #class_name) -> Self {
                Value::Object(crate::class::ClassInstance::Native(Arc::new(std::sync::Mutex::new(val)) as Arc<std::sync::Mutex<dyn NativeClassInstance>>) )
            }
        }



    }.into()
}

#[proc_macro_attribute]
pub fn instance_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = input_fn.sig.ident.clone();

    let attr = parse_macro_input!(attr as NativeFunctionAttr);

    let ParsedFunction {
        args,
        length_check,
        handle_return,
        xion_name: name,
    } = parse_function(&input_fn, &fn_name, &attr, true);

    let gen_sig = format_ident!("{name}_method");

    quote! {

        #input_fn

        fn #gen_sig(&self,args:Vec<Value>)->Result<Option<Value>, RuntimeError>{
            #length_check

            #(#args)*

            #handle_return

        }



    }
    .into()
}

#[proc_macro_attribute]
pub fn native_class_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut block = parse_macro_input!(item as ItemImpl);

    let ty = block.self_ty.clone();

    let mut funcs = Vec::new();
    //more items will be added
    #[allow(clippy::single_match)]
    for item in &block.items {
        match item {
            ImplItem::Fn(func)
                if let Some(a) = func
                    .attrs
                    .iter()
                    .find(|a| a.path().is_ident("instance_method")) =>
            {
                // funcs.push(func.sig.ident.clone());
                let actual_fn_name = func.sig.ident.clone();
                let fn_name = if let Ok(s) = a.parse_args::<LitStr>() {
                    s.value()
                } else {
                    actual_fn_name.clone().to_string()
                };
                funcs.push((fn_name, actual_fn_name));
            }
            _ => (),
        }
    }

    let call_funcs = funcs
        .iter()
        .map(|(name, _)| {
            let name_str = name.to_string();
            let func_name = format_ident!("{name}_method");

            quote! {#name_str=>self.#func_name(args)}
        })
        .collect::<Vec<_>>();

    let class_ty = quote! {#ty}.to_string();

    let func: ImplItemFn = parse_quote!(
        fn __method_call(&mut self, name: &str, args: Vec<Value>) -> Result<Option<Value>, RuntimeError> {
            match name{
                #(#call_funcs),*,
                _=>Err(RuntimeError::DoesNotExist(format!("method {name} in class {}",#class_ty)))
            }
        }
    );

    block.items.push(ImplItem::Fn(func));
    block.into_token_stream().into()
}

//ai
#[proc_macro_attribute]
pub fn value_helper(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct CustomMapping {
        ty: Type,
        _arrow1: Token![=>],
        pat: Expr,
        // _arrow2: Token![=>],
        block: Block,
    }

    // 2. Implement Parse for it
    impl Parse for CustomMapping {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let ty = input.parse()?;
            let _arrow1 = input.parse()?;
            let pat = input.parse()?;
            // let _arrow2 = input.parse()?;
            let block = input.parse()?; // parses the { ... } block

            Ok(CustomMapping {
                ty,
                _arrow1,
                pat,
                // _arrow2,
                block,
            })
        }
    }

    struct AttributeArgs(Punctuated<CustomMapping, Token![,]>);

    impl Parse for AttributeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            // Parse pairs separated by commas directly using a closure
            let mappings = Punctuated::parse_terminated_with(input, |i| i.parse())?;
            Ok(AttributeArgs(mappings))
        }
    }

    let input_enum = parse_macro_input!(item as ItemEnum);
    let attr = parse_macro_input!(attr as AttributeArgs);
    let custom_a: HashMap<String, (Expr, Block)> =
        std::collections::HashMap::from_iter(attr.0.into_iter().map(|s| {
            (
                {
                    let t = s.ty;
                    quote! {#t}.to_string()
                },
                (s.pat, s.block),
            )
        }));
    let enum_name = &input_enum.ident;

    let mut expansions = Vec::new();

    // Iterate through all variants of the enum
    for variant in &input_enum.variants {
        let variant_ident = &variant.ident;

        // Match variants in the `Name(Type)` form (unnamed fields with exactly 1 type)
        if let Fields::Unnamed(fields) = &variant.fields
            && fields.unnamed.len() == 1
        {
            let ty = &fields.unnamed[0].ty;

            let custom_arm = if let Some((pat, block)) = custom_a.get(&quote! {#ty}.to_string()) {
                quote! {#pat=>#block,}
            } else {
                quote! {}
            };

            expansions.push(quote! {
                // 1. impl From<Type> for Value
                impl From<#ty> for #enum_name {
                    fn from(val: #ty) -> Self {
                        #enum_name::#variant_ident(val)
                    }
                }


                // 2. impl FromValue for Type
                impl FromValue for #ty {
                    fn from_value(value: #enum_name) -> Result<Self,RuntimeError>
                    where
                        Self: Sized
                    {
                        match value {
                            #enum_name::#variant_ident(val) => Ok(val),
                            #custom_arm
                            _ => Err(RuntimeError::TypeError{actual_value:value.value_type(),expected_value:ValueType::#variant_ident}),
                        }
                    }
                }
                impl From<Option<#ty>> for #enum_name{
                    fn from(val:Option<#ty>) ->Self{
                        match val{
                            Some(val)=>#enum_name::#variant_ident(val),
                            None=>#enum_name::Null,
                        }
                    }
                }
                impl FromValue for Option<#ty> {
                    fn from_value(value: #enum_name) -> Result<Self,RuntimeError>
                    where
                        Self: Sized
                    {
                        match value {
                            #enum_name::#variant_ident(val) => Ok(Some(val)),
                            // #custom_arm
                            _ => Err(RuntimeError::TypeError{actual_value:value.value_type(),expected_value:ValueType::#variant_ident}),
                        }
                    }
                }

            });
        }
    }

    let expanded = quote! {
        // #(#values)*
        #input_enum
        #(#expansions)*
    };

    TokenStream::from(expanded)
}
