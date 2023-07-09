use std::fmt::{Debug, Display, Formatter};
use std::io::Write;
use std::sync::Arc;
use base64::alphabet::STANDARD;
use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use serde_json::Number;
use swc::config::IsModule;
use swc_core::common::{chain, FileName, GLOBALS, Globals, Mark, SourceMap};
use swc_core::common::errors::{EmitterWriter, Handler};
use swc_core::ecma::ast::EsVersion;
use swc_core::ecma::visit::as_folder;
use swc_ecma_parser::{EsConfig, Syntax};

pub mod deobfuscate;
mod shared_cursor;

/// A token generation error.
#[derive(Debug)]
pub enum GenerateTokenError {
    /// Failed to decode the "data" input.
    DataError(DecodeDataError),

    /// A JSON encoding error.
    JsonError(serde_json::Error),

    /// Failed to generate the math answer.
    GenerateAnswerError(GenerateAnswerError),
}

impl Display for GenerateTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataError(e) => write!(f, "DataError: {}", e),
            Self::JsonError(e) => write!(f, "JsonError: {}", e),
            Self::GenerateAnswerError(e) => write!(f, "GenerateAnswerError: {}", e)
        }
    }
}

impl std::error::Error for GenerateTokenError {}

impl From<DecodeDataError> for GenerateTokenError {
    fn from(err: DecodeDataError) -> Self {
        Self::DataError(err)
    }
}

impl From<serde_json::Error> for GenerateTokenError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError(err)
    }
}

impl From<GenerateAnswerError> for GenerateTokenError {
    fn from(err: GenerateAnswerError) -> Self {
        Self::GenerateAnswerError(err)
    }
}

/// A challenge request.
#[derive(serde::Deserialize)]
pub struct Challenge {
    /// The input value.
    #[serde(rename(deserialize = "a"))]
    pub input: f64,

    /// The code for the browser to evaluate to produce the answer.
    ///
    /// Note: this library does not evaluate JavaScript. It parses
    /// this code using SWC and computes the result with zero virtualization.
    /// See [generate_token] for more information.
    #[serde(rename(deserialize = "c"))]
    pub code: String,

    /// The challenge tag.
    #[serde(rename(deserialize = "t"))]
    pub tag: String
}

/// A data decoding error.
#[derive(Debug)]
pub enum DecodeDataError {
    /// A base64 decoding error.
    DecodeError(base64::DecodeError),

    /// A JSON parse error.
    JsonError(serde_json::Error)
}

impl Display for DecodeDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeError(e) => write!(f, "DecodeError: {}", e),
            Self::JsonError(e) => write!(f, "JsonError: {}", e)
        }
    }
}

impl std::error::Error for DecodeDataError {}

impl From<base64::DecodeError> for DecodeDataError {
    fn from(err: base64::DecodeError) -> Self {
        Self::DecodeError(err)
    }
}

impl From<serde_json::Error> for DecodeDataError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError(err)
    }
}

/// Decodes the given data.
pub fn decode_data(data: &str) -> Result<Challenge, DecodeDataError> {
    // JavaScript can produce padding, but may not.
    const PAD_OPTIONAL_CONFIG: GeneralPurposeConfig = GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent);
    const PAD_OPTIONAL: GeneralPurpose = GeneralPurpose::new(&STANDARD, PAD_OPTIONAL_CONFIG);

    // Decode from base64
    let decoded_data = PAD_OPTIONAL.decode(data)?;
    // Parse JSON
    Ok(serde_json::from_slice(&decoded_data)?)
}

/// A solved challenge.
#[derive(serde::Serialize)]
struct SolvedChallenge {
    /// The challenge answer.
    /// The first element is the produced answer from the math expression,
    /// the second is `Object.keys(globalThis.process || {})` (empty array in a legitimate
    /// environment), and the third element is `globalThis.marker`, which is currently
    /// set to `mark`.
    #[serde(rename(serialize = "r"))]
    answer: [serde_json::Value; 3],

    /// The challenge tag.
    #[serde(rename(serialize = "t"))]
    tag: String
}

