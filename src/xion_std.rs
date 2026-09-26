use crate::runtime::Value;

pub fn print(values: Vec<Value>) -> Option<Value> {
    if values.len() == 1 {
        println!("{:?}", values[0])
    } else if values.is_empty() {
        println!();
    } else {
        println!("{:?}", values);
    }

    None
}
