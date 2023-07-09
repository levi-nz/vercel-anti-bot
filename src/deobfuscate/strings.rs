use std::collections::VecDeque;
use std::default::Default;
use std::str::FromStr;
use swc_core::common::{Mark, Span};
use swc_core::common::errors::HANDLER;
use swc_core::common::util::take::Take;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use swc_core::ecma::ast::{ArrayLit, AssignExpr, BinaryOp, BinExpr, Callee, CallExpr, Decl, Expr, ExprStmt, FnDecl, Id, Ident, Lit, ModuleItem, Number, op, Program, Stmt, Str, VarDeclarator};
use swc_core::ecma::atoms::JsWord;
use swc_ecma_transforms::optimization::simplify::expr_simplifier;

/// Replaces obfuscated strings with the real strings.
pub struct Visitor;

impl VisitMut for Visitor {
    fn visit_mut_program(&mut self, program: &mut Program) {
        /// Matches `$e` to `Some`. If `None`, `$err` is emitted and the function returns.
        macro_rules! try_unwrap {
            ($e:expr, $err:expr) => {
                match $e {
                    Some(v) => v,
                    None => {
                        HANDLER.with(|handler| {
                            handler.err($err);
                        });
                        return;
                    }
                }
            };
        }

        // Find function that returns the obfuscated strings, along with the
        // initial obfuscated strings
        let mut obf_strings = FindObfuscatedStringsVisitor::default();
        program.visit_mut_children_with(&mut obf_strings);
        // The function Id that returns the obfuscated strings
        let get_obf_strings_fn_id = try_unwrap!(
            obf_strings.fn_id,
            "Couldn't find obfuscated strings function"
        );
        // The obfuscated strings
        let mut obfuscated_strings = try_unwrap!(
            obf_strings.obfuscated_strings,
            "Couldn't find obfuscated strings"
        );

        // Find the function that indexes the obfuscated strings array
        let mut index_fn_visitor = FindIndexFunctionVisitor::new(
            get_obf_strings_fn_id.clone()
        );
        program.visit_mut_children_with(&mut index_fn_visitor);
        // Offset and operand
        let index_data = try_unwrap!(index_fn_visitor.index, "Index data not found");
        // Index function id
        let index_fn_id = try_unwrap!(index_fn_visitor.fn_id, "Index function not found");

        // Find the expression that is used to calculate the answer
        // used for array modification
        let mut expr_visitor = FindObfExpression::new(
            get_obf_strings_fn_id.clone()
        );
        program.visit_mut_children_with(&mut expr_visitor);
        // The original expression
        let original_expr = try_unwrap!(
            expr_visitor.expr,
            "Array compute function not found"
        );
        // The answer to compare against
        let answer = try_unwrap!(expr_visitor.answer, "Answer not found");
        // The original obfuscated strings. We use this for failure checking
        // so we don't loop infinitely if we some how don't compute the
        // strings correctly.
        let original_obfuscated_strings = obfuscated_strings.clone();
        // Modify obfuscated_strings until we get the correct answer
        loop {
            // Convert parseInt calls to literal values
            let mut expr = original_expr.clone();
            let mut expr_evaluator = ExprVisitor::new(
                index_data.clone(),
                &obfuscated_strings
            );
            expr.visit_mut_children_with(&mut expr_evaluator);
            // Evaluate the expression using expr_simplifier transform
            let mut stmt = Stmt::Expr(ExprStmt {
                span: Default::default(),
                expr: Box::new(Expr::Bin(expr)),
            });
            let mut simplifier = expr_simplifier(
                Mark::new(),
                Default::default()
            );
            stmt.visit_mut_with(&mut simplifier);
            // Try get literal value
            if let Stmt::Expr(expr) = &stmt {
                if let Expr::Lit(lit) = &*expr.expr {
                    if let Lit::Num(n) = &lit {
                        if n.value == answer {
                            // Answer matches, stop looping
                            break;
                        }
                    }
                }
            }
            // Got NaN, or the wrong answer. Continue modifying the deque.
            let first = try_unwrap!(
                obfuscated_strings.pop_front(),
                "Obfuscated strings are empty"
            );
            obfuscated_strings.push_back(first);
            // If the deque becomes original_obfuscated_strings (what we started with)
            // then this means we failed to find the correct answer.
            // This shouldn't happen, but is here for safety purposes so we don't
            // loop forever.
            // In this case, we emit an error and return.
            if obfuscated_strings == original_obfuscated_strings {
                HANDLER.with(|handler| {
                    handler.err("Failed to compute obfuscated strings");
                });
                return;
            }
        }

        // Remove call expressions and related code
        let mut cleanup_visitor = CleanupVisitor::new(index_fn_id, index_data, &obfuscated_strings);
        program.visit_mut_children_with(&mut cleanup_visitor);
    }
}

