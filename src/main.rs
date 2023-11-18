use verilock::analysis;
use verilock::task;
use verilock::task::Case;

use std::env;
use std::path::PathBuf;

fn main() {
    let vec: Vec<String> = env::args().collect();
    let args = &vec[1..];
    if args.is_empty() {
        perform_both_experiments();
    } else if args.len() == 1 {
        let arg = args.first().unwrap().to_uppercase();
        if arg == "RQ1" {
            rq1();
        } else if arg == "RQ2" {
            rq2();
        } else {
            println!("Unrecognizable command-line arg: {arg}")
        }
    } else if args.len() == 2 {
        let first = &args[0].to_uppercase();
        if first == "CHECK" {
            check(&args[1]);
        } else if first == "SINGLE" {
            single(&args[1]);
        } else {
            println!("Unrecognizable command-line args: {}", args.join(" "))
        }
    } else {
        println!("too many arguments: {}", args.join(" "));
    }
}

fn perform_both_experiments() {
    println!("Perform both experiments");
    rq1();
    rq2();
}

fn rq1() {
    task::EXPERIMENT1.iter().for_each(analyze_with_info);
}

fn rq2() {
    task::EXPERIMENT2.iter().for_each(analyze_with_info);
}

fn single(c: &String) {
    let case_name = c.to_uppercase();
    let case_name = case_name.as_str();
    match case_name {
        "CASE1" => analysis::analyze(&task::VC1),
        "CASE2" => analysis::analyze(&task::VC2),
        "CASE3" => analysis::analyze(&task::VC3),
        "CASE4" => analysis::analyze(&task::VC4),
        "CASE5" => analysis::analyze(&task::VC5),
        "CASE6" => analysis::analyze(&task::VC6),
        "CASE7" => analysis::analyze(&task::VC7),
        "CASE8" => analysis::analyze(&task::VC8),
        "CASE1D" => analysis::analyze(&task::VC1_),
        "CASE2D" => analysis::analyze(&task::VC2_),
        "CASE3D" => analysis::analyze(&task::VC3_),
        "CASE4D" => analysis::analyze(&task::VC4_),
        "CASE5D" => analysis::analyze(&task::VC5_),
        "CASE6D" => analysis::analyze(&task::VC6_),
        "CASE7D" => analysis::analyze(&task::VC7_),
        "CASE8D" => analysis::analyze(&task::VC8_),
        "GEN1" => analysis::analyze(&task::GEN1),
        "GEN2" => analysis::analyze(&task::GEN2),
        "GEN3" => analysis::analyze(&task::GEN3),
        "GEN4" => analysis::analyze(&task::GEN4),
        "GEN5" => analysis::analyze(&task::GEN5),
        "GEN6" => analysis::analyze(&task::GEN6),
        "GEN7" => analysis::analyze(&task::GEN7),
        "GEN8" => analysis::analyze(&task::GEN8),
        "GEN9" => analysis::analyze(&task::GEN9),
        "GEN10" => analysis::analyze(&task::GEN10),
        _ => panic!("invalid case name"),
    }
}

fn analyze_with_info(c: &Case) {
    c.get_name().map(print_boxed_name);
    println!("-------------------");
    analysis::analyze(c);
    println!("-------------------");
}

fn print_boxed_name(name: &str) {
    let len = name.len();
    let line = [String::from("+"), "-".repeat(len), String::from("+")].join("");
    println!("{}", line);
    println!("|{}|", name);
    println!("{}", line);
}

fn check(p: &String) {
    let case = Case {
        path: Box::new(PathBuf::from(p)),
        identifier: task::ID.clone(),
    };
    analysis::analyze(&case)
}
