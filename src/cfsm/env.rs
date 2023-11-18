use crate::abstraction::protocol::Update;
use crate::abstraction::sv_info::{BinRel, BoolExpression, Primary};
use crate::error::{UnsolvableConstraints, VerilockError};
use im::HashSet;
use z3::ast::Ast;
use z3::{ast, Context, SatResult, Solver};

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct Environment {
    pub env: HashSet<BoolExpression>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            env: HashSet::new(),
        }
    }

    pub fn extend(&self, e: &BoolExpression) -> Environment {
        Environment {
            env: self.env.update(e.clone()),
        }
    }

    pub fn update(&self, u: &Update) -> Environment {
        let old_env: HashSet<BoolExpression> = self
            .env
            .iter()
            .map(|e| e.invalidate_and_rebind_var(&u.var))
            .collect();
        Environment {
            env: old_env.update(BoolExpression::Binary(
                Primary::Variable(u.var.clone()),
                BinRel::Eq,
                u.primary.clone(),
            )),
        }
    }

    pub fn satisfiable(&self, solver: &Solver) -> Result<bool, VerilockError> {
        unsafe {
            solver.push();
            let ctx = solver.get_context();
            for e in &self.env {
                solver.assert(&encode_bool_expression(&ctx, e));
            }
            match solver.check() {
                SatResult::Unsat => {
                    solver.pop(1);
                    Ok(false)
                }
                SatResult::Unknown => {
                    solver.pop(1);
                    Err(VerilockError::UnsolvableConstraints(
                        UnsolvableConstraints {
                            constraints: solver
                                .get_assertions()
                                .iter()
                                .map(|c| c.to_string())
                                .collect(),
                        },
                    ))
                }
                SatResult::Sat => {
                    solver.pop(1);
                    Ok(true)
                }
            }
        }
    }
}

fn encode_bool_expression<'a>(ctx: &'a Context, e: &BoolExpression) -> ast::Bool<'a> {
    match e {
        BoolExpression::True => ast::Bool::from_bool(&ctx, true),
        BoolExpression::False => ast::Bool::from_bool(&ctx, false),
        BoolExpression::Unknown => ast::Bool::from_bool(&ctx, true),
        BoolExpression::Binary(l, op, r) => {
            let l = encode_primary(ctx, l);
            let r = encode_primary(ctx, r);
            if l.is_none() || r.is_none() {
                ast::Bool::from_bool(&ctx, true)
            } else {
                let l = l.unwrap();
                let r = r.unwrap();
                match op {
                    BinRel::Eq => l._eq(&r),
                    BinRel::NotEq => l._eq(&r).not(),
                    BinRel::Gt => l.gt(&r),
                    BinRel::Lt => l.lt(&r),
                    BinRel::Ge => l.ge(&r),
                    BinRel::Le => l.le(&r),
                }
            }
        }
        BoolExpression::Not(sub) => encode_bool_expression(ctx, sub).not(),
        BoolExpression::And(l, r) => ast::Bool::and(
            ctx,
            &[
                &encode_bool_expression(ctx, l),
                &encode_bool_expression(ctx, r),
            ],
        ),
        BoolExpression::Or(l, r) => ast::Bool::or(
            ctx,
            &[
                &encode_bool_expression(ctx, l),
                &encode_bool_expression(ctx, r),
            ],
        ),
    }
}

fn encode_primary<'a>(ctx: &'a Context, p: &Primary) -> Option<ast::Int<'a>> {
    match p {
        Primary::Variable(v) => Some(ast::Int::new_const(ctx, format!("{}.{}", v.scope, v.name))),
        Primary::Int(i) => Some(ast::Int::from_i64(ctx, *i as i64)),
        Primary::Unknown => None,
    }
}
