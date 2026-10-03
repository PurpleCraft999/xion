use xion_interpreter::{runtime::Runtime};

fn main() {
    log::set_max_level(log::LevelFilter::Debug);
    log::set_boxed_logger(Box::new(simple_logger::SimpleLogger::new()))
        .expect("only returns error when the logger is already set");

    let nodes = xion_interpreter::parse_and_lex("xion_interpreter/test_programs/mod.xn");

    let _ = Runtime::main(nodes).run();
}
