use std::collections::{HashMap, HashSet};
use std::default::Default;
use swc_core::common::SyntaxContext;
use swc_core::common::util::take::Take;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use swc_core::ecma::ast::{Decl, Expr, FnDecl, Id, Ident, ModuleItem, Pat, Program, Stmt, VarDeclarator};
use swc_core::ecma::atoms::JsWord;

/// Replaces proxy variables with references to the real variable.
///
/// Example:
/// ```js
/// function doStuff() {}
/// function helloWorld() {
///     var a = doStuff;
///     return a();
/// }
/// ```
///
/// is replaced with:
///
/// ```js
/// function doStuff() {}
/// function helloWorld() {
///     return doStuff();
/// }
/// ```
#[derive(Default)]
pub struct Visitor {
    /// Function identifiers obtained from [FunctionVisitor].
    /// Used to check if an [Id] is a FunctionDeclaration.
    functions: HashSet<Id>,

    /// A map of [JsWord]'s along with the lowest [SyntaxContext].
    /// This is used for checking if an identifier's name exists
    /// in an upper scope to avoid name collisions.
    identifiers: HashMap<JsWord, SyntaxContext>,

    /// Variable replacements. The key is the variable [Id],
    /// and the value is the [JsWord] of the FunctionDeclaration
    /// being called.
    replacements: HashMap<Id, Ident>,

    /// Counter for generating unique names.
    new_name_counter: usize,

    /// Function identifier replacements.
    /// The key is the current function name, and the value is the new name.
    function_replacements: HashMap<Id, JsWord>
}

impl VisitMut for Visitor {
    fn visit_mut_program(&mut self, program: &mut Program) {
        /*
        Because of cases like these:

        var r = c;
        function c() {}

        We have to run a separate visitor first to get all the functions,
        instead of just traversing down.
         */
        let mut fn_visitor = FunctionVisitor::default();
        program.visit_mut_children_with(&mut fn_visitor);
        self.functions = fn_visitor.functions;
        self.identifiers = fn_visitor.identifiers;

        // Replace identifiers and remove variables
        program.visit_mut_children_with(self);

        // Rename functions that were marked for renaming due to collisions
        if !self.function_replacements.is_empty() {
            let mut renamer = RenameFunctionVisitor {
                replacements: self.function_replacements.clone()
            };
            program.visit_mut_children_with(&mut renamer);
        }
    }

    fn visit_mut_var_declarator(&mut self, declarator: &mut VarDeclarator) {
        declarator.visit_mut_children_with(self);

        // Is the declarator's name an identifier?
        if let Pat::Ident(var_id) = &declarator.name {
            // Is init an identifier?
            if let Some(expr) = &declarator.init {
                if let Expr::Ident(fn_id) = &**expr {
                    // Is the identifier a FunctionDeclaration?
                    if self.functions.contains(&fn_id.to_id()) {
                        /*
                        Get the replacement JsWord.

                        The replacement, in most cases, is `id.sym`, but there is
                        a special case we MUST handle to avoid breaking code.
                        Observe the following code:

                        var r = c;
                        function c() {}
                        function doStuff(c) {
                            r();
                        }

                        This looks normal at first, but the problem here is if we replace
                        r() with c(), then we'll be calling the parameter passed into the
                        function instead of the "c" function in the upper scope.
                        To avoid this, we have a map of JsWord -> SyntaxContext, each
                        value being the lowest SyntaxContext. We can check if a variable
                        of the same name exists in the upper scope by comparing the lowest
                        context (lowest_ctx) with the syntax context of the real function
                        being called (fn_id.span.ctxt).
                         */
                        let replacement_sym = if let Some(highest_ctx) = self.identifiers.get(&fn_id.sym) {
                            if fn_id.span.ctxt < *highest_ctx {
                                // Generate new name
                                let mut new_name = String::from("proxyFn");
                                new_name.push_str(self.new_name_counter.to_string().as_str());
                                self.new_name_counter += 1;

                                // Set function replacement
                                let replacement = JsWord::from(new_name);
                                self.function_replacements.insert(fn_id.to_id(), replacement.clone());

                                replacement
                            } else {
                                fn_id.sym.clone()
                            }
                        } else {
                            fn_id.sym.clone()
                        };

                        // Add a replacement
                        self.replacements.insert(var_id.to_id(), Ident {
                            span: fn_id.span.clone(),
                            sym: replacement_sym,
                            optional: false,
                        });
                        // Mark declarator for deletion
                        declarator.name.take();
                    }
                }
            }
        }
    }

    // Replace identifiers with their replacement.
    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if let Some(new_id) = self.replacements.get(&ident.to_id()) {
            *ident = new_id.clone();
        }
    }

    // All code below this line is for deleting marked nodes.

    // Remove marked declarators
    fn visit_mut_var_declarators(&mut self, declarators: &mut Vec<VarDeclarator>) {
        declarators.visit_mut_children_with(self);

        declarators.retain(|node| !node.name.is_invalid());
    }

    // Remove empty VariableDeclaration nodes
    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        stmt.visit_mut_children_with(self);

        if let Stmt::Decl(Decl::Var(var)) = stmt {
            if var.decls.is_empty() {
                stmt.take();
            }
        }
    }

    // Remove top-level statements.
    fn visit_mut_module_items(&mut self, stmts: &mut Vec<ModuleItem>) {
        stmts.visit_mut_children_with(self);

        stmts.retain(|stmt| !matches!(stmt, ModuleItem::Stmt(Stmt::Empty(..))));
    }
}

#[derive(Default)]
struct FunctionVisitor {
    /// Function [Id]'s.
    functions: HashSet<Id>,

    /// Identifier names and their highest (deepest) scopes.
    /// Used for collision checking.
    identifiers: HashMap<JsWord, SyntaxContext>,
}

impl VisitMut for FunctionVisitor {
    // Store FunctionDeclaration's identifiers so we can check in
    // visit_mut_var_declarator if an Id is a function
    fn visit_mut_fn_decl(&mut self, fn_decl: &mut FnDecl) {
        self.functions.insert(fn_decl.ident.to_id());
        fn_decl.visit_mut_children_with(self);
    }

    // Store the highest (deepest) scope index for each identifier.
    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        if let Some(v) = self.identifiers.get(&ident.sym) {
            if ident.span.ctxt > *v {
                self.identifiers.insert(ident.sym.clone(), ident.span.ctxt);
            }
        } else {
            self.identifiers.insert(ident.sym.clone(), ident.span.ctxt);
        }
    }
}

/// Renames functions.
struct RenameFunctionVisitor {
    replacements: HashMap<Id, JsWord>
}

impl VisitMut for RenameFunctionVisitor {
    fn visit_mut_fn_decl(&mut self, fn_decl: &mut FnDecl) {
        // Remove id from replacements and set the function name
        if let Some(new_name) = self.replacements.remove(&fn_decl.ident.to_id()) {
            fn_decl.ident.sym = new_name;

            // Visit children if we have remaining functions to replace
            if !self.replacements.is_empty() {
                fn_decl.visit_mut_children_with(self);
            }
        }
    }
}
