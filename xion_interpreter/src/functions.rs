use crate::{
    Value,
    ast::{Node, VariableAttributes},
    runtime::{Runtime, RuntimeError, Scope, Variable},
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
    params: Vec<(String, Option<String>, bool)>,
    body: Vec<Node>,
}
impl NonNativeFunction {
    pub fn new(name: String, params: Vec<(String, Option<String>, bool)>, body: Vec<Node>) -> Self {
        Self { name, params, body }
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
        for (i, (name, ty, nullable)) in self.params.iter().enumerate() {
            let name = name.to_owned();
            let ty = ty.to_owned();
            let ty = runtime.parse_type(ty, &name)?;
            let attr = VariableAttributes {
                is_nullable: *nullable,
                ..Default::default()
            };

            let value = if i < args.len() {
                args[i].clone()
            } else {
                if !*nullable {
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
                .add_var(name, Variable::with_attributes(value, ty, attr)?)
                .expect("this is safe because these are the first vars made");
        }
        runtime.run().map(|v| v.as_value())
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
