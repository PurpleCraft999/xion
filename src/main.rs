use xion::{ast::AstBuilder, runtime::Runtime, token::Lexer};

fn main() {
    log::set_max_level(log::LevelFilter::Trace);
    log::set_boxed_logger(Box::new(simple_logger::SimpleLogger::new()))
        .expect("only returns error when the logger is already set");

    let file = std::fs::read_to_string("class.xn").unwrap();
    let lexer = Lexer::new(&file);
    let mut lexed = lexer.lex();
    lexed.retain(|s| s != &xion::token::Token::WhiteSpace);
    log::debug!("lexed: {:?}", lexed);
    let ast = AstBuilder::new(lexed);
    let nodes = ast.build();

    let _ = Runtime::start(nodes).run();
}
