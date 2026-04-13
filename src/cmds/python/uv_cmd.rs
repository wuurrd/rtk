//! Handles `rtk uv run <tool>` by executing `uv run <tool>` with the
//! appropriate output filter applied. This preserves uv's virtualenv
//! context while still getting RTK's token savings.

use crate::cmds::python::{mypy_cmd, pytest_cmd, ruff_cmd};
use crate::core::runner;
use crate::core::utils::{resolved_command, strip_ansi};
use anyhow::Result;

pub fn run(args: &[String], verbose: u8) -> Result<i32> {
    match args.first().map(|s| s.as_str()) {
        Some("run") => dispatch_run(&args[1..], verbose),
        // `uv pip` is already handled by pip_cmd (auto-detects uv).
        // `uv sync` and other subcommands: passthrough.
        _ => run_passthrough(args, verbose),
    }
}

fn dispatch_run(args: &[String], verbose: u8) -> Result<i32> {
    // Skip optional `--` separator
    let args = if args.first().map(|s| s.as_str()) == Some("--") {
        &args[1..]
    } else {
        args
    };

    match args.first().map(|s| s.as_str()) {
        Some("pytest") => run_pytest(&args[1..], verbose),
        Some("ruff") => run_ruff(&args[1..], verbose),
        Some("mypy") => run_mypy(&args[1..], verbose),
        Some("python" | "python3") => dispatch_python_m(&args[1..], verbose),
        _ => run_passthrough_run(args, verbose),
    }
}

/// Handle `uv run python -m <tool> [args]`
fn dispatch_python_m(args: &[String], verbose: u8) -> Result<i32> {
    if args.len() >= 2 && args[0] == "-m" {
        match args[1].as_str() {
            "pytest" => return run_pytest(&args[2..], verbose),
            "mypy" => return run_mypy(&args[2..], verbose),
            _ => {}
        }
    }
    // Not a known `-m <tool>` pattern — passthrough the whole thing
    let mut full_args = vec!["python".to_string()];
    full_args.extend_from_slice(args);
    run_passthrough_run(&full_args, verbose)
}

fn run_pytest(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("uv");
    cmd.arg("run").arg("pytest");

    let has_tb_flag = args.iter().any(|a| a.starts_with("--tb"));
    let has_quiet_flag = args.iter().any(|a| a == "-q" || a == "--quiet");

    if !has_tb_flag {
        cmd.arg("--tb=short");
    }
    if !has_quiet_flag {
        cmd.arg("-q");
    }

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: uv run pytest {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "uv run pytest",
        &args.join(" "),
        pytest_cmd::filter_pytest_output,
        runner::RunOptions::stdout_only().tee("uv-pytest"),
    )
}

fn run_ruff(args: &[String], verbose: u8) -> Result<i32> {
    let is_check = args.is_empty()
        || args[0] == "check"
        || (!args[0].starts_with('-') && args[0] != "format" && args[0] != "version");

    let is_format = args.iter().any(|a| a == "format");

    let mut cmd = resolved_command("uv");
    cmd.arg("run").arg("ruff");

    if is_check {
        if !args.contains(&"--output-format".to_string()) {
            cmd.arg("check").arg("--output-format=json");
        } else {
            cmd.arg("check");
        }

        let start_idx = if !args.is_empty() && args[0] == "check" {
            1
        } else {
            0
        };
        for arg in &args[start_idx..] {
            cmd.arg(arg);
        }

        if args
            .iter()
            .skip(start_idx)
            .all(|a| a.starts_with('-') || a.contains('='))
        {
            cmd.arg(".");
        }
    } else {
        for arg in args {
            cmd.arg(arg);
        }
    }

    if verbose > 0 {
        eprintln!("Running: uv run ruff {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "uv run ruff",
        &args.join(" "),
        move |stdout| {
            if is_check && !stdout.trim().is_empty() {
                ruff_cmd::filter_ruff_check_json(stdout)
            } else if is_format {
                ruff_cmd::filter_ruff_format(stdout)
            } else {
                stdout.trim().to_string()
            }
        },
        runner::RunOptions::stdout_only(),
    )
}

fn run_mypy(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("uv");
    cmd.arg("run").arg("mypy");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: uv run mypy {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "uv run mypy",
        &args.join(" "),
        |raw| mypy_cmd::filter_mypy_output(&strip_ansi(raw)),
        runner::RunOptions::default(),
    )
}

/// Passthrough for `uv run <unknown-tool> [args]` — no filtering.
fn run_passthrough_run(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("uv");
    cmd.arg("run");
    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: uv run {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "uv run",
        &args.join(" "),
        |raw| raw.to_string(),
        runner::RunOptions::default(),
    )
}

/// Passthrough for `uv <subcommand>` (sync, etc.) — no filtering.
fn run_passthrough(args: &[String], verbose: u8) -> Result<i32> {
    let mut cmd = resolved_command("uv");
    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: uv {}", args.join(" "));
    }

    runner::run_filtered(
        cmd,
        "uv",
        &args.join(" "),
        |raw| raw.to_string(),
        runner::RunOptions::default(),
    )
}