/// Finds the function that returns the obfuscated strings
/// along with the obfuscated strings.
#[derive(Default)]
struct FindObfuscatedStringsVisitor {
    /// If we're currently inside a FunctionDeclaration.
    /// This is only used internally.
    is_inside_fn: bool,

    /// The [Id] of the function that returns the obfuscated strings.
    fn_id: Option<Id>,

    /// The obfuscated strings.
    obfuscated_strings: Option<VecDeque<JsWord>>
}

impl VisitMut for FindObfuscatedStringsVisitor {
    fn visit_mut_fn_decl(&mut self, fn_decl: &mut FnDecl) {
        // Stop visiting if we already found the function
        if self.fn_id.is_some() {
            return;
        }

        // Visit children of this function
        let old_is_inside_fn = self.is_inside_fn;
        self.is_inside_fn = true;
        fn_decl.visit_mut_children_with(self);
        self.is_inside_fn = old_is_inside_fn;

        // If the obfuscated strings were found then set fn_id
        if self.obfuscated_strings.is_some() {
            self.fn_id = Some(fn_decl.ident.to_id());
            fn_decl.take();
        }
    }

    fn visit_mut_array_lit(&mut self, array: &mut ArrayLit) {
        if !self.is_inside_fn {
            return;
        }

        let mut obfuscated_strings = VecDeque::new();
        for element in &array.elems {
            if let Some(v) = element {
                // Spread operator shouldn't be present on any elements
                if v.spread.is_some() {
                    return;
                }

                if let Expr::Lit(Lit::Str(s)) = &*v.expr {
                    obfuscated_strings.push_back(s.value.clone());
                } else {
                    // All elements should be string literals
                    return;
                }
            } else {
                // All elements should be Some
                return;
            }
        }

        self.obfuscated_strings = Some(obfuscated_strings);
    }
}

/// Finds the function that indexes the obfuscated strings.
struct FindIndexFunctionVisitor {
    /// The [Id] of the function that returns the obfuscated strings,
    /// obtained from [FindObfuscatedStringsVisitor].
    get_obfuscated_strings_fn: Id,

    /// If the visitor is currently inside a function declaration.
    /// This is only used internally.
    is_inside_fn_decl: bool,

    /// If the visitor is currently inside the index function.
    /// This is only used internally.
    is_inside_correct_fn: bool,

    /// The index data, containing the offset and the binary operator.
    index: Option<Index>,

    /// The [Id] of the function that indexes strings.
    fn_id: Option<Id>
}

#[derive(Copy, Clone)]
struct Index {
    /// The operand to use to get the real index.
    offset: f64,

    /// The operator to use with offset.
    op: BinaryOp
}

/// Computes a fake index into the real index.
fn get_index(index: u32, offset: u32, op: BinaryOp) -> Option<u32> {
    match op {
        BinaryOp::LShift => Some(index << offset),
        BinaryOp::RShift => Some(index >> offset),
        BinaryOp::ZeroFillRShift => Some(index >> offset),
        BinaryOp::Add => Some(index + offset),
        BinaryOp::Sub => Some(index - offset),
        BinaryOp::Mul => Some(index * offset),
        BinaryOp::Div => Some(index / offset),
        BinaryOp::Mod => Some(index % offset),
        BinaryOp::BitOr => Some(index | offset),
        BinaryOp::BitXor => Some(index ^ offset),
        BinaryOp::BitAnd => Some(index & offset),
        BinaryOp::Exp => Some(index.pow(offset)),
        _ => None
    }
}

impl FindIndexFunctionVisitor {
    /// Creates a new [FindIndexFunctionVisitor] with the obfuscated strings function identifier
    /// obtained from [FindObfuscatedStringsVisitor].
    fn new(get_obfuscated_strings_fn: Id) -> Self {
        Self {
            get_obfuscated_strings_fn,
            is_inside_fn_decl: false,
            is_inside_correct_fn: false,
            index: None,
            fn_id: None
        }
    }
}

