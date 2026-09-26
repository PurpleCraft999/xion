use xion::{ast::AstBuilder, runtime::Runtime, token::Lexer};

fn main() {
    let file = std::fs::read_to_string("file.xn").unwrap();
    let lexer = Lexer::new(&file);
    let mut lexed = lexer.lex();
    lexed.retain(|s| s != &xion::token::Token::WhiteSpace);
    println!("lexed: {:?}", lexed);
    let ast = AstBuilder::new(lexed);
    let nodes = ast.build();

    Runtime::start(nodes).run();
}
