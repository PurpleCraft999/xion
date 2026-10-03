use crate::runtime::Value;
type Input = Vec<Value>;
type Output = Option<Value>;

pub fn print(values: Vec<Value>) -> Option<Value> {
    if values.len() == 1 {
        println!("{}", values[0])
    } else if values.is_empty() {
        println!();
    } else {
        for (i, value) in values.iter().enumerate() {
            print!("{value}");
            if i < values.len() - 1 {
                print!(",")
            }
        }
        println!()
    }
    None
}

pub fn input(_: Vec<Value>) -> Option<Value> {
    let mut out = String::new();
    match std::io::stdin().read_line(&mut out) {
        Ok(_) => (),
        Err(err) => println!("Error reading input steam {err}"),
    }
    Some(Value::String(out))
}

pub fn instantiate(_: Input) -> Output {
    log::error!("instantiate was called directly");
    None
}

// use crate::runtime::Variable;
// #[derive(xion_interpreter_proc_macros::MakeClass)]
// struct Name{
//     name:String
// }

