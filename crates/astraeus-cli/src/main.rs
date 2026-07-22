use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

use astraeus_artifacts::CalculationArtifact;
use astraeus_core::{CalculationRequest, EphemerisAdapter};
use astraeus_swiss::SwissEphemerisAdapter;
use serde_json::json;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("astraeus: {error}");
            eprintln!("usage: astraeus chart cast [REQUEST|-] [--pretty]");
            eprintln!("       astraeus artifact <canonicalize|inspect> [PATH|-] [--pretty]");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    let [group, command, rest @ ..] = args.as_slice() else {
        return Err("a command is required".into());
    };
    if group != "artifact" && group != "chart" {
        return Err(format!("unknown command group `{group}`"));
    }
    let pretty = rest.iter().any(|arg| arg == "--pretty");
    let paths = rest
        .iter()
        .filter(|arg| arg.as_str() != "--pretty")
        .collect::<Vec<_>>();
    if paths.len() > 1 {
        return Err("only one artifact path may be supplied".into());
    }
    let input = read_input(paths.first().map(|path| path.as_str()))?;
    if group == "chart" {
        if command != "cast" {
            return Err(format!("unknown chart command `{command}`"));
        }
        let request: CalculationRequest = serde_json::from_str(&input)
            .map_err(|error| format!("invalid calculation request: {error}"))?;
        let result = SwissEphemerisAdapter::moshier()
            .calculate(&request)
            .map_err(|error| format!("chart calculation failed: {error}"))?;
        let artifact = CalculationArtifact::new(request, result)
            .map_err(|error| format!("could not build calculation artifact: {error}"))?;
        return if pretty {
            artifact.to_pretty_json().map_err(|error| error.to_string())
        } else {
            artifact.to_json().map_err(|error| error.to_string())
        };
    }
    let artifact = CalculationArtifact::from_json(&input)
        .map_err(|error| format!("invalid calculation artifact: {error}"))?;
    match command.as_str() {
        "canonicalize" => artifact
            .to_json()
            .map_err(|error| error.to_string())
            .and_then(|canonical| {
                if pretty {
                    artifact.to_pretty_json().map_err(|error| error.to_string())
                } else {
                    Ok(canonical)
                }
            }),
        "inspect" => serde_json::to_string_pretty(&json!({
            "kind": "calculation",
            "content_id": artifact.content_id().map_err(|error| error.to_string())?,
            "schema_version": astraeus_artifacts::SCHEMA_VERSION,
            "instant": artifact.request().instant().as_datetime().to_rfc3339(),
            "zodiac": format!("{:?}", artifact.request().zodiac()),
            "objects": artifact.request().objects().len(),
        }))
        .map_err(|error| error.to_string()),
        _ => Err(format!("unknown artifact command `{command}`")),
    }
}

fn read_input(path: Option<&str>) -> Result<String, String> {
    match path.unwrap_or("-") {
        "-" => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|error| format!("could not read stdin: {error}"))?;
            Ok(input)
        }
        path => fs::read_to_string(path).map_err(|error| format!("could not read {path}: {error}")),
    }
}
