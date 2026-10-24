use crate::ast::VariableAttributes;
use crate::runtime::{ParentScope, Runtime};
use crate::{
    Value,
    functions::Function,
    runtime::{RuntimeError, RuntimeReturn, Scope, Variable},
};
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct RuntimeClass {
    pub(crate) class_path: ClassPath,
    pub(crate) fields: HashMap<String, Variable>,
    pub(crate) methods: HashMap<String, Function>,
    pub(crate) static_fields: HashMap<String, Variable>,
}
impl RuntimeClass {
    pub fn new(
        class_path: ClassPath,
        fields: HashMap<String, Variable>,
        methods: HashMap<String, Function>,
        static_fields: HashMap<String, Variable>,
    ) -> Result<Self, ClassDefinitionError> {
        Self::verify_field_names(&fields.keys().collect())?;
        Self::verify_field_names(&static_fields.keys().collect())?;

        if let Some((k, _)) = fields.iter().find(|(k, _)| static_fields.contains_key(*k)) {
            return Err(ClassDefinitionError::FieldNameDuplicate(k.to_owned()));
        }

        Ok(Self {
            class_path,
            fields,
            methods,
            static_fields,
        })
    }

    pub fn name(&self) -> &str {
        self.class_path.name()
    }

    pub fn instantiate(
        self: &Arc<Self>,
        arguments: Vec<crate::ast::Node>,
        scope: Scope,
    ) -> Result<RuntimeClassInstance, RuntimeError> {
        let mut instance = RuntimeClassInstance {
            class: Arc::clone(self),
            fields: self.fields.clone(),
        };
        log::debug!("instantiating class {self:?}");
        let mut runtime =
            instantiate_inner(scope, &self.class_path, arguments, self.fields.clone());
        runtime.run()?;
        instance.set_fields(runtime.get_current_scope().variables.clone());
        Ok(instance)
    }
}

pub fn instantiate_inner(
    mut scope: Scope,
    path: &ClassPath,
    mut arguments: Vec<crate::ast::Node>,
    fields: HashMap<String, Variable>,
) -> Runtime {
    //allows for Class(a:a);
    const INJECTED_VAR_NAME: &str = "@from_constructor";

    // let mut instance = class.instantiate();

    //adds INJECTED_VAR_NAME is the end of any variables so no name collision happens
    scope.parent_scope = if let ParentScope::Normal(n) = scope
        .parent_scope
        .as_ref()
        .expect("we declared a child scope")
    {
        Some(ParentScope::Normal(Box::new(Scope {
            variables: n
                .variables
                .clone()
                .into_iter()
                .map(|(k, v)| (k + INJECTED_VAR_NAME, v))
                .collect(),
            ..Scope::new(path.name().to_string() + "Constructor")
        })))
    } else {
        log::error!("child scope does not have parent");
        None
    };
    for argument in &mut arguments {
        if let crate::ast::Node::VarReasign { new_value, .. } = argument
            && let crate::ast::Node::VarRef(name) = &mut **new_value
        {
            *name += INJECTED_VAR_NAME
        }
    }
    scope.variables.extend(fields);

    // should run any VarAssigns for the class
    crate::runtime::Runtime::with_scope_and_nodes(arguments, scope)
}

