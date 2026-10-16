#[cfg(feature = "tool-json")]
use minipy_native::{interpreter::InterpreterUnit, ir::AdmissionKind};
#[cfg(feature = "tool-json")]
use std::io::{self, BufRead};

#[cfg(feature = "tool-json")]
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.expect("pipe");
        let source: String = serde_json::from_str(&line).expect("JSON source");
        let outcome = match InterpreterUnit::prepare_script(&source, &[]) {
            Ok(_) => "accepted",
            Err(e) => match e.kind {
                AdmissionKind::Syntax => "syntax",
                AdmissionKind::Type => "type",
                AdmissionKind::Schema => "schema",
                AdmissionKind::Backend => "backend",
                AdmissionKind::Resource => "bound",
                AdmissionKind::Unsupported => {
                    if e.reason.contains("unsupported builtin 'sum'") {
                        "builtin_sum"
                    } else if e.reason.contains("unsupported builtin 'all'") {
                        "builtin_all"
                    } else if e.reason.contains("unsupported builtin 'any'") {
                        "builtin_any"
                    } else if e.reason.contains("unsupported builtin 'range'") {
                        "builtin_range"
                    } else if e.reason.contains("unsupported builtin 'enumerate'") {
                        "builtin_enumerate"
                    } else if e.reason.contains("unsupported builtin") {
                        "builtin_other"
                    } else if e.reason.contains("unsupported statement") {
                        "statement"
                    } else if e.reason.contains("unsupported expression") {
                        "expression"
                    } else if e.reason.contains("unknown") || e.reason.contains("unbound") {
                        "name"
                    } else if e.reason.contains("tool") || e.reason.contains("Tool") {
                        "tool"
                    } else if e.reason.contains("MAX_") || e.reason.contains("maximum") {
                        "bound"
                    } else {
                        "other_unsupported"
                    }
                }
            },
        };
        println!("{outcome}");
    }
}

#[cfg(not(feature = "tool-json"))]
fn main() {}
