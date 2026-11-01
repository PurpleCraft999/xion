use std::{fs, path::PathBuf};

use log::{debug, error};
use xion_interpreter::{functions::FunctionReturn, runtime::Runtime};

fn main() {
    log::set_max_level(log::LevelFilter::Info);
    log::set_boxed_logger(Box::new(simple_logger::SimpleLogger::new()))
        .expect("only returns error when the logger is already set");

    let main_file;

    #[cfg(debug_assertions)]
    {
        main_file = "xion_interpreter/test_programs/new_var_test.xn".to_string();
    }

    #[cfg(not(debug_assertions))]
    {
        let args: Vec<_> = std::env::args().collect();
        main_file = match args.get(1) {
            Some(s) => s.to_string(),
            None => {
                println!("please pass the main file");
                return;
            }
        };
    }
    let path = PathBuf::from(main_file);
    let nodes = match xion_interpreter::lex_and_build_ast(&fs::read_to_string(&path).unwrap()) {
        Ok(a) => a,
        Err(e) => {
            error!(target:"main","{e:?}");
            return;
        }
    };
    debug!("running main");

    let mut runtime = match Runtime::main(nodes.nodes) {
        Ok(r) => r,
        Err(e) => {
            error!(target:"main", "{e}");
            return;
        }
    };

    debug!("{runtime:?}");

    match runtime.run() {
        Ok(FunctionReturn::Value(v)) => println!("program finished with an output of {v}"),
        Ok(FunctionReturn::None) => println!("program finished with no output"),
        Err(e) => {
            log::error!(target:"main", "{e}");
        }
    }
}