impl VisitMut for FindIndexFunctionVisitor {
    fn visit_mut_fn_decl(&mut self, fn_decl: &mut FnDecl) {
        let old_is_inside_fn_decl = self.is_inside_fn_decl;
        self.is_inside_fn_decl = true;
        fn_decl.visit_mut_children_with(self);
        self.is_inside_fn_decl = old_is_inside_fn_decl;

        // Set fn_id.
        //
        // We add an extra check at the end as the function that
        // returns the obfuscated strings calls itself.
        if self.is_inside_correct_fn && self.fn_id.is_none() && self.get_obfuscated_strings_fn != fn_decl.ident.to_id() {
            self.fn_id = Some(fn_decl.ident.to_id());
            fn_decl.take();
        }

        // Reset state
        if self.is_inside_fn_decl && self.is_inside_correct_fn {
            self.is_inside_correct_fn = false;
        }
    }

    fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
        // Ignore if we're in the correct function
        if self.is_inside_correct_fn {
            return;
        }

        if let Callee::Expr(expr) = &call.callee {
            if let Expr::Ident(id) = expr.as_ref() {
                if id.to_id() == self.get_obfuscated_strings_fn {
                    self.is_inside_correct_fn = true;
                }
            }
        }
    }

    fn visit_mut_assign_expr(&mut self, assignment: &mut AssignExpr) {
        assignment.visit_mut_children_with(self);

        // Skip if we're not inside the correct function, or if we
        // already got the expression
        if !self.is_inside_correct_fn || self.index.is_some() {
            return;
        }

        // Skip if the assignment operator isn't "=",
        // or if the right side of the assignment isn't a BinaryExpression
        if assignment.op != op!("=") {
            return;
        }

        // Is the right side of the assignment a binary expression?
        if let Expr::Bin(bin) = &*assignment.right {
            // Is the right side of the binary expression a numeric literal?
            if let Expr::Lit(Lit::Num(n)) = &*bin.right {
                self.index = Some(Index {
                    offset: n.value,
                    op: bin.op
                });
            }
        }
    }
}

/// Finds the answer and expression.
struct FindObfExpression {
    /// The function that returns the obfuscated strings.
    get_obfuscated_strings_fn: Id,

    /// If we're inside the correct CallExpression.
    /// This is only used internally.
    is_inside_correct_call_expr: bool,

    /// The expected answer for the computation.
    answer: Option<f64>,

    /// The expression used to compute the potential answer.
    expr: Option<BinExpr>
}

impl FindObfExpression {
    fn new(get_obfuscated_strings_fn: Id) -> Self {
        Self {
            get_obfuscated_strings_fn,
            is_inside_correct_call_expr: false,
            answer: None,
            expr: None
        }
    }
}

impl VisitMut for FindObfExpression {
    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        // Is the first argument the function that returns the obfuscated strings?
        if let Some(fn_id_arg) = call_expr.args.get(0) {
            if let Expr::Ident(id) = &*fn_id_arg.expr {
                if id.to_id() != self.get_obfuscated_strings_fn {
                    return;
                }
            }
        }

        // Value to use to check if the obfuscation reverse process has completed
        let answer = if let Some(answer_arg) = call_expr.args.get(1) {
            if let Expr::Lit(Lit::Num(n)) = &*answer_arg.expr {
                n.value
            } else {
                return;
            }
        } else {
            return;
        };

        self.answer = Some(answer);
        call_expr.span.take();

        // Set state
        let old_is_inside_correct_call_expr = self.is_inside_correct_call_expr;
        self.is_inside_correct_call_expr = true;
        call_expr.visit_mut_children_with(self);
        self.is_inside_correct_call_expr = old_is_inside_correct_call_expr;
    }

    fn visit_mut_var_declarator(&mut self, declarator: &mut VarDeclarator) {
        if !self.is_inside_correct_call_expr {
            return;
        }
        declarator.visit_mut_children_with(self);

        if let Some(expr) = &declarator.init {
            if let Expr::Bin(bin) = &**expr {
                self.expr = Some(bin.clone());
            }
        }
    }
}

/// Replaces the `parseInt` calls in the expression and the calls to the index function.
struct ExprVisitor<'strings> {
    /// The index data.
    index_data: Index,

    /// The obfuscated strings.
    obfuscated_strings: &'strings VecDeque<JsWord>
}

impl<'strings> ExprVisitor<'strings> {
    fn new(index_data: Index, obfuscated_strings: &'strings VecDeque<JsWord>) -> Self {
        Self {
            index_data,
            obfuscated_strings
        }
    }
}

/// Parses a string as an integer, ignoring non-numeric characters.
/// This is the equivalent to `parseInt` in JavaScript.
fn atoi<F: FromStr>(input: &str) -> Result<F, <F as FromStr>::Err> {
    let i = input
        .find(|c: char| !c.is_numeric())
        .unwrap_or_else(|| input.len());

    input[..i].parse::<F>()
}

