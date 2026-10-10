use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Block, Expr, Fields, GenericArgument, ItemEnum, ItemFn, PathArguments, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};
//ai
#[proc_macro_attribute]
pub fn native_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;

    // Generate a unique struct name, e.g., `add` -> `AddFn`
    let struct_name_str: String = format!("{}Fn", make_pascal_case_from_snake(fn_name.to_string()));
    let struct_name = syn::Ident::new(&struct_name_str, fn_name.span());

    let mut arg_extractions = Vec::new();
    let mut arg_names = Vec::new();
    let mut skip_length_check = false;

    let get_inner_type_from = |ty: &Type, name: &str| -> Option<Type> {
        if let Type::Path(type_path) = ty
            && let Some(segment) = type_path.path.segments.last()
            && segment.ident == name
            && let PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(GenericArgument::Type(inner_ty)) = args.args.first()
        {
            return Some(inner_ty.clone());
        }

        None
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

                    arg_extractions.push(quote! {
                        let #ident = <#ty>::from_value(
                            args.get(#i).ok_or(RuntimeError::Other(format!("Missing argument at index {}", #i)))?.clone()
                        )?;
                    });
                }
            }
        }
    }

    let num_args = arg_names.len();

    let length_check = if skip_length_check {
        quote! {}
    } else {
        quote! {
            if args.len() != #num_args {
                return Err(RuntimeError::FnCallArgLengthMixMatch { expected: #num_args, found: args.len() });
            }
        }
    };

    let return_handling = match &input_fn.sig.output {
        ReturnType::Default => {
            quote! {
                #fn_name(#(#arg_names),*);
                Ok(None)
            }
        }

        ReturnType::Type(_, _ty) => {
            quote! {
                let res = #fn_name(#(#arg_names),*);
                Ok(Some(Value::from(res)))
            }
        }
    };

    let expanded = quote! {
        // Keep the original function intact
        #input_fn

        // Generate the invoker struct
        #[derive(Debug)]
        pub struct #struct_name;

        impl NativeFunction for #struct_name {
            fn call(&self, args: Vec<Value>) -> Result<Option<Value>, RuntimeError> {
                #length_check

                // 2. Extract and type-check arguments dynamically
                #(#arg_extractions)*

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
