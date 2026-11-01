use crate::{
    Value,
    ast::{AstType, Node, VariableAttributes},
    runtime::{Runtime, RuntimeError, RuntimeType, Scope, Variable},
};

#[derive(Debug, Clone)]
pub enum Function {
    NonNative(NonNativeFunction),
    Native(std::sync::Arc<dyn NativeFunction>),
}
impl Function {
    pub fn call(&self, args: Vec<Value>, scope: Scope) -> crate::runtime::RuntimeReturn {
        match self {
            Self::NonNative(f) => f.call(args, scope),
            Self::Native(f) => f.call(args),
        }
    }
}

pub trait NativeFunction: std::fmt::Debug + Send + Sync {
    fn call(&self, args: Vec<Value>) -> Result<Option<Value>, RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct NonNativeFunction {
    name: String,
    params: Vec<(String, AstType)>,
    body: Vec<Node>,
    ret_type: Option<RuntimeType>,
}
impl NonNativeFunction {
    pub fn new(
        name: String,
        params: Vec<(String, AstType)>,
        body: Vec<Node>,
        ret_type: Option<RuntimeType>,
    ) -> Self {
        Self {
            name,
            params,
            body,
            ret_type,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, args: Vec<Value>, scope: Scope) -> crate::runtime::RuntimeReturn {
        if args.len() > self.params.len() {
            return Err(RuntimeError::FnCallArgLengthMixMatch {
                function_name: self.name().to_string(),
                expected: self.params.len(),
                found: args.len(),
            });
        }

        let mut runtime = Runtime::with_scope_and_nodes(self.body.clone(), scope);
        for (i, (name, ty)) in self.params.iter().enumerate() {
            let name = name.to_owned();
            let nullable = ty.nullable;
            let ty = ty.ty.to_owned();

            let ty = runtime.parse_type(ty)?;
            let attr = VariableAttributes {
                // is_nullable: nullable,
                ..Default::default()
            };

            let value = if i < args.len() {
                args[i].clone()
            } else {
                if !nullable {
                    return Err(RuntimeError::FnCallArgLengthMixMatch {
                        function_name: self.name().to_string(),
                        expected: self.params.len(),
                        found: args.len(),
                    });
                }
                Value::Null
            };

            runtime
                .get_current_scope_mut()
                .add_var(
                    name,
                    Variable::with_attributes(value, RuntimeType { ty, nullable }, attr)?,
                )
                .expect("this is safe because these are the first vars made");
        }
        let log_target = &("function call: ".to_string() + &self.name);
        let ret_value = runtime.run().map(|v| v.as_value())?;
        match (&self.ret_type, &ret_value) {
            (None, None) => Ok(None),
            (None, Some(_)) => {
                log::error!(target:log_target,"expected no return value but found one");
                Err(RuntimeError::Other(
                    "expected no return value but one was found".into(),
                ))
            }
            (Some(ret_type), Some(ret)) if ret_type.can_accept(ret) => Ok(ret_value),
            (Some(ret_type), Some(ret)) => {
                log::error!(target:log_target,"return type does not match delcared type");
                Err(RuntimeError::VariableTypeError {
                    actual_value: ret.value_type(),
                    expected_value: ret_type.clone(),
                })
            }
            (Some(_), None) => {
                log::error!(target:log_target,"expects a return value but found none");
                Err(RuntimeError::Other(
                    "expects a return value but found none".into(),
                ))
            }
        }
    }
}
#[derive(Debug, Clone)]
pub enum FunctionReturn {
    Value(Value),
    None,
}
impl FunctionReturn {
    pub fn as_value(self) -> Option<Value> {
        match self {
            Self::Value(v) => Some(v),
            Self::None => None,
        }
    }
}
