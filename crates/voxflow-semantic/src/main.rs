use anyhow::{bail, Result};
use voxflow_semantic::{generate_dataset, validate_dataset_dir, write_dataset_dir};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some("generate") => {
            let dir = args
                .next()
                .unwrap_or_else(|| "data/semantic-intent".to_string());
            let dataset = generate_dataset();
            write_dataset_dir(&dataset, &dir)?;
            let report = validate_dataset_dir(dir)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.passed {
                bail!("generated dataset did not pass validation");
            }
            Ok(())
        }
        Some("validate") => {
            let dir = args
                .next()
                .unwrap_or_else(|| "data/semantic-intent".to_string());
            let report = validate_dataset_dir(dir)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.passed {
                bail!("dataset validation failed");
            }
            Ok(())
        }
        Some(other) => bail!("unknown command: {other}"),
    }
}

fn print_help() {
    println!(
        "voxflow-semantic 0.3.0\n\nUSAGE:\n  voxflow-semantic generate [data-dir]\n  voxflow-semantic validate [data-dir]\n"
    );
}
