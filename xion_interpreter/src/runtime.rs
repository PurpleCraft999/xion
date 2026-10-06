use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::path::Path;
use std::sync::{Arc, Mutex};

use log::{debug, error, warn};

use crate::ast::Node::*;
use crate::ast::{MathSign, Node};
use crate::class::{ClassInstance, RuntimeClass};
use crate::functions::{Function, NativeFunction, NativeFunctionHeader, NonNativeFunction};
use crate::runtime::MathError::{InvalidTypeLeft, InvalidTypeRight};
use crate::runtime::RuntimeError::{DoesNotExist, Other};
use crate::{parse_and_lex, xion_std};

pub type RuntimeReturn = Result<Option<Value>, RuntimeError>;

#[derive(Debug, Clone)]
pub struct Runtime {
    nodes: Vec<Node>,
    current_scope: Scope,
    return_value: Option<Value>,
}
impl Runtime {
    pub fn main(nodes: Vec<Node>) -> Self {
        let mut scope = Scope::new();
        if let Err(e) = scope.add_module("lang") {
            error!("error adding lang module: {e}");
        }
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
            VarDeclare { name, value } => {
                let value = self.eval(*value)?.ok_or(RuntimeError::RequireValue)?;
                self.get_current_scope_mut()
                    .add_var(name.clone(), Variable::new(value))?;
                Ok(None)
            }
            StringLiteral(str) => Ok(Some(Value::String(str))),
            VarRef(var) => self
                .get_current_scope()
                .with_var(&var, |v| v.get().clone())
                .ok_or(RuntimeError::DoesNotExist(format!(
                    "cannot get variable \"{var}\""
                )))
                .map(Some),

            VarReasign { name, new_value } => {
                let var = self.eval(*new_value)?.ok_or(RuntimeError::RequireValue)?;
                if self.get_current_scope_mut().update_var(&name, var).is_err() {
                    Err(RuntimeError::DoesNotExist(format!(
                        "cannot reassign variable \"{name}\""
                    )))
                } else {
                    Ok(None)
                }
            }
            NumberLiteral(num) => Ok(Some(Value::Number(num))),
            FloatLiteral(float) => Ok(Some(Value::Float(float))),
            FnCall { name, arguments } => {
                let Some(func) = self
                    .get_current_scope()
                    .with_function(&name, |func| func.clone())
                else {
                    return Err(DoesNotExist(format!("cannot call function \"{}\"", name)));
                };

                //class constructor
                match func {
                    Function::Native(_) => {
                        if let Some(class) = self.get_current_scope().get_class(&name) {
                            const INJECTED_VAR_NAME: &str = "@from_constructor";
                            debug!("instantiating class {class:?}");
                            let mut instance = class.instantiate();
                            let mut scope = self.child_scope();

                            scope.parent_scope = if let ParentScope::Normal(n) = scope
                                .parent_scope
                                .as_ref()
                                .expect("we declared a child scope")
                            {
                                Some(ParentScope::Normal(Box::new(Scope {
                                    vars: n
                                        .vars
                                        .clone()
                                        .into_iter()
                                        .map(|(k, v)| (k + INJECTED_VAR_NAME, v))
                                        .collect(),
                                    ..Default::default()
                                })))
                            } else {
                                error!("child scope does not have parent");
                                None
                            };
                            let mut arguments = arguments;
                            for argument in &mut arguments {
                                if let Node::VarReasign { new_value, .. } = argument
                                    && let Node::VarRef(name) = &mut **new_value
                                {
                                    *name += INJECTED_VAR_NAME
                                }
                            }
                            // //
                            // scope.vars.extend(class.fields.clone());
                            scope.vars.extend(class.fields.clone());

                            // should run any VarAssigns for the class
                            let mut runtime = Runtime::with_scope_and_nodes(arguments, scope);
                            runtime.run()?;
                            instance.set_fields(runtime.current_scope.vars);

                            return Ok(Some(Value::Object(instance)));
                        }
                    }
                    Function::NonNative(_) => (),
                }

                //evaluates any variable names and the like
                let arguments = self.eval_list(arguments)?;

                func.call(arguments, self.child_scoped_no_var())
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
                let left = self.eval(*left)?.ok_or(RuntimeError::RequireValue)?;
                let right = self.eval(*right)?.ok_or(RuntimeError::RequireValue)?;
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
            ArrayLiteral(vec) => Ok(Some(Value::Array(self.eval_list(vec)?))),
            MethodCall {
                var_name,
                method_name,
                args,
            } => {
                let args = self.eval_list(args)?;

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
                        "cannot invoke method \"{}\" on var \"{1}\" because var \"{1}\"",
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
                    Value::Object(obj) => obj.get_field(&field_name).map(|v| v.get().clone()),
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
                        Value::Object(obj) => match obj.get_field_mut(&field_name) {
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
            If { condition, body } => {
                let condition = self.eval(*condition)?.ok_or(RuntimeError::RequireValue)?;
                if let Some(b) = condition.to_bool() {
                    if b {
                        let current_scope = Arc::new(Mutex::new(self.get_current_scope().clone()));
                        let run_scope = Scope::with_mutable_parent(current_scope.clone());
                        let mut runtime = Runtime::with_scope_and_nodes(body, run_scope);
                        let r = runtime.run()?;
                        *self.get_current_scope_mut() =
                            current_scope.lock().expect("panicking").clone();
                        Ok(r)
                    } else {
                        Ok(None)
                    }
                } else {
                    Err(RuntimeError::TypeError {
                        actual_value: condition.value_type(),
                        expected_value: ValueType::Bool,
                    })
                }
            }
            IsEqual { left, right } => {
                let left = self.eval(*left)?;
                let right = self.eval(*right)?;

                Ok(Some(Value::Bool(left == right)))
            }
            ClassDeclare { .. } | FnDeclare { .. } | Import(_) => Ok(None),
        }
    }
    fn early_eval(&mut self, node: Node) -> RuntimeReturn {
        match node {
            ClassDeclare { name, body } => {
                //TODO: In the future this scope should be only globals,consts, and the like and not completly empty
                let mut runtime = Runtime::with_scope_and_nodes(body, self.child_scoped_no_var());
                runtime.run()?;

                let runtime_class = RuntimeClass {
                    name: name.clone(),
                    fields: runtime.current_scope.vars,
                    methods: runtime.current_scope.functions,
                };
                self.get_current_scope_mut().add_class(runtime_class)?;

                self.get_current_scope_mut()
                    .add_native_fn(&name, xion_std::instantiate)?;
                Ok(None)
            }
            FnDeclare {
                name,
                parameters,
                body,
            } => {
                self.get_current_scope_mut()
                    .add_func(NonNativeFunction::new(name.clone(), parameters, body))?;
                Ok(None)
            }
            Import(name) => self
                .get_current_scope_mut()
                .add_module(&name)
                .map(|_| None)
                .map_err(|e| Other(format!("import error: {e}"))),
            _ => Ok(None),
        }
    }
    ///makes a child scope
    fn child_scope(&self) -> Scope {
        Scope::with_parent(self.get_current_scope().clone())
    }
    fn child_scoped_no_var(&self) -> Scope {
        let scope = {
            let mut scope = self.get_current_scope().clone();
            while !scope.vars.is_empty() {
                scope.vars = HashMap::new();
                if let Some(s) = scope.parent_scope {
                    scope = match s {
                        ParentScope::Mut(m) => m.lock().expect("panicking").clone(),
                        ParentScope::Normal(n) => *n,
                    }
                }
            }

            scope
        };

        Scope::with_parent(scope)
    }

    fn eval_list(&mut self, vec: Vec<Node>) -> Result<Vec<Value>, RuntimeError> {
        let mut arguments = Vec::new();

        for node in vec {
            let eval = self.eval(node)?;
            if let Some(node) = eval {
                arguments.push(node)
            } else {
                return Err(RuntimeError::RequireValue);
            }
        }
        Ok(arguments)
    }
    ///parses functions and classes
    fn parse_ahead(&mut self) -> RuntimeReturn {
        for node in self
            .nodes
            .clone()
            .into_iter()
            .filter(|n| matches!(n, FnDeclare { .. } | ClassDeclare { .. } | Import(_)))
        {
            self.early_eval(node)
                .map_err(|e| RuntimeError::Other(format!("error during early eval: {e}")))?;
        }
        Ok(None)
    }

    pub fn run(&mut self) -> RuntimeReturn {
        self.parse_ahead()?;

        let nodes = std::mem::take(&mut self.nodes);
        for node in nodes {
            if matches!(node, Return(_)) {
                self.eval(node)?;
                break;
            }

            self.eval(node)?;
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
    Float(f64),
}
impl Value {
    fn add(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Bool(left) => match other {
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::Number(left) => match other {
                Value::Float(right) => Ok(Value::Float(*left as f64 + *right)),
                Value::Number(right) => Ok(Value::Number(*left + *right)),
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::String(left) => match other {
                Value::Bool(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::Number(right) => Ok(Value::String(left.to_owned() + &(right.to_string()))),
                Value::String(right) => Ok(Value::String(left.to_owned() + right)),
                Value::Array(right) => Ok(Value::String(left.to_owned() + &vec_to_string(right))),
                Value::Float(right) => Ok(Value::String(left.to_owned() + &right.to_string())),
                Value::Object(_) => unimplemented!(),
            },
            Value::Array(left) => match other {
                Value::String(right) => Ok(Value::String(vec_to_string(left) + right)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::Float(left) => match other {
                Value::Float(right) => Ok(Value::Float(*left + *right)),
                Value::Number(right) => Ok(Value::Float(*left + *right as f64)),
                Value::String(right) => Ok(Value::String(left.to_string() + right)),
                e => Err(InvalidTypeRight(e.value_type())),
            },
            Value::Object(_) => unimplemented!(),
        }
    }
    fn sub(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Number(left) => match other {
                Value::Float(right) => Ok(Value::Float(*left as f64 - *right)),
                Value::Number(right) => Ok(Value::Number(*left - *right)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::Float(left) => match other {
                Value::Float(right) => Ok(Value::Float(left - right)),
                Value::Number(right) => Ok(Value::Float(left - *right as f64)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },

            Value::Object(_) => unimplemented!(),
            e => Err(MathError::InvalidTypeLeft(e.value_type())),
        }
    }
    fn mul(&self, other: &Value) -> Result<Value, MathError> {
        match self {
            Value::Number(left) => match other {
                Value::Float(right) => Ok(Value::Float(*left as f64 * *right)),
                Value::Number(right) => Ok(Value::Number(*left * *right)),
                Value::String(right) => string_mult(right, *left).map(Value::String),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::String(left) => match other {
                Value::Number(right) => string_mult(left, *right).map(Value::String),
                e => Err(InvalidTypeRight(e.value_type())),
            },
            Value::Float(left) => match other {
                Value::Float(right) => Ok(Value::Float(left * right)),
                Value::Number(right) => Ok(Value::Float(*left * *right as f64)),
                e => Err(MathError::InvalidTypeRight(e.value_type())),
            },
            Value::Object(_) => unimplemented!(),
            e => Err(MathError::InvalidTypeLeft(e.value_type())),
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
            (Value::Number(left), Value::Float(right)) => {
                if *right == 0.0 {
                    Err(MathError::DivisionByZero)
                } else {
                    Ok(Value::Float(*left as f64 / right))
                }
            }
            (Value::Float(left), Value::Number(right)) => {
                if *right == 0 {
                    Err(MathError::DivisionByZero)
                } else {
                    Ok(Value::Float(left / *right as f64))
                }
            }
            (Value::Float(left), Value::Float(right)) => {
                if *right == 0.0 {
                    Err(MathError::DivisionByZero)
                } else {
                    Ok(Value::Float(left / right))
                }
            }

            (Value::Number(_), e) => Err(InvalidTypeRight(e.value_type())),
            (e, Value::Number(_)) => Err(InvalidTypeLeft(e.value_type())),
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
            Self::Float(_) => ValueType::Float,
        }
    }
    fn to_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(bool) => Some(*bool),
            Self::Number(num) => Some(*num != 0),

            _ => None,
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
    Float,
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
            Self::Float(f) => f.to_string(),
        };
        write!(f, "{string_value}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MathError {
    InvalidTypeLeft(ValueType),
    InvalidTypeRight(ValueType),
    UnexpectedNegativeInt,
    DivisionByZero,
}
#[derive(Debug, Clone)]
pub enum ParentScope {
    Normal(Box<Scope>),
    Mut(Arc<Mutex<Scope>>),
}
impl ParentScope {
    fn with_var_mut<T>(&mut self, name: &str, c: impl FnOnce(&mut Variable) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.lock().ok()?.with_var_mut(name, c),
            ParentScope::Normal(p) => p.with_var_mut(name, c),
        }
    }
    fn with_var<T>(&self, name: &str, c: impl FnOnce(&Variable) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.lock().ok()?.with_var(name, c),
            ParentScope::Normal(p) => p.with_var(name, c),
        }
    }
    fn with_function<T>(&self, name: &str, c: impl FnOnce(&Function) -> T) -> Option<T> {
        match self {
            ParentScope::Mut(p) => p.lock().ok()?.with_function(name, c),
            ParentScope::Normal(p) => p.with_function(name, c),
        }
    }
    fn get_class(&self, name: &str) -> Option<Arc<RuntimeClass>> {
        match self {
            ParentScope::Mut(m) => m.lock().ok()?.get_class(name),
            ParentScope::Normal(p) => p.get_class(name),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub(crate) parent_scope: Option<ParentScope>,
    pub(crate) vars: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
    classes: HashMap<String, Arc<RuntimeClass>>,
}
impl Scope {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_native_fn(
        &mut self,
        name: &str,
        func: NativeFunctionHeader,
    ) -> Result<(), RuntimeError> {
        if self.functions.contains_key(name) {
            Err(RuntimeError::AlreadyExists(format!(
                "cannot add native function \"{name}\""
            )))
        } else {
            self.functions
                .insert(name.to_owned(), Function::Native(NativeFunction::new(func)));
            Ok(())
        }
    }

    pub fn with_parent(scope: Scope) -> Self {
        // scope.vars=HashMap::new();
        Self {
            parent_scope: Some(ParentScope::Normal(Box::new(scope))),
            ..Default::default()
        }
    }
    pub fn with_mutable_parent(scope: Arc<Mutex<Scope>>) -> Self {
        Self {
            parent_scope: Some(ParentScope::Mut(scope)),
            ..Default::default()
        }
    }

    pub fn add_var(&mut self, name: String, var: Variable) -> Result<(), RuntimeError> {
        if self.vars.contains_key(&name) {
            return Err(RuntimeError::AlreadyExists(format!(
                "cannot create variable \"{name}\""
            )));
        }

        self.vars.insert(name, var);
        Ok(())
    }

    pub fn update_var(&mut self, name: &str, var_value: Value) -> Result<(), RuntimeError> {
        match self.vars.get_mut(name) {
            Some(value) => {
                *value.get_mut() = var_value;
                Ok(())
            }
            None => {
                if let Some(parent) = &self.parent_scope {
                    match parent {
                        ParentScope::Mut(parent) => {
                            if let Some(var) =
                                parent.lock().expect("we are panicing").vars.get_mut(name)
                            {
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

    pub fn add_func(&mut self, func: NonNativeFunction) -> Result<(), RuntimeError> {
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

        self.classes
            .insert(class.name().to_owned(), Arc::new(class));
        Ok(())
    }
    pub fn get_class(&self, name: &str) -> Option<Arc<RuntimeClass>> {
        self.classes
            .get(name)
            .cloned()
            .or(match &self.parent_scope {
                Some(s) => s.get_class(name),
                None => None,
            })
    }
    fn extend(&mut self, other: Scope) -> Result<(), RuntimeError> {
        for (name, func) in other.functions {
            match func {
                Function::NonNative(f) => self.add_func(f),
                Function::Native(f) => self.add_native_fn(&name, f.inner()),
            }?
        }
        for (name, var) in other.vars {
            self.add_var(name, var)?
        }
        for (_, class) in other.classes {
            self.add_class(Arc::unwrap_or_clone(class))?
        }
        Ok(())
    }

    pub fn add_module(&mut self, name: &str) -> Result<(), RuntimeError> {
        let module = make_module(name)?;
        self.extend(module)
    }
}

pub fn make_module(name: &str) -> Result<Scope, RuntimeError> {
    fn parse_file(path: &Path) -> Result<Scope, RuntimeError> {
        let nodes = parse_and_lex(path);
        let mut run = Runtime::with_scope_and_nodes(nodes, Scope::new());
        run.run()?;
        Ok(run.current_scope)
    }
    fn parse_path(path: &Path) -> Result<Scope, RuntimeError> {
        let mut scope = Scope::new();
        if path.is_file() {
            scope.extend(parse_file(path)?)?
        } else if path.is_dir() {
            let dir = std::fs::read_dir(path)
                .map_err(|e| RuntimeError::Other(format!("error: {e} on path {path:?}")))?
                .flatten();
            for file in dir {
                let file_type = file
                    .file_type()
                    .map_err(|e| RuntimeError::Other(e.to_string()))?;
                if file_type.is_file() {
                    scope.extend(parse_file(&file.path())?)?;
                } else if file_type.is_dir() {
                    scope.extend(parse_path(&file.path())?)?
                } else {
                    error!("expected only files and dirs");
                    return Err(RuntimeError::Other(
                        "expected only files in module folder".to_string(),
                    ));
                }
            }
        }
        Ok(scope)
    }
    if let Some(std) = xion_std::get_std_lib(name) {
        return Ok(std);
    }

    let mut path = std::path::PathBuf::from(String::from("xlibs/") + name);
    // debug!("module path {:?}",path);
    if !path.exists() {
        path.set_extension("xn");
        if !path.exists() {
            error!("path {path:?} does not exists");
            return Err(RuntimeError::DoesNotExist(format!(
                "cannot import module \"{name}\""
            )));
        }
    }

    let mut scope = Scope::new();
    match scope.extend(parse_path(&path)?) {
        Ok(()) => (),
        Err(e) => {
            error!("module creation error: {e}");
            return Err(e);
        }
    }
    Ok(scope)
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
                "while conducting a math equasion this error occurred {err:?}"
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
