use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::rc::Rc;

use log::{debug, error, warn};

use crate::ast::Node::*;
use crate::ast::{MathSign, Node};
use crate::functions::{Function, NativeFunction, NativeFunctionHeader, NonNativeFunction};
use crate::runtime::MathError::{InvalidTypeLeft, InvalidTypeRight};
use crate::xion_std;

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

    fn eval(&mut self, node: Node) -> Option<Value> {
        match node {
            VarDeclare(var_ast) => {
                if let Some(value) = self.eval(var_ast.value) {
                    if self
                        .get_current_scope_mut()
                        .add_var(var_ast.name.clone(), Variable::new(value))
                        .is_err()
                    {
                        error!("variable {} already exists cannot crease", var_ast.name);
                    }
                } else {
                    error!("cant parse value of var {}", var_ast.name)
                }

                None
            }
            StringLiteral(str) => Some(Value::String(str)),
            VarRef(var) => self
                .get_current_scope()
                .get_var(&var)
                .map(|v| (*v.get()).clone()),
            VarReasign { name, new_value } => {
                let var = self.eval(*new_value)?;

                // self.get_current_scope().get_var(&name).and_then(|v|v.value)
                if self.get_current_scope_mut().update_var(&name, var).is_err() {
                    error!("cannot reassign var {name} because it does not exist");
                }
                None
            }
            NumberLiteral(num) => Some(Value::Number(num)),
            ClassDeclare(class) => {
                //TODO: In the future this scope should be only globals,consts, and the like and not completly empty
                let mut runtime = Runtime::with_scope_and_nodes(class.fields, Scope::new());
                runtime.run();

                let runtime_class = Class {
                    name: class.name.clone(),
                    fields: runtime.current_scope.vars,
                    methods: runtime.current_scope.functions,
                };
                if self
                    .get_current_scope_mut()
                    .add_class(runtime_class)
                    .is_err()
                {
                    error!("class {} already exists in this scope", class.name)
                }
                self.get_current_scope_mut()
                    .add_native_fn(&class.name, xion_std::instantiate);
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
                    error!("function {} already exists cannot create", func.name);
                }

                None
            }
            FnCall(func_ast) => {
                let Some(func) = self
                    .get_current_scope()
                    .get_function(&func_ast.name)
                    .cloned()
                else {
                    error!("tried to call unknown function {}", func_ast.name);
                    return None;
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
                            runtime.run();
                            instance.fields = runtime.current_scope.vars;

                            return Some(Value::Object(instance));
                        }
                    }
                    Function::NonNative(_) => (),
                }

                //evaluates any variable names and the like
                let arguments = self.eval_list(func_ast.args);

                func.call(arguments, self.child_scope())
            }
            BoolLiteral(b) => Some(Value::Bool(b)),
            Return(value) => {
                let value = if let Some(value) = value {
                    self.eval(*value)
                } else {
                    None
                };
                self.return_value = value;
                //force execution to stop by removing all remaining nodes
                self.nodes = Vec::new();

                None
            }
            Math { left, op, right } => {
                let left = self.eval(*left);
                let right = self.eval(*right);
                if let Some(left) = left
                    && let Some(right) = right
                {
                    let result = match op {
                        MathSign::Plus => left.add(&right),
                        MathSign::Minus => left.sub(&right),
                        MathSign::Multiply => left.mul(&right),
                        MathSign::Division => left.div(&right),
                    };
                    if let Ok(value) = result {
                        Some(value)
                    } else if let Err(err) = result {
                        error!("invalid math operation {err:?}");
                        None
                    } else {
                        unreachable!("it must be either Ok or Err")
                    }
                } else {
                    None
                }
            }
            ArrayLiteral(vec) => Some(Value::Array(
                vec.into_iter().map_while(|n| self.eval(n)).collect(),
            )),
            MethodCall {
                var_name,
                method_name,
                args,
            } => {
                let args = self.eval_list(args);
                let Some(var) = self.get_current_scope().get_var(&var_name) else {
                    error!("tried to invoke method on non existing var");
                    return None;
                };

                match &*var.get() {
                    Value::Object(obj) => {
                        match obj.call_method(&method_name, args, self.child_scope()) {
                            Ok(r) => r,
                            Err(_) => {
                                error!(
                                    "tried to call method {method_name} on {var_name} but the type of {} does not have that method",
                                    obj.class.name
                                );
                                None
                            }
                        }
                    }
                    _ => None,
                }
            }
            FieldAccess {
                var_name,
                field_name,
            } => {
                let Some(var) = self.get_current_scope().get_var(&var_name) else {
                    error!("tried to invoke method on non existing var");
                    return None;
                };
                match &*var.get() {
                    Value::Object(obj) => match obj.fields.get(&field_name) {
                        Some(v) => Some(v.get().clone()),
                        None => None,
                    },
                    _ => None,
                }
            }
            FieldReasign {
                var_name,
                field_name,
                new_value,
            } => {
                let new_value = self.eval(*new_value)?;
                let Some(var) = self.get_current_scope_mut().get_var_mut(&var_name) else {
                    error!("tried to invoke method on non existing var");
                    return None;
                };
                match var.get_mut() {
                    Value::Object(obj) => match obj.fields.get_mut(&field_name) {
                        Some(v) =>{
                            v.value=new_value;
                            None

                        },
                        None => None,
                    },
                    _ => None,
                }
            }
        }
    }
    ///makes a child scope
    fn child_scope(&self) -> Scope {
        Scope::with_parent(self.get_current_scope().clone())
    }

    fn eval_list(&mut self, vec: Vec<Node>) -> Vec<Value> {
        let mut arguments = Vec::new();

        for node in vec {
            if let Some(node) = self.eval(node) {
                arguments.push(node)
            }
        }
        arguments
    }

    pub fn run(&mut self) -> Option<Value> {
        let nodes = std::mem::take(&mut self.nodes);
        for node in nodes {
            self.eval(node);
        }
        debug!("{self:?}");
        self.return_value.take()
    }
}
#[derive(Debug, Clone)]
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

    // fn call_method(&mut self)
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
            Self::String(s) => format!("{s:?}"),
            Self::Array(v) => vec_to_string(v),
            //temp
            Self::Object(o) => o.to_string(),
        };
        write!(f, "{string_value}")
    }
}

