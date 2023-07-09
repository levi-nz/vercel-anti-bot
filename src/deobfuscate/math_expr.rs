use swc_core::common::Mark;
use swc_core::ecma::ast::{ArrayLit, Callee, Expr, ExprStmt, FnExpr, Id, Lit, MemberProp, Number, Pat, Stmt};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use swc_ecma_transforms::optimization::simplify::expr_simplifier;

/// Computes the math expression, which resolves the challenge answer.
pub struct Visitor {
    /// The input value from the challenge.
    input: f64,

    /// The [Id] of the parent function's input parameter.
    input_param: Option<Id>,

    /// If we're inside an ArrayLiteral.
    is_inside_array_lit: bool,

    /// If we're inside the math expression.
    is_inside_correct_expr: bool,

    /// The computed answer from the math expression.
    pub answer: Option<f64>
}

impl Visitor {
    /// Constructs a new [Visitor] with the input from the challenge.
    pub fn new(input: f64) -> Self {
        Self {
            input,
            input_param: None,
            is_inside_array_lit: false,
            is_inside_correct_expr: false,
            answer: None
        }
    }
}

/// Gets the value of a `Math` field, like `Math.PI`.
/// If no field with the given name exists, `None` is returned.
fn get_field(field: &str) -> Option<f64> {
    use std::f64::consts::*;

    match field {
        "E" => Some(E),
        "LN10" => Some(LN_10),
        "LN2" => Some(LN_2),
        "LOG10E" => Some(LOG10_E),
        "LOG2E" => Some(LOG2_E),
        "PI" => Some(PI),
        "SQRT1_2" => Some(FRAC_1_SQRT_2),
        "SQRT2" => Some(SQRT_2),
        _ => None
    }
}

/// Computes a `Math` function call with the given arguments.
/// For example, `Math.max(1, 2)` will return `Some(2)`.
/// This function also handles missing arguments, ie `Math.sign()`
/// returns `Some(f64::NAN)`.
///
/// If the function with the given name isn't found, `None` is returned.
fn compute_call(fn_name: &str, args: &[f64]) -> Option<f64> {
    /// Gets the argument at index, defaulting to NaN if it doesn't exist.
    macro_rules! get_arg {
        ($index:expr) => {
            *args.get($index).unwrap_or(&f64::NAN)
        }
    }

    // Functions from:
    // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Math
    match fn_name {
        "abs" => Some(get_arg!(0).abs()),
        "acos" => Some(get_arg!(0).acos()),
        "acosh" => Some(get_arg!(0).acosh()),
        "asin" => Some(get_arg!(0).asin()),
        "asinh" => Some(get_arg!(0).asinh()),
        "atan" => Some(get_arg!(0).atan()),
        "atan2" => Some(get_arg!(0).atan2(get_arg!(1))),
        "atanh" => Some(get_arg!(0).atanh()),
        "cbrt" => Some(get_arg!(0).cbrt()),
        "ceil" => Some(get_arg!(0).ceil()),
        "clz32" => Some((get_arg!(0) as i32).leading_zeros() as f64),
        "cos" => Some(get_arg!(0).cos()),
        "cosh" => Some(get_arg!(0).cosh()),
        "exp" => Some(get_arg!(0).exp()),
        "expm1" => Some(get_arg!(0).exp_m1()),
        "floor" => Some(get_arg!(0).floor()),
        "fround" => Some(get_arg!(0) as f32 as f64),
        "hypot" => Some(get_arg!(0).hypot(get_arg!(1))),
        "imul" => Some((get_arg!(0) as i32 * get_arg!(1) as i32) as f64),
        "log" => Some(get_arg!(0).ln()),
        "log10" => Some(get_arg!(0).log10()),
        "log1p" => Some(get_arg!(0).ln_1p()),
        "log2" => Some(get_arg!(0).log2()),
        "max" => Some(get_arg!(0).max(get_arg!(1))),
        "min" => Some(get_arg!(0).min(get_arg!(1))),
        "pow" => Some(get_arg!(0).powf(get_arg!(1))),
        // "random" would go here, but it wouldn't make sense for them to use it
        "round" => Some(get_arg!(0).round()),
        "sign" => Some({
            let v = get_arg!(0);
            if v == f64::NAN {
                f64::NAN
            } else if v > 0.0 {
                1.0
            } else if v < 0.0 {
                -1.0
            } else {
                0.0
            }
        }),
        "sin" => Some(get_arg!(0).sin()),
        "sinh" => Some(get_arg!(0).sinh()),
        "sqrt" => Some(get_arg!(0).sqrt()),
        "tan" => Some(get_arg!(0).tan()),
        "tanh" => Some(get_arg!(0).tanh()),
        "trunc" => Some(get_arg!(0).trunc()),
        _ => None
    }
}

