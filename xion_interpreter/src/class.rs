use crate::runtime::ParentScope;
use crate::{
    Value,
    functions::Function,
    runtime::{RuntimeError, RuntimeReturn, Scope, Variable},
};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
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

    pub fn instantiate(self: &Arc<Self>) -> RuntimeClassInstance {
        RuntimeClassInstance {
            class: Arc::clone(self),
            fields: self.fields.clone(),
        }
    }
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
    pub fn call_method(&mut self, name: &str, args: Vec<Value>) -> RuntimeReturn {
        let Some(method) = self.class.methods.get(name) else {
            return Err(RuntimeError::DoesNotExist(name.to_owned()));
        };
        let mut scope = Scope::new(self.class.name().to_string());
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

    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.fields.get_mut(name)
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

pub trait NativeClass {}
