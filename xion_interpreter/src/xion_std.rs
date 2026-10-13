use xion_interpreter_proc_macros::lib;

#[lib]
mod lang {
    use xion_interpreter_proc_macros::instance_method;

    // use super::*;
    #[native_function]
    fn print(value: Value, mut end: Option<String>) {
        // s();
        print!("{value}{}", end.get_or_insert("\n".to_string()))

        // if values.len() == 1 {
        //     println!("{}", values[0])
        // } else if values.is_empty() {
        //     println!();
        // } else {
        //     for (i, value) in values.iter().enumerate() {
        //         print!("{value}");
        //         if i < values.len() - 1 {
        //             print!(",")
        //         }
        //     }
        //     println!()
        // }
    }
    ///calls to_string on input
    #[native_function("str")]
    fn to_str(input: Value) -> String {
        input.to_string()
    }
    #[native_function("int")]
    fn to_int(input: Value) -> Option<i64> {
        Some(match input {
            Value::Bool(b) => b as i64,
            Value::Int(i) => i,
            Value::Float(f) => f.round() as i64,
            _ => return None,
        })
    }
    #[derive(Debug, NativeClass)]
    struct Test {
        #[value = "testing"]
        name: String,
        #[value = 4]
        id: Static<i64>,
    }

    #[xion_interpreter_proc_macros::native_class_impl]
    impl TestInstance {
        #[instance_method]
        fn name(&self) -> String {
            self.name.clone()
        }
        #[instance_method("clone")]
        fn me(&self) -> Self {
            TestInstance {
                name: "sub-test".to_string(),
            }
        }
    }
}
#[lib]
mod io {

    #[native_function]
    fn input() -> String {
        let mut out = String::new();
        match std::io::stdin().read_line(&mut out) {
            Ok(_) => (),
            Err(err) => println!("Error reading input steam {err}"),
        }
        out.trim().to_string()
    }
}

#[lib]
mod time {
    use std::time::{Duration, SystemTime};
    ///returns -1.0 if system time is before unix_epoch
    /// returns current time in seconds otherwise
    #[native_function]
    fn time() -> f64 {
        match std::time::SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(t) => t.as_secs_f64(),
            Err(s) => {
                log::error!(
                    "system time is behind the unix epoch by {} seconds",
                    s.duration().as_secs()
                );

                -1f64
            }
        }
    }
    /// returns 0 if slept
    ///
    /// returns 1 if time was negative
    ///
    /// sleeps for said amount of seconds
    #[native_function]
    fn sleep(time: f64) -> i64 {
        if time < 0.0 {
            1
        } else {
            std::thread::sleep(Duration::from_secs_f64(time));
            0
        }
    }
}

pub use stdlib::get_std_lib;

mod stdlib {
    use super::*;

    use crate::runtime::Scope;
    use std::{collections::HashMap, sync::OnceLock};
    static DEFAULT_LIBS: OnceLock<HashMap<String, Scope>> = OnceLock::new();

    macro_rules! lib {
        ($map:expr, $($name:expr=>$mod_name:ident),+ $(,)?) => {
            $(
                {
                let mut scope = crate::runtime::Scope::new(String::from("std"));
                // $block(&mut scope).expect("no names are the same in the same module");
                // let block:fn(&mut Scope)-> Result<(), RuntimeError> = $block;
                // block(&mut scope).expect("all names are unique");

                for (name,func) in $mod_name::get_std_lib_fn(){
                    scope.add_native_function(&name,func).expect("function name is unique");
                }
                for class in $mod_name::get_std_lib_classes(){
                    scope.add_class(crate::class::Class::Native(class)).expect("class names are unique");
                }




                $map.insert($name.to_string(),scope);
                }

            )+
        };
    }

    fn build_default_libs() -> HashMap<String, crate::runtime::Scope> {
        let mut map = HashMap::new();
        lib! {map,
            "lang"=>lang,
            "io"=>io,
            "time"=>time,
        };
        map
    }

    // pub fn add_fn_to_std_lib<F:NativeFunction>(std_fn_name:&str,func:F){

    // }

    fn get_libs() -> &'static HashMap<String, Scope> {
        DEFAULT_LIBS.get_or_init(build_default_libs)
    }

    pub fn get_std_lib(name: &str) -> Option<Scope> {
        get_libs().get(name).cloned()
    }
}