/// Generates a token with the given response from the `/openai.jpeg` request.
pub fn generate_token(data: &str) -> Result<String, GenerateTokenError> {
    // Decode challenge
    let challenge = decode_data(data)?;

    // Generate math answer
    let math_answer = generate_answer(
        challenge.input,
        format!("({})", challenge.code)
    )?.unwrap_or(f64::NAN);
    // Create answer array
    let answer: [serde_json::Value; 3] = [
        // NaN and Infinity are not valid JSON.
        // JavaScript produces null for these cases in JSON.stringify,
        // so we do the same.
        Number::from_f64(math_answer)
            .map_or(
                serde_json::Value::Null,
                |n| serde_json::Value::Number(n)
            ),

        // Object.keys(globalThis.process || {})
        // Always an empty array in a legitimate environment.
        serde_json::Value::Array(Vec::new()),

        // globalThis.marker
        // Currently set in their JavaScript code to "mark" and
        // appears to be static.
        serde_json::Value::String(String::from("mark"))
    ];

    // Encode JSON
    let encoded = serde_json::to_vec(&SolvedChallenge {
        answer,
        tag: challenge.tag,
    })?;
    // Encode to base64
    Ok(base64::engine::general_purpose::STANDARD.encode(encoded))
}

#[derive(Debug)]
pub enum GenerateAnswerError {
    /// SWC failed to parse the JavaScript code.
    ParseError(anyhow::Error),

    /// Failed to parse transform error(s).
    TransformErrorParseError(std::string::FromUtf8Error),

    /// One or more errors were emitted from a transform.
    TransformErrors(Vec<String>)
}

impl Display for GenerateAnswerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(e) => write!(f, "ParseError: {}", e),
            Self::TransformErrorParseError(e) => write!(f, "TransformErrorParseError: {}", e),
            Self::TransformErrors(errs) => write!(f, "TransformErrors: {}", errs.join(", "))
        }
    }
}

impl std::error::Error for GenerateAnswerError {}

impl From<anyhow::Error> for GenerateAnswerError {
    fn from(err: anyhow::Error) -> Self {
        Self::ParseError(err)
    }
}

impl From<std::string::FromUtf8Error> for GenerateAnswerError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::TransformErrorParseError(err)
    }
}

impl From<Vec<String>> for GenerateAnswerError {
    fn from(errors: Vec<String>) -> Self {
        Self::TransformErrors(errors)
    }
}

