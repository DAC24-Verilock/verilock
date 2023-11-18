use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter, Result};

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Primary {
    Int(i32),
    Unknown,
    Variable(Var),
}

impl Primary {
    pub fn show(&self) -> String {
        match self {
            Primary::Int(i) => i.to_string(),
            Primary::Unknown => String::from("?"),
            Primary::Variable(v) => v.name.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct Var {
    pub scope: String,
    pub name: String,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum BinRel {
    Eq,
    NotEq,
    Gt,
    Lt,
    Ge,
    Le,
}

impl BinRel {
    pub fn show(&self) -> String {
        match self {
            BinRel::Eq => String::from("=="),
            BinRel::NotEq => String::from("!="),
            BinRel::Gt => String::from(">"),
            BinRel::Lt => String::from("<"),
            BinRel::Ge => String::from(">="),
            BinRel::Le => String::from("<="),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum BoolExpression {
    True,
    False,
    Unknown,
    Binary(Primary, BinRel, Primary),
    Not(Box<BoolExpression>),
    And(Box<BoolExpression>, Box<BoolExpression>),
    Or(Box<BoolExpression>, Box<BoolExpression>),
}

impl BoolExpression {
    pub fn show(&self) -> String {
        match self {
            BoolExpression::True => String::from("true"),
            BoolExpression::False => String::from("false"),
            BoolExpression::Unknown => String::from("?"),
            BoolExpression::Binary(l, op, r) => format!("{}{}{}", l.show(), op.show(), r.show()),
            BoolExpression::Not(e) => format!("¬{}", e.show()),
            BoolExpression::And(l, r) => format!("{}∧{}", l.show(), r.show()),
            BoolExpression::Or(l, r) => format!("{}∨{}", l.show(), r.show()),
        }
    }

    pub fn invalidate_and_rebind_var(&self, v: &Var) -> BoolExpression {
        match self {
            BoolExpression::True => BoolExpression::True,
            BoolExpression::False => BoolExpression::False,
            BoolExpression::Unknown => BoolExpression::Unknown,
            BoolExpression::Binary(l, op, r) => {
                if let Primary::Variable(lv) = l {
                    if lv == v {
                        return BoolExpression::True;
                    }
                }
                if let Primary::Variable(rv) = r {
                    if rv == v {
                        return BoolExpression::True;
                    }
                }
                BoolExpression::Binary(l.clone(), op.clone(), r.clone())
            }
            BoolExpression::Not(e) => {
                let e = e.invalidate_and_rebind_var(v);
                if e == BoolExpression::True {
                    BoolExpression::True
                } else {
                    BoolExpression::Not(Box::new(e))
                }
            }
            BoolExpression::And(l, r) => {
                let l = l.invalidate_and_rebind_var(v);
                let r = r.invalidate_and_rebind_var(v);
                if l == BoolExpression::True {
                    r
                } else if r == BoolExpression::True {
                    l
                } else {
                    BoolExpression::And(Box::new(l), Box::new(r))
                }
            }
            BoolExpression::Or(l, r) => {
                let l = l.invalidate_and_rebind_var(v);
                let r = r.invalidate_and_rebind_var(v);
                if l == BoolExpression::True {
                    r
                } else if r == BoolExpression::True {
                    l
                } else {
                    BoolExpression::Or(Box::new(l), Box::new(r))
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct ModuleInstance {
    pub scope: String,
    pub type_name: String,
    pub instance_name: String,
}

impl ModuleInstance {
    pub fn group_parent(type_name: &String) -> ModuleInstance {
        ModuleInstance {
            scope: String::from(""),
            type_name: type_name.clone(),
            instance_name: String::from("$group_parent$"),
        }
    }
}

impl Display for ModuleInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}: {} in {}",
            self.instance_name, self.type_name, self.scope
        )
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum Channel {
    Instance(ChannelInstance),
    Ref(Var),
}

impl Channel {
    pub fn show(&self) -> String {
        match self {
            Channel::Instance(i) => i.instance_name.clone(),
            Channel::Ref(r) => r.name.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct ChannelInstance {
    pub scope: String,
    pub type_name: String,
    pub instance_name: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum PortDirection {
    Input,
    Output,
    Inout,
    Ref,
    Interface,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Port {
    pub direction: PortDirection,
    pub id: String,
    pub index: usize, /* start from 0 */
}

#[derive(Debug, PartialEq, Clone)]
pub struct ModuleInfo {
    pub module_name: String,
    pub ports: Vec<Port>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InterfaceInfo {
    pub interface_name: String,
    pub tasks: HashSet<String>,
}

pub fn parse_bin_rel(op_str: &str) -> Option<BinRel> {
    match op_str {
        "==" => Some(BinRel::Eq),
        "!=" => Some(BinRel::NotEq),
        ">" => Some(BinRel::Gt),
        "<" => Some(BinRel::Lt),
        ">=" => Some(BinRel::Ge),
        "<=" => Some(BinRel::Le),
        _ => None,
    }
}

impl Display for Primary {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Primary::Int(i) => write!(f, "{}", i),
            Primary::Unknown => write!(f, "?"),
            Primary::Variable(v) => write!(f, "{}", v.name),
        }
    }
}

impl Display for BinRel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let show = self.show();
        write!(f, "{show}")
    }
}

impl Display for BoolExpression {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            BoolExpression::True => write!(f, "true"),
            BoolExpression::False => write!(f, "false"),
            BoolExpression::Unknown => write!(f, "?"),
            BoolExpression::Binary(l, op, r) => {
                write!(f, "{} {} {}", l.to_string(), op.to_string(), r.to_string())
            }
            BoolExpression::Not(e) => write!(f, "¬{}", e.to_string()),
            BoolExpression::And(l, r) => write!(f, "{}∧{}", l.to_string(), r.to_string()),
            BoolExpression::Or(l, r) => write!(f, "{}∨{}", l.to_string(), r.to_string()),
        }
    }
}

pub fn merge_with_and(e1: BoolExpression, e2: BoolExpression) -> BoolExpression {
    merge_with(e1, e2, BoolExpression::And)
}

fn merge_with<F>(e1: BoolExpression, e2: BoolExpression, f: F) -> BoolExpression
where
    F: Fn(Box<BoolExpression>, Box<BoolExpression>) -> BoolExpression,
{
    if e1 == e2 {
        e1
    } else {
        if e1 == BoolExpression::Unknown || e2 == BoolExpression::Unknown {
            BoolExpression::Unknown
        } else if e1 == BoolExpression::True {
            e2
        } else if e2 == BoolExpression::True {
            e1
        } else {
            f(Box::new(e1), Box::new(e2))
        }
    }
}
