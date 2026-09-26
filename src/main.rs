use my_lang::token::Lexer;

fn main() {
    let lexer = Lexer::new("{}; let class classs this_is_a_name \"a string\"  \n ");
    let lexed = lexer.lex();
    println!("{:?}", lexed);
}
