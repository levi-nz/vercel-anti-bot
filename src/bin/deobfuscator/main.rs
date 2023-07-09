use std::env;
use std::sync::Arc;
use swc::config::Options;
use swc_core::common::errors::{ColorConfig, Handler};
use swc_core::common::{chain, FileName, Globals, GLOBALS, Mark, SourceMap};
use swc_core::common::comments::SingleThreadedComments;
use swc_core::ecma::transforms::base::pass::noop;
use swc_ecma_transforms::optimization::simplify::expr_simplifier;
use vercel_anti_bot::{decode_data, deobfuscate};
use swc_core::ecma::visit::as_folder;

// Deobfuscates the script from the given data.
// This is mainly intended for debug purposes.
fn main() {
    // Get data
    let args: Vec<String> = env::args().collect();
    let data = match args.get(1) {
        Some(v) => v,
        None => {
            println!("You must pass in the challenge data.");
            println!("This can be obtained from the request to /openai.jpeg. Read README for more info.");
            return;
        }
    };
    // Decode challenge
    let challenge = decode_data(data.as_str().trim())
        .expect("failed to decode challenge");

    let cm = Arc::<SourceMap>::default();
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));
    let c = swc::Compiler::new(cm.clone());
    let fm = cm.new_source_file(
        FileName::Custom("usage.js".into()),
        format!("({})", challenge.code).into()
    );

    let globals = Globals::new();
    GLOBALS.set(&globals, || {
        let output = c.process_js_with_custom_pass(
            fm,
            None,
            &handler,
            &Options::default(),
            SingleThreadedComments::default(),
            |_| noop(),
            |_| chain!(
                expr_simplifier(Mark::new(), Default::default()),
                as_folder(deobfuscate::proxy_vars::Visitor::default()),
                as_folder(deobfuscate::strings::Visitor),
                as_folder(deobfuscate::computed_member_expr::Visitor),
                as_folder(deobfuscate::math_expr::Visitor::new(0.2))
            )
        )
            .expect("process_js_with_custom_pass failed");

        println!("{}", output.code);
    });
}
