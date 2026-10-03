use crate::{
    ast::Node,
    runtime::{Runtime, Scope, Value, Variable},
};

#[derive(Debug, Clone)]
pub enum Function {
    NonNative(NonNativeFunction),
    Native(NativeFunction),
}
impl Function {
    pub fn call(&self, args: Vec<Value>, scope: Scope) -> crate::runtime::RuntimeReturn {
        match self {
            Self::NonNative(f) => f.call(args, scope),
            Self::Native(f) => Ok((f.func)(args)),
        }
    }
}

pub type NativeFunctionHeader = fn(Vec<Value>) -> Option<Value>;

#[derive(Debug, Clone)]
pub struct NativeFunction {
    func: NativeFunctionHeader,
}
impl NativeFunction {
    pub fn new(func: NativeFunctionHeader) -> Self {
        Self { func }
    }
    pub fn inner(self) -> NativeFunctionHeader {
        self.func
    }
}

#[derive(Debug, Clone)]
pub struct NonNativeFunction {
    name: String,
    params: Vec<String>,
    body: Vec<Node>,
}
impl NonNativeFunction {
    pub fn new(name: String, params: Vec<String>, body: Vec<Node>) -> Self {
        Self { name, params, body }
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, args: Vec<Value>, scope: Scope) -> crate::runtime::RuntimeReturn {
        let mut runtime = Runtime::with_scope_and_nodes(self.body.clone(), scope);
        for (name, value) in self.params.iter().zip(args) {
            let name = name.to_owned();
            runtime
                .get_current_scope_mut()
                .add_var(name, Variable::new(value))
                .expect("this is safe because these are the first vars made");
        }
        runtime.run()
    }
}