/// Generates the answer to the challenge.
/// The returned `Option` is `None` if the expression couldn't be computed.
fn generate_answer(input: f64, code: String) -> Result<Option<f64>, GenerateAnswerError> {
    let cm = Arc::<SourceMap>::default();
    let err_dst = shared_cursor::SharedCursor::new();
    let handler = Handler::with_emitter(
        false,
        false,
        Box::new(EmitterWriter::new(
            Box::new(err_dst.clone()) as Box<dyn Write + Send>,
            None,
            true,
            false
        ))
    );
    let compiler = swc::Compiler::new(cm.clone());
    let fm = cm.new_source_file(FileName::Custom("input.js".into()), code);

    let mut answer = None;
    let globals = Globals::new();
    let mut parse_error = None;
    GLOBALS.set(&globals, || {
        // We can't return an error inside a closure, so we use a match instead.
        let program = match compiler.parse_js(
            fm,
            &handler,
            EsVersion::latest(), // Who knows what version they target, but this works
            Syntax::Es(EsConfig::default()),
            IsModule::Bool(false),
            None
        ) {
            Ok(v) => v,
            Err(e) => {
                parse_error = Some(e);
                return;
            }
        };

        // Run the transformations.
        use swc_ecma_transforms::optimization::simplify::expr_simplifier;
        use swc_ecma_transforms::resolver;
        // Visitor that computes the value we need
        let mut math_expr_visitor = deobfuscate::math_expr::Visitor::new(input);

        compiler.transform(&handler, program, true, chain!(
            // Squash the expressions like 4 + 5 * 2 into constant values
            expr_simplifier(Mark::new(), Default::default()),
            // Resolve identifiers to scope-aware values
            resolver(Mark::new(), Mark::new(), false),
            // Remove proxy variables
            as_folder(deobfuscate::proxy_vars::Visitor::default()),
            // Remove string obfuscation
            as_folder(deobfuscate::strings::Visitor),
            // Convert expressions like Math["floor"] to Math.floor
            as_folder(deobfuscate::computed_member_expr::Visitor),
            // Compute math expression to a constant value
            as_folder(&mut math_expr_visitor)
        ));

        // Set answer
        answer = math_expr_visitor.answer;
    });
    // Return deferred parse error
    if let Some(e) = parse_error {
        return Err(GenerateAnswerError::from(e));
    }

    // Parse emitted errors
    let errors: Vec<String> = String::from_utf8(err_dst.get_ref().unwrap().clone())?
        .split("\n")
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    // Return error if not empty
    if !errors.is_empty() {
        return Err(GenerateAnswerError::from(errors));
    }

    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test data taken from browser
    const TEST_DATA: &str = "eyJ0IjoiZXlKaGJHY2lPaUprYVhJaUxDSmxibU1pT2lKQk1qVTJSME5OSW4wLi4yMHA0T3VUcTFDVGRkVXRmLmhxMm4wbkVHOXFwZ2NlbWE2T1Rma1o0d3F2aTJ4SlJqaXd1YVhqTkZIai1ET1JRbDFyUGVaYXFDREdlc19sNXU5NFBTVHpnUHFlN3RNZGZxbUhGemVyRjBpNjJxSzlVV3Z1MDRaaG1iM3R1MjQ1eVJ2aGd1aXdtRmZONEt6VGcuYlRZTXBOZXg1cmhQNnpScFZUVG5NZyIsImMiOiJmdW5jdGlvbihhKXtmdW5jdGlvbiB4KGUscyl7dmFyIHQ9cigpO3JldHVybiB4PWZ1bmN0aW9uKG4saSl7bj1uLSgtODkxNSsyMjczKzMzODcqMik7dmFyIGM9dFtuXTtyZXR1cm4gY30seChlLHMpfShmdW5jdGlvbihlLHMpe2Zvcih2YXIgdD14LG49ZSgpO1tdOyl0cnl7dmFyIGk9cGFyc2VJbnQodCgxNDYpKS8xKigtcGFyc2VJbnQodCgxMzIpKS8yKStwYXJzZUludCh0KDE0MSkpLzMrcGFyc2VJbnQodCgxMzUpKS80KihwYXJzZUludCh0KDEzMykpLzUpKy1wYXJzZUludCh0KDEzOSkpLzYqKHBhcnNlSW50KHQoMTM3KSkvNykrcGFyc2VJbnQodCgxNDcpKS84KihwYXJzZUludCh0KDE0MikpLzkpK3BhcnNlSW50KHQoMTM0KSkvMTArcGFyc2VJbnQodCgxNDApKS8xMSooLXBhcnNlSW50KHQoMTQzKSkvMTIpO2lmKGk9PT1zKWJyZWFrO24ucHVzaChuLnNoaWZ0KCkpfWNhdGNoe24ucHVzaChuLnNoaWZ0KCkpfX0pKHIsLTk4MTA0MystMTMxNDEzKjUrMjI5ODEwMSk7ZnVuY3Rpb24gcigpe3ZhciBlPVtcIm1hcmtlclwiLFwia2V5c1wiLFwiMzEwODk4V21vbnBtXCIsXCI0NDcwNDU2SVFmZVZhXCIsXCI2S1BveGN4XCIsXCI3NzM5NWVUWHJTWFwiLFwiNTE4MjczMFZjcXRyZlwiLFwiMjI4eGVweWxhXCIsXCJsb2cxcFwiLFwiODQ3bXJJbmFHXCIsXCJwcm9jZXNzXCIsXCI2NTM1OG1KTGJVRlwiLFwiNDQzM1ZMS3JzclwiLFwiMjkxMzMxMlNQRlNpTVwiLFwiOVl0RkRXUlwiLFwiNTg4dUJIUU5MXCJdO3JldHVybiByPWZ1bmN0aW9uKCl7cmV0dXJuIGV9LHIoKX1yZXR1cm4gZnVuY3Rpb24oKXt2YXIgZT14O3JldHVyblthK01hdGhbZSgxMzYpXShhL01hdGguUEkpLE9iamVjdFtlKDE0NSldKGdsb2JhbFRoaXNbZSgxMzgpXXx8e30pLGdsb2JhbFRoaXNbZSgxNDQpXV19KCl9IiwiYSI6MC42NzM3ODM4NzE5MjA3MTEyfQ==";

    #[test]
    fn test_generate_token() {
        let result = generate_token(TEST_DATA)
            .expect("generate_token failed");

        // Token on right taken from browser
        assert_eq!(result, "eyJyIjpbMC44NjgwOTMzNDIwMDg1MDAxLFtdLCJtYXJrIl0sInQiOiJleUpoYkdjaU9pSmthWElpTENKbGJtTWlPaUpCTWpVMlIwTk5JbjAuLjIwcDRPdVRxMUNUZGRVdGYuaHEybjBuRUc5cXBnY2VtYTZPVGZrWjR3cXZpMnhKUmppd3VhWGpORkhqLURPUlFsMXJQZVphcUNER2VzX2w1dTk0UFNUemdQcWU3dE1kZnFtSEZ6ZXJGMGk2MnFLOVVXdnUwNFpobWIzdHUyNDV5UnZoZ3Vpd21GZk40S3pUZy5iVFlNcE5leDVyaFA2elJwVlRUbk1nIn0=");
    }
}