impl VisitMut for Visitor {
    fn visit_mut_fn_expr(&mut self, fn_expr: &mut FnExpr) {
        if !self.input_param.is_some() {
            if let Some(param) = fn_expr.function.params.get(0) {
                if let Pat::Ident(input_param) = &param.pat {
                    self.input_param = Some(input_param.to_id());
                }
            }
        }

        fn_expr.visit_mut_children_with(self);
    }

    fn visit_mut_array_lit(&mut self, array_lit: &mut ArrayLit) {
        if let Some(Some(_)) = array_lit.elems.get(0) {
            let old_is_inside_array_lit = self.is_inside_array_lit;
            self.is_inside_array_lit = true;
            array_lit.visit_mut_children_with(self);
            self.is_inside_array_lit = old_is_inside_array_lit;
        } else {
            array_lit.visit_mut_children_with(self);
        }
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if self.is_inside_array_lit {
            let old_is_inside_correct_expr = self.is_inside_correct_expr;
            self.is_inside_correct_expr = true;
            expr.visit_mut_children_with(self);

            if self.answer.is_none() && self.is_inside_correct_expr && !old_is_inside_correct_expr {
                let mut stmt = Stmt::Expr(ExprStmt {
                    span: Default::default(),
                    expr: Box::new(expr.clone())
                });
                let mut simplifier = expr_simplifier(
                    Mark::new(),
                    Default::default()
                );
                stmt.visit_mut_with(&mut simplifier);
                // Try to get literal value
                if let Stmt::Expr(expr_stmt) = &stmt {
                    if let Expr::Lit(Lit::Num(number)) = &*expr_stmt.expr {
                        self.answer = Some(number.value);
                        *expr = Expr::Lit(Lit::Num(Number::from(number.value)));
                    }
                }
            }

            self.is_inside_correct_expr = old_is_inside_correct_expr;
        } else {
            expr.visit_mut_children_with(self);
        }

        if !self.is_inside_correct_expr {
            return;
        }

        if let Expr::Ident(id) = expr {
            // Handle input parameter
            if let Some(input_param) = &self.input_param {
                // Ignore identifiers that aren't the input parameter
                if id.to_id() != *input_param {
                    return;
                }
                // Replace identifier with input value
                *expr = Expr::Lit(Lit::Num(Number::from(self.input)));
            }
        } else if let Expr::Member(member_expr) = expr {
            // Handle expressions like Math.PI
            if let Expr::Ident(obj) = &*member_expr.obj {
                // Ignore non-Math objects
                if obj.sym.to_string().as_str() != "Math" {
                    return;
                }
                // Get property as &str
                let field_name = if let MemberProp::Ident(id) = &member_expr.prop {
                    id.sym.clone().to_string()
                } else {
                    return;
                };
                // Replace field with value
                if let Some(value) = get_field(field_name.as_str()) {
                    *expr = Expr::Lit(Lit::Num(Number::from(value)));
                }
            }
        } else if let Expr::Call(call_expr) = expr {
            // Handle calls like Math.max(1, 2)

            // Get callee as MemberExpression
            let member_expr = if let Callee::Expr(callee) = &call_expr.callee {
                if let Expr::Member(member_expr) = &**callee {
                    member_expr
                } else {
                    return;
                }
            } else {
                return;
            };

            // Get property as Identifier
            let obj = if let Expr::Ident(id) = &*member_expr.obj {
                id
            } else {
                return;
            };

            // Ignore non-Math objects
            if obj.sym.to_string() != "Math" {
                return;
            }

            // Get function name as &str
            let fn_name = if let MemberProp::Ident(property) = &member_expr.prop {
                property.sym.to_string()
            } else {
                return;
            };

            // Convert arguments to f64
            let args: Vec<f64> = call_expr.args
                .clone()
                .into_iter()
                .map(|arg| {
                    if let Expr::Lit(Lit::Num(number)) = &*arg.expr {
                        number.value
                    } else {
                        // Try evaluate the expression
                        let mut stmt = Stmt::Expr(ExprStmt {
                            span: Default::default(),
                            expr: Box::new(*arg.expr.clone())
                        });
                        let mut simplifier = expr_simplifier(
                            Mark::new(),
                            Default::default()
                        );
                        stmt.visit_mut_with(&mut simplifier);
                        // Try to get literal value
                        if let Stmt::Expr(expr_stmt) = &stmt {
                            if let Expr::Lit(Lit::Num(number)) = &*expr_stmt.expr {
                                number.value
                            } else {
                                f64::NAN
                            }
                        } else {
                            f64::NAN
                        }
                    }
                })
                .collect::<Vec<f64>>();
            // Compute result
            if let Some(result) = compute_call(fn_name.as_str(), &args) {
                *expr = Expr::Lit(Lit::Num(Number::from(result)));
            }
        }
    }
}