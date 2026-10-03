use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

use log::{debug, error, warn};

use crate::ast::Node::*;
use crate::ast::{MathSign, Node};
use crate::class::{ClassInstance, RuntimeClass};
use crate::functions::{Function, NativeFunction, NativeFunctionHeader, NonNativeFunction};
use crate::runtime::MathError::{InvalidTypeLeft, InvalidTypeRight};
use crate::runtime::RuntimeError::DoesNotExist;
use crate::xion_std;

pub type RuntimeReturn = Result<Option<Value>, RuntimeError>;

#[derive(Debug, Clone)]
pub struct Runtime {
    nodes: Vec<Node>,
    current_scope: Scope,
    return_value: Option<Value>,
}
impl Runtime {
    pub fn start(nodes: Vec<Node>) -> Self {
        let mut scope = Scope::new();
        scope.attach_std_lib();

        Self::with_scope_and_nodes(nodes, scope)
    }
    pub fn with_scope_and_nodes(nodes: Vec<Node>, current_scope: Scope) -> Self {
        Self {
            nodes,
            current_scope,
            return_value: None,
        }
    }

    pub fn get_current_scope_mut(&mut self) -> &mut Scope {
        &mut self.current_scope
    }
    pub fn get_current_scope(&self) -> &Scope {
        &self.current_scope
    }