#[derive(Debug, Clone)]
enum MathError {
    InvalidTypeLeft,
    InvalidTypeRight,
    UnexpectedNegativeInt,
    DivisionByZero,
}

#[derive(Debug, Clone)]
pub struct Scope {
    parent_scope: Option<Box<Scope>>,
    vars: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
    classes: HashMap<String, Rc<Class>>,
}
impl Scope {
    fn new() -> Self {
        Self {
            parent_scope: None,
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
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
            classes: HashMap::new(),
        }
    }

    pub fn add_var(&mut self, name: String, var: Variable) -> Result<(), AlreadyExists> {
        if self.vars.contains_key(&name) {
            return Err(AlreadyExists);
        }

        self.vars.insert(name, var);
        Ok(())
    }

    fn update_var(&mut self, name: &str, var: Value) -> Result<(), DoesNotExist> {
        match self.vars.get_mut(name) {
            Some(value) => {
                *value.get_mut() = var;
                Ok(())
            }
            None => {
                if let Some(parent) = &self.parent_scope {
                    if parent.vars.contains_key(name) {
                        warn!("tried to update read only variable from parent scope")
                    }
                }
                Err(DoesNotExist)
            }
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

    pub fn get_var_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.vars.get_mut(name).or_else(|| match &mut self.parent_scope {
            Some(scope) => scope.get_var_mut(name),
            None => None,
        })
    }
    pub fn get_var(&self,name: &str)->Option<&Variable>{
        self.vars.get(name).or_else(|| match &self.parent_scope {
            Some(scope) => scope.get_var(name),
            None => None,
        })
    }

    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name).or(match &self.parent_scope {
            Some(s) => s.get_function(name),
            None => None,
        })
    }
    pub fn add_class(&mut self, class: Class) -> Result<(), AlreadyExists> {
        if self.classes.contains_key(&class.name) {
            return Err(AlreadyExists);
        }

        self.classes.insert(class.name.clone(), Rc::new(class));
        Ok(())
    }
    pub fn get_class(&self, name: &str) -> Option<Rc<Class>> {
        self.classes
            .get(name)
            .cloned()
            .or(match &self.parent_scope {
                Some(s) => s.get_class(name),
                None => None,
            })
    }
}

#[derive(Debug)]
pub struct AlreadyExists;
#[derive(Debug)]
pub struct DoesNotExist;

#[derive(Debug, Clone)]
pub struct Variable {
    value: Value,
}
impl Variable {
    pub fn new(value: Value) -> Self {
        Self { value: value }
    }
    pub fn get(&self) -> &Value {
        &self.value
    }
    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.value
    }
}

#[derive(Debug, Clone)]
pub struct Class {
    name: String,
    fields: HashMap<String, Variable>,
    methods: HashMap<String, Function>,
}
impl Class {
    pub fn instantiate(self: &Rc<Self>) -> ClassInstance {
        ClassInstance {
            class: Rc::clone(self),
            fields: self.fields.clone(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ClassInstance {
    class: Rc<Class>,
    fields: HashMap<String, Variable>,
}
impl ClassInstance {
    pub fn call_method(
        &self,
        name: &str,
        args: Vec<Value>,
        mut scope: Scope,
    ) -> Result<Option<Value>, DoesNotExist> {
        let Some(method) = self.class.methods.get(name) else {
            return Err(DoesNotExist);
        };
        scope.vars.extend(self.fields.clone().into_iter());

        Ok(method.call(args, scope))
        // return Ok(None);
    }
}
impl Display for ClassInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = String::new();
        for (name, var) in &self.fields {
            fields += name;
            fields.push('=');
            fields += &var.get().to_string();
            fields.push(',');
        }
        fields.pop();

        write!(f, "{}Instance{{ fields:[{fields}] }}", self.class.name)
    }
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod runtime;
