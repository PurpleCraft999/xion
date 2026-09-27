use crate::runtime::Value;

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