    fn eval(&mut self, node: Node) -> RuntimeReturn {
        match node {
            VarDeclare(var_ast) => {
                let value = self
                    .eval(var_ast.value)?
                    .ok_or(RuntimeError::RequireValue)?;
                self.get_current_scope_mut()
                    .add_var(var_ast.name.clone(), Variable::new(value))?;
                Ok(None)
            }
            StringLiteral(str) => Ok(Some(Value::String(str))),
            VarRef(var) => self
                .get_current_scope()
                .with_var(&var, |v| v.get().clone())
                .ok_or(RuntimeError::DoesNotExist(format!(
                    "cannot get variable {var}"
                )))
                .map(Some),

            VarReasign { name, new_value } => {
                let var = self.eval(*new_value)?.ok_or(RuntimeError::RequireValue)?;

                if self.get_current_scope_mut().update_var(&name, var).is_err() {
                    Err(RuntimeError::DoesNotExist(format!(
                        "cannot reassign variable {name}"
                    )))
                } else {
                    Ok(None)
                }
            }
            NumberLiteral(num) => Ok(Some(Value::Number(num))),

            FnCall(func_ast) => {
                let Some(func) = self
                    .get_current_scope()
                    .with_function(&func_ast.name, |func| func.clone())
                else {
                    return Err(DoesNotExist(format!(
                        "cannot call function {}",
                        func_ast.name
                    )));
                };

                //class constructor
                match func {
                    Function::Native(_) => {
                        if let Some(class) = self.get_current_scope().get_class(&func_ast.name) {
                            debug!("instantiating class {class:?}");
                            let mut instance = class.instantiate();
                            let mut scope = Scope::new();
                            scope.vars = class.fields.clone();
                            // should run any VarAssigns for the class
                            let mut runtime = Runtime::with_scope_and_nodes(func_ast.args, scope);
                            runtime.run()?;
                            instance.fields = runtime.current_scope.vars;

                            return Ok(Some(Value::Object(instance)));
                        }
                    }
                    Function::NonNative(_) => (),
                }

                //evaluates any variable names and the like
                let args = func_ast.args;
                let arguments = self.eval_list(args);

                func.call(arguments, self.child_scope())
            }
            BoolLiteral(b) => Ok(Some(Value::Bool(b))),
            Return(value) => {
                let value = if let Some(value) = value {
                    self.eval(*value)?
                } else {
                    None
                };
                self.return_value = value;

                Ok(None)
            }
            Math { left, op, right } => {
                let left = self
                    .eval(*left)?
                    .ok_or(RuntimeError::MathError(MathError::InvalidTypeLeft))?;
                let right = self
                    .eval(*right)?
                    .ok_or(RuntimeError::MathError(MathError::InvalidTypeRight))?;
                // if let Some(left) = left
                //     && let Some(right) = right
                {
                    let value = match op {
                        MathSign::Plus => left.add(&right),
                        MathSign::Minus => left.sub(&right),
                        MathSign::Multiply => left.mul(&right),
                        MathSign::Division => left.div(&right),
                    };
                    value.map(Some).map_err(RuntimeError::MathError)
                }
            }
            ArrayLiteral(vec) => Ok(Some(Value::Array(
                vec.into_iter()
                    .map_while(|n| self.eval(n).ok().flatten())
                    .collect(),
            ))),
            MethodCall {
                var_name,
                method_name,
                args,
            } => {
                let args = self.eval_list(args);

                let s = self.get_current_scope_mut().with_var_mut(&var_name, |var| {
                    match var.get_mut() {
                        Value::Object(obj) => obj.call_method(&method_name, args),
                        o => Err(RuntimeError::TypeError {
                            actual_value: o.value_type(),
                            expected_value: ValueType::Object,
                        }),
                    }
                });
                match s {
                    Some(v) => v,
                    None => Err(RuntimeError::DoesNotExist(format!(
                        "cannot invoke method {} on var {1} because var {1}",
                        method_name, var_name
                    ))),
                }
            }
            FieldAccess {
                var_name,
                field_name,
            } => Ok(self
                .get_current_scope()
                .with_var(&var_name, |var| match &var.get() {
                    Value::Object(obj) => obj.fields.get(&field_name).map(|v| v.get().clone()),
                    _ => None,
                })
                .flatten()),
            FieldReasign {
                var_name,
                field_name,
                new_value,
            } => {
                let new_value = self.eval(*new_value)?.ok_or(RuntimeError::RequireValue)?;

                Ok(self
                    .get_current_scope_mut()
                    .with_var_mut(&var_name, |var| match var.get_mut() {
                        Value::Object(obj) => match obj.fields.get_mut(&field_name) {
                            Some(v) => {
                                v.value = new_value;
                                None
                            }
                            None => None,
                        },
                        _ => None,
                    })
                    .flatten())
            }
            ClassDeclare(_) | FnDeclare(_) => Ok(None),
        }
    }
    fn early_eval(&mut self, node: Node) -> RuntimeReturn {
        match node {
            ClassDeclare(class) => {
                //TODO: In the future this scope should be only globals,consts, and the like and not completly empty
                let mut runtime = Runtime::with_scope_and_nodes(class.fields, Scope::new());
                runtime.run()?;

                let runtime_class = RuntimeClass {
                    name: class.name.clone(),
                    fields: runtime.current_scope.vars,
                    methods: runtime.current_scope.functions,
                };
                self.get_current_scope_mut().add_class(runtime_class)?;

                self.get_current_scope_mut()
                    .add_native_fn(&class.name, xion_std::instantiate);
                Ok(None)
            }
            FnDeclare(func) => {
                self.get_current_scope_mut()
                    .add_func(NonNativeFunction::new(
                        func.name.clone(),
                        func.paramaters,
                        func.body,
                    ))?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    ///makes a child scope
    fn child_scope(&self) -> Scope {
        Scope::with_parent(self.get_current_scope().clone())
    }

    fn eval_list(&mut self, vec: Vec<Node>) -> Vec<Value> {
        let mut arguments = Vec::new();

        for node in vec {
            if let Some(node) = self.eval(node).ok().flatten() {
                arguments.push(node)
            }
        }
        arguments
    }
    ///parses functions and classes
    fn parse_ahead(&mut self) -> RuntimeReturn {
        for node in self
            .nodes
            .clone()
            .into_iter()
            .filter(|n| matches!(n, FnDeclare(_) | ClassDeclare(_)))
        {
            self.early_eval(node)
                .map_err(|e| RuntimeError::Other(format!("error during early eval {e}")))?;
        }
        Ok(None)
    }

    pub fn run(&mut self) -> RuntimeReturn {
        self.parse_ahead()?;
        let nodes = std::mem::take(&mut self.nodes);
        for node in nodes {
            if let Err(err) = self.eval(node) {
                error!("{err}");
                return Err(err);
            }
            if self.return_value.is_some() {
                break;
            }
        }
        debug!("{self:?}");
        Ok(self.return_value.take())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(i64),
    Bool(bool),
    Array(Vec<Value>),
    Object(ClassInstance),
}
impl Value {
    fn add(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(left) => match other {
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
                Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
                    Err(MathError::InvalidTypeRight)
                }
            },
            Value::Number(left) => match other {
                Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
                    Err(MathError::InvalidTypeRight)
                }
                Value::Number(right) => Ok(Value::Number(*left + *right)),
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
            },
            Value::String(left) => match other {
                Value::Bool(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::Number(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::String(right) => Ok(Value::String(left.to_owned() + right)),
                Value::Array(right) => Ok(Value::String(left.to_owned() + &vec_to_string(right))),
                Value::Object(_) => unimplemented!(),
            },
            Value::Array(left) => match other {
                Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
                    Err(MathError::InvalidTypeRight)
                }
                Value::String(right) => Ok(Value::String(vec_to_string(left) + right)),
            },
            Value::Object(_) => unimplemented!(),
        }
    }
    fn sub(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(_) | Value::String(_) | Value::Array(_) => Err(MathError::InvalidTypeLeft),
            Value::Number(left) => match other {
                Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
                    Err(MathError::InvalidTypeRight)
                }
                Value::Number(right) => Ok(Value::Number(*left - *right)),
            },
            Value::Object(_) => unimplemented!(),
        }
    }
    fn mul(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(_) | Value::Array(_) => Err(MathError::InvalidTypeLeft),
            Value::Number(left) => match other {
                Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
                    Err(MathError::InvalidTypeRight)
                }
                Value::Number(right) => Ok(Value::Number(*left * *right)),
                Value::String(right) => string_mult(right, *left).map(Value::String),
            },
            Value::String(left) => match other {
                Value::Number(right) => string_mult(left, *right).map(Value::String),
                Value::Array(_) | Value::Bool(_) | Value::String(_) | Value::Object(_) => {
                    Err(InvalidTypeRight)
                }
            },
            Value::Object(_) => unimplemented!(),
        }
    }
    fn div(&self, other: &Value) -> Result<Value, MathError> {
        match (self, other) {
            (Value::Number(left), Value::Number(right)) => {
                if *right == 0 {
                    Err(MathError::DivisionByZero)
                } else {
                    Ok(Value::Number(left / right))
                }
            }
            (Value::Number(_), _) => Err(InvalidTypeRight),
            (_, Value::Number(_)) => Err(InvalidTypeLeft),
            _ => unreachable!(
                "number ocupies both slots in previous brances therefore this cannot be reached"
            ),
        }
    }
    fn value_type(&self) -> ValueType {
        match self {
            Self::Array(_) => ValueType::Array,
            Self::Bool(_) => ValueType::Bool,
            Self::Number(_) => ValueType::Int,
            Self::Object(_) => ValueType::Object,
            Self::String(_) => ValueType::String,
        }
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Array,
    Bool,
    Int,
    Object,
    String,
}

