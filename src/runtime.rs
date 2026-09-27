use std::collections::HashMap;

use crate::ast::Node::*;
use crate::ast::{MathSign, Node};
use crate::functions::{Function, NativeFunction, NativeFunctionHeader, NonNativeFunction};
use crate::xion_std;

#[derive(Debug, Clone)]
pub struct Runtime {
    nodes: Vec<Node>,
    scope: Scope,
    return_value: Option<Value>,
}
impl Runtime {
    pub fn start(nodes: Vec<Node>) -> Self {
        let mut scope = Scope::new();
        scope.attach_std_lib();

        Self::with_scope_and_nodes(nodes, scope)
    }
    pub fn with_scope_and_nodes(nodes: Vec<Node>, scope: Scope) -> Self {
        Self {
            nodes,
            scope,
            return_value: None,
        }
    }

    pub fn get_current_scope_mut(&mut self) -> &mut Scope {
        &mut self.scope
    }
    pub fn get_current_scope(&self) -> &Scope {
        &self.scope
    }

    fn eval(&mut self, node: Node, errors: &mut ErrorLog) -> Option<Value> {
        match node {
            Var(var_ast) => {
                if let Some(value) = self.eval(var_ast.value, errors) {
                    let var = Variable { value };
                    if self
                        .get_current_scope_mut()
                        .add_var(var_ast.name.clone(), var)
                        .is_err()
                    {
                        errors.error_string(format!(
                            "variable {} already exists cannot crease",
                            var_ast.name
                        ));
                    }
                } else {
                    errors.error_string(format!("cant parse value of var {}", var_ast.name));
                }

                None
            }
            StringLiteral(str) => Some(Value::String(str)),
            VarRef(var) => self.get_current_scope().get_var_value(&var).cloned(),
            VarReasign { name, new_value } => {
                let var = Variable {
                    value: self.eval(*new_value, errors)?,
                };
                if self.get_current_scope_mut().update_var(&name, var).is_err() {
                    errors.error_string(format!(
                        "cannot reassign var {name} because it does not exist"
                    ));
                }
                None
            }
            NumberLiteral(num) => Some(Value::Number(num)),
            Class(_class) => {
                println!("classes not implemented");
                None
            }
            FnDeclare(func) => {
                if self
                    .get_current_scope_mut()
                    .add_func(NonNativeFunction::new(
                        func.name.clone(),
                        func.paramaters,
                        func.body,
                    ))
                    .is_err()
                {
                    errors.error_string(format!(
                        "function {} already exists cannot create",
                        func.name
                    ));
                }

                None
            }
            FnCall(func_ast) => {
                let func = self
                    .get_current_scope()
                    .get_function(&func_ast.name)
                    .cloned()?;

                // if let Some(func) = self
                //     .get_current_scope()
                //     .get_function(&func_ast.name)
                //     .cloned()
                // {
                let mut values = Vec::new();
                for node in func_ast.args {
                    if let Some(node) = self.eval(node, errors) {
                        values.push(node)
                    }
                }
                // func.call(args)

                func.call(values, self.scope())
                // } else {
                //     println!("cannot find function {}",func_ast.name)
                //     None
                // }
                // None
            }
            BoolLiteral(b) => Some(Value::Bool(b)),
            Return(value) => {
                let value = if let Some(value) = value {
                    self.eval(*value, errors)
                } else {
                    None
                };
                self.return_value = value;

                None
            }
            Math { left, op, right } => {
                let left = self.eval(*left, errors);
                let right = self.eval(*right, errors);
                if let Some(left) = left
                    && let Some(right) = right
                {
                    Some(match op {
                        MathSign::Plus => left.add(&right).ok()?,
                        MathSign::Minus => left.sub(&right).ok()?,
                        MathSign::Multiply => unimplemented!(),
                    })
                } else {
                    None
                }
            }
        }
    }

    fn scope(&self) -> Scope {
        Scope::with_parent(self.get_current_scope().clone())
    }

