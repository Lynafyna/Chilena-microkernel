//! echo — print text to screen

pub fn run(args: &[&str]) {
    println!("{}", args.join(" "));
}
