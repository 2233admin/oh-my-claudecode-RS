//! Proc macros for oh-my-claudecode-RS.
//!
//! Provides `#[derive(Tool)]` to auto-implement the `Tool` trait from struct fields.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Expr, Fields, Lit, Meta, parse_macro_input};

/// Derive macro that auto-implements the `Tool` trait for a struct.
///
/// # Usage
///
/// ```ignore
/// use omc_macros::Tool;
/// use omc_shared::tools::tool_trait::{ExecResult, ToolRiskLevel};
///
/// #[derive(Tool)]
/// #[tool(name = "read_file", description = "Read a file", risk = "ReadOnly")]
/// struct ReadFileTool {
///     /// Path to the file
///     path: String,
///     /// Optional line range
///     line_range: Option<(usize, usize)>,
/// }
///
/// impl ReadFileTool {
///     fn run(&self) -> impl std::future::Future<Output = anyhow::Result<ExecResult>> + Send + '_ {
///         async move {
///             Ok(ExecResult::ok(format!("Read {}", self.path)))
///         }
///     }
/// }
/// ```
///
/// # Attributes
///
/// - `name` (required): Tool name as shown to the LLM
/// - `description` (required): What the tool does
/// - `risk` (optional): `ReadOnly`, `Standard`, or `Dangerous` (default: `Standard`)
///
/// # Field attributes
///
/// - `#[tool(required)]`: Override Option detection, mark as required
/// - `#[tool(desc = "...")]`: Field description (falls back to doc comments)
///
/// # Supported field types
///
/// `String`, `bool`, `i32`, `i64`, `u32`, `u64`, `f64`, `usize`,
/// `Option<T>`, `Vec<T>`, `Vec<String>`.
#[proc_macro_derive(Tool, attributes(tool))]
pub fn derive_tool(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_tool_derive(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn impl_tool_derive(input: &DeriveInput) -> Result<proc_macro2::TokenStream, syn::Error> {
    let name = &input.ident;

    // Parse #[tool(...)] attributes
    let (tool_name, tool_desc, risk_level) = parse_tool_attrs(input)?;

    // Validate struct and extract named fields
    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "#[derive(Tool)] only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(Tool)] only supports structs",
            ));
        }
    };

    // Note: the struct must also derive `serde::Deserialize` (or `serde::Deserialize`).
    // We can't check for it here because derive attributes are stripped before expansion.
    // If missing, the user will get a clear error from the generated `serde_json::from_value` call.

    // Build JSON Schema properties + required array
    let mut property_tokens = Vec::new();
    let mut required_names = Vec::new();

    for field in fields.iter() {
        let field_name = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "unnamed field"))?;
        let field_name_str = field_name.to_string();

        let (schema_type, is_optional) = rust_type_to_schema(&field.ty)?;
        let description = field_description(field);

        let mut prop = quote! {
            props.insert(#field_name_str.to_string(), serde_json::json!({
                "type": #schema_type,
                "description": #description,
            }));
        };

        // Check for #[tool(required)] override
        let forced_required = has_tool_attr(field, "required");
        let is_required = forced_required || !is_optional;

        if is_required {
            required_names.push(field_name_str.clone());
        }

        // Handle array items for Vec types
        if schema_type == "array" {
            let items_type = vec_inner_type(&field.ty).unwrap_or_else(|| "string".to_string());
            prop = quote! {
                props.insert(#field_name_str.to_string(), serde_json::json!({
                    "type": "array",
                    "items": { "type": #items_type },
                    "description": #description,
                }));
            };
        }

        property_tokens.push(prop);
    }

    let required_json = if required_names.is_empty() {
        quote! { serde_json::json!([]) }
    } else {
        quote! { serde_json::json!([#(#required_names),*]) }
    };

    let expanded = quote! {
        impl omc_shared::tools::tool_trait::Tool for #name {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_desc
            }

            fn parameters(&self) -> serde_json::Value {
                let mut props = serde_json::Map::new();
                #(#property_tokens)*
                serde_json::json!({
                    "type": "object",
                    "properties": props,
                    "required": #required_json,
                })
            }

            fn execute(
                &self,
                parameters: omc_shared::tools::tool_trait::ToolParameters,
            ) -> omc_shared::tools::tool_trait::BoxFuture<'_, anyhow::Result<omc_shared::tools::tool_trait::ExecResult>> {
                Box::pin(async move {
                    let value = serde_json::to_value(&parameters)?;
                    let args: #name = serde_json::from_value(value)
                        .map_err(|e| anyhow::anyhow!("parameter deserialization failed: {e}"))?;
                    args.run().await
                })
            }

            fn risk_level(&self) -> omc_shared::tools::tool_trait::ToolRiskLevel {
                #risk_level
            }
        }
    };

    Ok(expanded)
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