impl<'strings> VisitMut for ExprVisitor<'strings> {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if let Expr::Call(call) = expr {
            // Check if this is a call to parseInt
            if let Callee::Expr(callee_expr) = &call.callee {
                if let Expr::Ident(id) = &**callee_expr {
                    if id.sym.to_string() != "parseInt" {
                        return;
                    }
                } else {
                    return;
                }
            } else {
                return;
            }

            // Get call to get_index
            if let Some(argument) = call.args.get(0) {
                if let Expr::Call(get_index_call) = &*argument.expr {
                    // Is a CallExpression
                    if let Some(offset_arg) = get_index_call.args.get(0) {
                        if let Expr::Lit(Lit::Num(offset)) = &*offset_arg.expr {
                            // NaN as a node
                            let nan = Expr::Ident(
                                Ident::new(
                                    JsWord::from("NaN"),
                                    Span::default()
                                )
                            );

                            // Replace node
                            *expr = match get_index(offset.value as u32, self.index_data.offset as u32, self.index_data.op) {
                                Some(index) => {
                                    match self.obfuscated_strings.get(index as usize) {
                                        Some(s) => match atoi::<usize>(s.to_string().as_str()) {
                                            Ok(n) => Expr::Lit(Lit::Num(Number::from(n))),
                                            Err(_) => nan
                                        },
                                        None => nan
                                    }
                                },
                                None => nan
                            };
                        }
                    }
                }
            }
        }
    }
}

/// Replaces calls to the index function with the plaintext strings and
/// removes related code to string obfuscation.
struct CleanupVisitor<'strings> {
    /// The [Id] of the index function.
    index_fn_id: Id,

    /// The index data.
    index_data: Index,

    /// The deobfuscated strings.
    plaintext_strings: &'strings VecDeque<JsWord>
}

impl<'strings> CleanupVisitor<'strings> {
    fn new(index_fn_id: Id, index_data: Index, plaintext_strings: &'strings VecDeque<JsWord>) -> Self {
        Self {
            index_fn_id,
            index_data,
            plaintext_strings
        }
    }
}

impl<'strings> VisitMut for CleanupVisitor<'strings> {
    fn visit_mut_stmt(&mut self, s: &mut Stmt) {
        s.visit_mut_children_with(self);

        if let Stmt::Expr(expr_stmt) = s {
            if matches!(&*expr_stmt.expr, Expr::Invalid(..)) {
                s.take();
            }
        } else if let Stmt::Decl(Decl::Fn(fn_decl)) = s {
            if fn_decl.ident.is_dummy() {
                // Remove FunctionDeclaration's that return obfuscated strings and index
                // the obfuscated strings
                s.take();
            }
        }
    }

    // Remove empty statements
    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        stmts.visit_mut_children_with(self);

        stmts.retain(|s| !matches!(s, Stmt::Empty(..)));
    }

    // Remove empty ModuleItem's
    fn visit_mut_module_items(&mut self, stmts: &mut Vec<ModuleItem>) {
        stmts.visit_mut_children_with(self);
        stmts.retain(|stmt| !matches!(stmt, ModuleItem::Stmt(Stmt::Empty(..))));
    }

    // Remove invalid expressions
    fn visit_mut_exprs(&mut self, exprs: &mut Vec<Box<Expr>>) {
        exprs.visit_mut_children_with(self);
        exprs.retain(|expr| !matches!(**expr, Expr::Invalid(..)));
    }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        expr.visit_mut_children_with(self);

        if let Expr::Call(call_expr) = expr {
            // Remove marked CallExpression's
            if call_expr.span.is_dummy() {
                expr.take();
                return;
            }

            // Replace calls to index function with the plaintext string
            if let Callee::Expr(callee_expr) = &call_expr.callee {
                if let Expr::Ident(id) = &**callee_expr {
                    // Does the callee match the index function identifier?
                    if id.to_id() != self.index_fn_id {
                        return;
                    }

                    // Get index value
                    let index = if let Some(arg) = call_expr.args.get(0) {
                        if let Expr::Lit(Lit::Num(n)) = &*arg.expr {
                            n.value
                        } else {
                            return;
                        }
                    } else {
                        return;
                    };

                    // Replace call with literal value
                    if let Some(real_index) = get_index(index as u32, self.index_data.offset as u32, self.index_data.op) {
                        if let Some(plaintext) = self.plaintext_strings.get(real_index as usize) {
                            *expr = Expr::Lit(Lit::Str(Str::from(plaintext.clone())));
                        }
                    }
                }
            }
        }
    }
}
