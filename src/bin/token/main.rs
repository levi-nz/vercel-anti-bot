use std::env;
use vercel_anti_bot::generate_token;

// Generates a valid token from the given response from the /openai.jpeg request.
fn main() {
    let args: Vec<String> = env::args().collect();
    let data = match args.get(1) {
        Some(v) => v,
        None => {
            println!("You must pass in the challenge data.");
            println!("This can be obtained from the request to /openai.jpeg. Read README for more info.");
            return;
        }
    };

    let token = generate_token(data.as_str())
        .expect("failed to generate token");

    println!("{}", token);
}
