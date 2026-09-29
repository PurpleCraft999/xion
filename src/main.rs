use xion::{ast::AstBuilder, runtime::Runtime, token::Lexer};

fn main() {
    simple_logger::SimpleLogger::new()
        .init()
        .expect("only returns error when the logger is already set");

    let file = std::fs::read_to_string("class.xn").unwrap();
    let lexer = Lexer::new(&file);
    let mut lexed = lexer.lex();
    lexed.retain(|s| s != &xion::token::Token::WhiteSpace);
    log::debug!("lexed: {:?}", lexed);
    let ast = AstBuilder::new(lexed);
    let nodes = ast.build();

    Runtime::start(nodes).run();
}
