use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use sv_parser;
use sv_parser::SyntaxTree;

// parse the SystemVerilog project according to the project root
pub fn parse_project(root: &PathBuf) -> Vec<SyntaxTree> {
    let mut set: HashSet<PathBuf> = HashSet::new();
    scan_folder(root, &mut set);
    parse_project_helper(&set, &root)
}

fn scan_folder(root: &PathBuf, map: &mut HashSet<PathBuf>) {
    if root.is_file() && root.extension().unwrap() == "sv" {
        map.insert(root.clone());
    } else {
        for item in root.read_dir().unwrap() {
            let entry = item.unwrap();
            scan_folder(&entry.path(), map);
        }
    }
}

fn parse_project_helper(map: &HashSet<PathBuf>, root: &PathBuf) -> Vec<SyntaxTree> {
    map.iter()
        .map(|i| parse_single_file(i, &vec![root]))
        .collect()
}

fn parse_single_file(path: &PathBuf, includes: &Vec<&PathBuf>) -> SyntaxTree {
    let defines = HashMap::new();
    let result = sv_parser::parse_sv(&path, &defines, includes, false, false);
    match result {
        Ok((ast, _)) => ast,
        Err(e) => {
            eprintln!("{}", e.to_string());
            panic!()
        }
    }
}