impl PartialEq for RuntimeClass {
    fn eq(&self, other: &Self) -> bool {
        self.class_path == other.class_path
    }
}
impl RuntimeClass {
    fn verify_field_names(fields: &HashSet<&String>) -> Result<(), ClassDefinitionError> {
        let reserved_field_names = ["class".to_string()];
        for name in reserved_field_names {
            if fields.contains(&name) {
                return Err(ClassDefinitionError::ReservedFieldName(name.to_string()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum ClassDefinitionError {
    ReservedFieldName(String),
    FieldNameDuplicate(String),
}
impl Display for ClassDefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedFieldName(e) => write!(f, "reserved field name \"{e}\" used in class "),
            Self::FieldNameDuplicate(name) => write!(
                f,
                "two or more fields with the name {name} are defined in this class"
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeClassInstance {
    class: Arc<RuntimeClass>,
    fields: HashMap<String, Variable>,
}
impl RuntimeClassInstance {
    pub fn call_method(&mut self, name: &str, args: Vec<Value>, scope: Scope) -> RuntimeReturn {
        let Some(method) = self.class.methods.get(name) else {
            return Err(RuntimeError::DoesNotExist(name.to_owned()));
        };
        let mut scope = Scope::with_parent(scope);
        scope.variables = std::mem::take(&mut self.fields);
        let scope = Arc::new(Mutex::new(scope));

        let scope = Scope::with_mutable_parent(scope);
        let result = method.call(args, scope.clone());
        match scope.parent_scope {
            Some(p) => match p {
                ParentScope::Mut(m) => {
                    self.fields = {
                        let loc = m.lock().expect("we are already panicing");
                        loc.variables.clone()
                    }
                }
                ParentScope::Normal(_) => panic!("parent was set to mut"),
            },
            None => panic!("we set a parent"),
        };
        result
    }
    pub fn get_field(&self, name: &str) -> Option<&Variable> {
        self.fields.get(name)
    }

    pub fn get_field_mut(&mut self, name: &str) -> Result<&mut Variable, RuntimeError> {
        self.fields
            .get_mut(name)
            .ok_or(RuntimeError::DoesNotExist(format!(
                "field {name} of class {:?}",
                self.class
            )))
    }
    pub fn set_fields(&mut self, new: HashMap<String, Variable>) {
        self.fields = new;
    }
    pub fn class(&self) -> Arc<RuntimeClass> {
        Arc::clone(&self.class)
    }
}

impl PartialEq for RuntimeClassInstance {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class && map_equal(&self.fields, &other.fields)
    }
}

impl std::fmt::Display for RuntimeClassInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = String::new();
        for (name, var) in &self.fields {
            fields += name;
            fields.push(':');
            fields += &var.get_value().to_string();
            fields.push(',');
        }
        fields.pop();

        write!(f, "{}Instance {{ fields:[{fields}] }}", self.class.name())
    }
}

fn map_equal<K: Eq + Hash, V: PartialEq>(one: &HashMap<K, V>, two: &HashMap<K, V>) -> bool {
    one.iter()
        .all(|(k, v)| two.get(k).filter(|v2| *v2 == v).is_some())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassPath {
    path: String,
    name: String,
}
impl ClassPath {
    pub fn new(module_name: String, class_name: String) -> Self {
        Self {
            path: module_name,
            name: class_name,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Class {
    Runtime(Arc<RuntimeClass>),
    Native(Arc<dyn NativeClass>),
}
impl Class {
    pub fn name(&self) -> &str {
        match self {
            Self::Native(n) => n.name(),
            Self::Runtime(r) => r.name(),
        }
    }
    pub fn get_static_field(&self, name: &str) -> Option<Variable> {
        match self {
            Self::Native(n) => n.get_static_field(name),
            Self::Runtime(r) => r.static_fields.get(name).cloned(),
        }
    }
    pub fn instantiate(
        &self,
        args: Vec<crate::ast::Node>,
        scope: Scope,
    ) -> Result<ClassInstance, RuntimeError> {
        match self {
            Self::Runtime(r) => r
                .instantiate(args, scope)
                .map(|r| ClassInstance::Runtime(Arc::new(Mutex::new(r)))),
            Self::Native(n) => n
                .instantiate(n.clone(), args, scope)
                .map(ClassInstance::Native),
        }
    }
}
#[derive(Debug, Clone)]
pub enum ClassInstance {
    Runtime(Arc<Mutex<RuntimeClassInstance>>),
    Native(Arc<Mutex<dyn NativeClassInstance>>),
}
impl ClassInstance {
    pub fn call_method(&self, method_name: &str, args: Vec<Value>, scope: Scope) -> RuntimeReturn {
        match self {
            Self::Runtime(r) => r
                .lock()
                .expect("panicking")
                .call_method(method_name, args, scope),
            Self::Native(n) => n.lock().expect("panicking").call_method(method_name, args),
        }
    }
    pub fn get_field(&self, field_name: &str) -> Option<Variable> {
        match self {
            Self::Runtime(r) => r.lock().expect("panicking").get_field(field_name).cloned(),
            Self::Native(n) => n.lock().expect("panicking").get_field(field_name),
        }
    }
    pub fn get_field_mut(
        &self,
        field_name: &str,
        f: impl FnOnce(&mut Variable) -> Result<(), RuntimeError> + 'static,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Runtime(r) => r
                .lock()
                .expect("panicking")
                .get_field_mut(field_name)
                .and_then(f),
            Self::Native(n) => n
                .lock()
                .expect("panicking")
                .get_field_mut(field_name, Box::new(f)),
        }
    }
    pub fn class(&self) -> Class {
        match self {
            Self::Runtime(r) => Class::Runtime(r.lock().expect("panicking").class()),
            Self::Native(n) => Class::Native(n.lock().expect("panicking").class()),
        }
    }
}
impl PartialEq for ClassInstance {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Runtime(left), Self::Runtime(right)) => {
                *left.lock().expect("pancking") == *right.lock().expect("panicking")
            }
            (Self::Native(left), Self::Native(right)) => {
                *left.lock().expect("pancking") == *right.lock().expect("panicking")
            }
            //they couldnt possibly be equal
            (Self::Native(_), Self::Runtime(_)) => false,
            (Self::Runtime(_), Self::Native(_)) => false,
        }
    }
}
impl Display for ClassInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native(n) => write!(f, "{:?}", n.lock().expect("panicking")),
            Self::Runtime(r) => write!(f, "{}", r.lock().expect("panicking")),
        }
    }
}

#[derive(Debug)]
pub struct Static<T: Into<Value> + Debug + Clone>(T);
impl<T: Into<Value> + Debug + Clone> Static<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
    pub fn value(&self) -> &T {
        &self.0
    }
    // pub fn to_value(&self)->Value{
    //     self.0.clone().into()
    // }
    pub fn to_variable(&self) -> Variable {
        Variable::with_attributes(
            self.0.clone().into(),
            crate::runtime::VariableType::Any,
            VariableAttributes::attr_static(),
        )
        .expect("has the any type")
    }
}

pub trait NativeClassInstance: Debug + Send + Sync {
    fn call_method(&mut self, name: &str, args: Vec<Value>) -> RuntimeReturn;
    fn get_field(&self, name: &str) -> Option<Variable>;
    fn class(&self) -> Arc<dyn NativeClass>;
    #[allow(clippy::type_complexity)]
    fn get_field_mut(
        &mut self,
        name: &str,
        f: Box<dyn FnOnce(&mut Variable) -> Result<(), RuntimeError>>,
    ) -> Result<(), RuntimeError>;
}
impl PartialEq for dyn NativeClassInstance {
    fn eq(&self, other: &Self) -> bool {
        //TODO make this check fields
        self.class() == other.class()
    }
}

pub trait NativeClass: Debug + Send + Sync {
    fn instantiate(
        &self,
        this: Arc<dyn NativeClass>,
        values: Vec<crate::ast::Node>,
        scope: Scope,
    ) -> Result<Arc<Mutex<dyn NativeClassInstance>>, RuntimeError>;
    fn name(&self) -> &str;
    fn get_static_field(&self, name: &str) -> Option<Variable>;
}
impl PartialEq for dyn NativeClass {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}