fn parse_tool_attrs(
    input: &DeriveInput,
) -> Result<(String, String, proc_macro2::TokenStream), syn::Error> {
    let mut tool_name = None;
    let mut tool_desc = None;
    let mut risk_str = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("tool") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    tool_name = Some(s.value());
                }
            } else if meta.path.is_ident("description") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    tool_desc = Some(s.value());
                }
            } else if meta.path.is_ident("risk") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    risk_str = Some(s.value());
                }
            }
            Ok(())
        })?;
    }

    let tool_name = tool_name.ok_or_else(|| {
        syn::Error::new_spanned(
            input,
            "#[derive(Tool)] requires #[tool(name = \"...\")] attribute",
        )
    })?;

    let tool_desc = tool_desc.ok_or_else(|| {
        syn::Error::new_spanned(
            input,
            "#[derive(Tool)] requires #[tool(description = \"...\")] attribute",
        )
    })?;

    let risk_level = match risk_str.as_deref() {
        Some("ReadOnly") => quote! { omc_shared::tools::tool_trait::ToolRiskLevel::ReadOnly },
        Some("Dangerous") => quote! { omc_shared::tools::tool_trait::ToolRiskLevel::Dangerous },
        _ => quote! { omc_shared::tools::tool_trait::ToolRiskLevel::Standard },
    };

    Ok((tool_name, tool_desc, risk_level))
}

// ---------------------------------------------------------------------------
// Type → JSON Schema
// ---------------------------------------------------------------------------

fn rust_type_to_schema(ty: &syn::Type) -> Result<(String, bool), syn::Error> {
    // Option<T>
    if let Some(inner) = extract_option_inner(ty) {
        let (inner_schema, _) = rust_type_to_schema_inner(inner)?;
        return Ok((inner_schema, true));
    }
    // Vec<T>
    if extract_vec_inner(ty).is_some() {
        return Ok(("array".to_string(), false));
    }
    rust_type_to_schema_inner(ty).map(|(s, _)| (s, false))
}

fn rust_type_to_schema_inner(ty: &syn::Type) -> Result<(String, bool), syn::Error> {
    if let syn::Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
    {
        let ident = seg.ident.to_string();
        return match ident.as_str() {
            "String" | "str" => Ok(("string".to_string(), false)),
            "bool" => Ok(("boolean".to_string(), false)),
            "i32" | "i64" | "u32" | "u64" | "usize" | "isize" => Ok(("integer".to_string(), false)),
            "f32" | "f64" => Ok(("number".to_string(), false)),
            "Vec" => Ok(("array".to_string(), false)),
            _ => Ok(("object".to_string(), false)),
        };
    }
    // Tuple types like (usize, usize)
    if matches!(ty, syn::Type::Tuple(_)) {
        return Ok(("array".to_string(), false));
    }
    Ok(("object".to_string(), false))
}

fn extract_option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

fn extract_vec_inner(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

fn vec_inner_type(ty: &syn::Type) -> Option<String> {
    let inner = extract_vec_inner(ty)?;
    let (schema_type, _) = rust_type_to_schema_inner(inner).ok()?;
    Some(schema_type)
}

// ---------------------------------------------------------------------------
// Field helpers
// ---------------------------------------------------------------------------

fn field_description(field: &syn::Field) -> String {
    // Check #[tool(desc = "...")] first
    for attr in &field.attrs {
        if attr.path().is_ident("tool") {
            let mut desc = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("desc") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        desc = Some(s.value());
                    }
                }
                Ok(())
            });
            if let Some(d) = desc {
                return d;
            }
        }
    }
    // Fall back to doc comments
    let doc = extract_doc_comment(field);
    if doc.is_empty() {
        field
            .ident
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    } else {
        doc
    }
}

fn extract_doc_comment(field: &syn::Field) -> String {
    let mut lines = Vec::new();
    for attr in &field.attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(expr_lit) = &nv.value
            && let Lit::Str(s) = &expr_lit.lit
        {
            let line = s.value().trim().to_string();
            if !line.is_empty() {
                lines.push(line);
            }
        }
    }
    lines.join(" ")
}

fn has_tool_attr(field: &syn::Field, attr_name: &str) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("tool") {
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(attr_name) {
                    found = true;
                }
                Ok(())
            });
            if found {
                return true;
            }
        }
    }
    false
}