fn vec_to_string<T: ToString>(vec: &Vec<T>) -> String {
    if vec.is_empty() {
        return String::from("[]");
    }

    let mut vec_str = String::from('[');
    for item in vec {
        vec_str += &item.to_string();
        vec_str.push(',');
    }
    vec_str.pop();
    vec_str.push(']');
    vec_str
}
fn string_mult(string: &str, int: i64) -> Result<String, MathError> {
    if int < 0 {
        Err(MathError::UnexpectedNegativeInt)
    } else {
        let mut new = String::new();
        for _ in 0..int {
            new += string;
        }
        Ok(new)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string_value = match self {
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.to_string(),
            Self::String(s) => s.to_owned(),
            Self::Array(v) => vec_to_string(v),
            Self::Object(o) => o.to_string(),
        };
        write!(f, "{string_value}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MathError {
    InvalidTypeLeft,
    InvalidTypeRight,
    UnexpectedNegativeInt,
    DivisionByZero,
}
#[derive(Debug, Clone)]
pub enum ParentScope {
    Normal(Box<Scope>),
    Mut(Rc<std::cell::RefCell<Scope>>),
}
impl ParentScope {
    fn with_var_mut<T>(&mut self, name: &str, c: impl FnOnce(&mut Variable) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.borrow_mut().with_var_mut(name, c),
            ParentScope::Normal(p) => p.with_var_mut(name, c),
        }
    }
    fn with_var<T>(&self, name: &str, c: impl FnOnce(&Variable) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.borrow_mut().with_var(name, c),
            ParentScope::Normal(p) => p.with_var(name, c),
        }
    }
    fn with_function<T>(&self, name: &str, c: impl FnOnce(&Function) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.borrow_mut().with_function(name, c),
            ParentScope::Normal(p) => p.with_function(name, c),
        }
    }
    fn get_class(&self, name: &str) -> Option<Rc<RuntimeClass>> {
        match self {
            ParentScope::Mut(m) => m.borrow().get_class(name),
            ParentScope::Normal(p) => p.get_class(name),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub(crate) parent_scope: Option<ParentScope>,
    pub(crate) vars: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
    classes: HashMap<String, Rc<RuntimeClass>>,
}
impl Scope {
    pub fn new() -> Self {
        Self::default()
    }
    fn attach_std_lib(&mut self) {
        self.add_native_fn("print", xion_std::print);
        self.add_native_fn("input", xion_std::input);
    }
    fn add_native_fn(&mut self, name: &str, func: NativeFunctionHeader) {
        self.functions
            .insert(name.to_owned(), Function::Native(NativeFunction::new(func)));
    }

    pub fn with_parent(scope: Scope) -> Self {
        Self {
            parent_scope: Some(ParentScope::Normal(Box::new(scope))),
            ..Default::default()
        }
    }
    pub fn with_mutable_parent(scope: Rc<RefCell<Scope>>) -> Self {
        Self {
            parent_scope: Some(ParentScope::Mut(scope)),
            ..Default::default()
        }
    }

    pub fn add_var(&mut self, name: String, var: Variable) -> Result<(), RuntimeError> {
        if self.vars.contains_key(&name) {
            return Err(RuntimeError::AlreadyExists(format!(
                "cannot create variable {name}"
            )));
        }

        self.vars.insert(name, var);
        Ok(())
    }

    fn update_var(&mut self, name: &str, var_value: Value) -> Result<(), RuntimeError> {
        match self.vars.get_mut(name) {
            Some(value) => {
                *value.get_mut() = var_value;
                Ok(())
            }
            None => {
                if let Some(parent) = &self.parent_scope {
                    match parent {
                        ParentScope::Mut(parent) => {
                            if let Some(var) = parent.borrow_mut().vars.get_mut(name) {
                                *var.get_mut() = var_value;
                                Ok(())
                            } else {
                                Err(RuntimeError::DoesNotExist(name.to_owned()))
                            }
                        }
                        ParentScope::Normal(parent) => {
                            if parent.vars.contains_key(name) {
                                warn!("tried to update read only variable from parent scope")
                            }
                            Err(RuntimeError::DoesNotExist(name.to_owned()))
                        }
                    }
                } else {
                    Err(RuntimeError::DoesNotExist(name.to_owned()))
                }
            }
        }
    }

    fn add_func(&mut self, func: NonNativeFunction) -> Result<(), RuntimeError> {
        if self.functions.contains_key(func.name()) {
            return Err(RuntimeError::AlreadyExists(format!(
                "funtion {}",
                func.name()
            )));
        }

        self.functions
            .insert(func.name().to_owned(), Function::NonNative(func));
        Ok(())
    }

    pub fn with_var_mut<T>(
        &mut self,
        name: &str,
        closure: impl FnOnce(&mut Variable) -> T,
    ) -> Option<T> {
        match self.vars.get_mut(name) {
            Some(v) => Some(closure(v)),
            None => {
                if let Some(parent) = &mut self.parent_scope {
                    parent.with_var_mut(name, closure)
                } else {
                    None
                }
            }
        }
    }
    pub fn with_var<T>(&self, name: &str, closure: impl FnOnce(&Variable) -> T) -> Option<T> {
        match self.vars.get(name) {
            Some(v) => Some(closure(v)),
            None => {
                if let Some(parent) = &self.parent_scope {
                    parent.with_var(name, closure)
                } else {
                    None
                }
            }
        }
    }

    pub fn with_function<T>(&self, name: &str, closure: impl FnOnce(&Function) -> T) -> Option<T> {
        match self.functions.get(name) {
            Some(v) => Some(closure(v)),
            None => {
                if let Some(parent) = &self.parent_scope {
                    parent.with_function(name, closure)
                } else {
                    None
                }
            }
        }
    }
    pub fn add_class(&mut self, class: RuntimeClass) -> Result<(), RuntimeError> {
        if self.classes.contains_key(class.name()) {
            return Err(RuntimeError::AlreadyExists(format!(
                "class {}",
                class.name()
            )));
        }

        self.classes.insert(class.name().to_owned(), Rc::new(class));
        Ok(())
    }
    // fn add_native_class<T:ToClass>(&mut self){
    //     self.add_class(T::to_class()).expect("names should not overlap with native classes")
    // }
    pub fn get_class(&self, name: &str) -> Option<Rc<RuntimeClass>> {
        self.classes
            .get(name)
            .cloned()
            .or(match &self.parent_scope {
                Some(s) => s.get_class(name),
                None => None,
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    value: Value,
}
impl Variable {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
    pub fn get(&self) -> &Value {
        &self.value
    }
    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.value
    }
}

#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    DoesNotExist(String),
    AlreadyExists(String),
    RequireValue,
    MathError(MathError),
    Other(String),
    TypeError {
        actual_value: ValueType,
        expected_value: ValueType,
    },
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::DoesNotExist(value) => write!(f, "{value} does not exist"),
            RuntimeError::AlreadyExists(value) => write!(f, "{value} already exists"),
            RuntimeError::RequireValue => write!(f, "a value was required but eval was None"),
            RuntimeError::MathError(err) => write!(
                f,
                "while conducting a math equasion this error oqured {err:?}"
            ),
            RuntimeError::Other(o) => write!(f, "{o}"),
            RuntimeError::TypeError {
                actual_value,
                expected_value,
            } => write!(
                f,
                "a value of type {:?} was expected but a value of {:?} was found instead",
                actual_value, expected_value
            ),
        }
    }
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod test;