    pub fn run(mut self) -> Option<Value> {
        let nodes = std::mem::take(&mut self.nodes);
        let mut error_handler = ErrorLog::new();
        for node in nodes {
            self.eval(node, &mut error_handler);
            if error_handler.has_new_error() {
                println!("Error: {}", error_handler.get_last())
            }
        }
        println!("{self:?}");
        self.return_value
    }
}
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(i64),
    Bool(bool),
}
impl Value {
    fn add(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(left) => match other {
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
                Value::Bool(_) | Value::Number(_) => Err(MathError::InvalidTypeRight),
            },
            Value::Number(left) => match other {
                Value::Bool(_) => Err(MathError::InvalidTypeRight),
                Value::Number(right) => Ok(Value::Number(*left + *right)),
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
            },
            Value::String(left) => match other {
                Value::Bool(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::Number(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::String(right) => Ok(Value::String(left.to_owned() + right)),
            },
        }
    }
    fn sub(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(_) | Value::String(_) => Err(MathError::InvalidTypeLeft),
            Value::Number(left) => match other {
                Value::Bool(_) | Value::String(_) => Err(MathError::InvalidTypeRight),
                Value::Number(right) => Ok(Value::Number(*left - *right)),
            },
        }
    }
}
#[derive(Debug, Clone)]
enum MathError {
    InvalidTypeLeft,
    InvalidTypeRight,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string_value = match self {
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.to_string(),
            Self::String(s) => s.to_owned(),
        };
        write!(f, "{string_value}")
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    parent_scope: Option<Box<Scope>>,
    vars: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
}
impl Scope {
    fn new() -> Self {
        Self {
            parent_scope: None,
            vars: HashMap::new(),
            functions: HashMap::new(),
        }
    }
    fn attach_std_lib(&mut self) {
        self.add_native_fn("print", xion_std::print);
        self.add_native_fn("input", xion_std::input);
    }
    fn add_native_fn(&mut self, name: &str, func: NativeFunctionHeader) {
        self.functions
            .insert(name.to_owned(), Function::Native(NativeFunction::new(func)));
    }

    fn with_parent(scope: Scope) -> Self {
        Self {
            parent_scope: Some(Box::new(scope)),
            vars: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    pub fn add_var(&mut self, name: String, var: Variable) -> Result<(), AlreadyExists> {
        if self.vars.contains_key(&name) {
            return Err(AlreadyExists);
        }

        self.vars.insert(name, var);
        Ok(())
    }

    fn update_var(&mut self, name: &str, var: Variable) -> Result<(), DoesNotExist> {
        // if !self.vars.contains_key(name){
        //     return Err(DoesNotExist);
        // }

        match self.vars.get_mut(name) {
            Some(value) => {
                *value = var;
                Ok(())
            }
            None => Err(DoesNotExist),
        }
    }

    fn add_func(&mut self, func: NonNativeFunction) -> Result<(), AlreadyExists> {
        if self.functions.contains_key(func.name()) {
            return Err(AlreadyExists);
        }

        self.functions
            .insert(func.name().to_owned(), Function::NonNative(func));
        Ok(())
    }

    pub fn get_var(&self, name: &str) -> Option<&Variable> {
        self.vars.get(name).or_else(|| match &self.parent_scope {
            Some(scope) => scope.get_var(name),
            None => None,
        })
    }
    pub fn get_var_value(&self, name: &str) -> Option<&Value> {
        match self.get_var(name) {
            Some(v) => Some(&v.value),
            None => None,
        }
    }
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name).or(match &self.parent_scope {
            Some(s) => s.get_function(name),
            None => None,
        })
    }

    // fn add_function
}

#[derive(Debug)]
pub struct AlreadyExists;
#[derive(Debug)]
struct DoesNotExist;

struct ErrorLog {
    errors: Vec<String>,
    changed: bool,
}
impl ErrorLog {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            changed: false,
        }
    }
    // fn error(&mut self,error:&str){
    //     self.errors.push(error.to_owned());
    //     self.changed=true
    // }
    fn error_string(&mut self, error: String) {
        self.errors.push(error);
        self.changed = true
    }
    fn has_new_error(&mut self) -> bool {
        if self.changed {
            self.changed = false;
            true
        } else {
            false
        }
    }
    fn get_last(&self) -> &String {
        self.errors.last().unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    value: Value,
}
impl Variable {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}
