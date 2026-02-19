//! Integration tests for mermaid-cli

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_test_dir() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("test.mmd");
    let output_path = temp_dir.path().join("output.svg");
    (temp_dir, input_path, output_path)
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("mermaid"))
        .stdout(predicate::str::contains("-i"))
        .stdout(predicate::str::contains("-o"))
        .stdout(predicate::str::contains("-f"));
}

#[test]
fn test_cli_basic_svg_rendering() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap()]);
    let output = cmd.output().unwrap();

    if !output.status.success() {
        panic!(
            "Command failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let actual_output = input_path.with_extension("svg");
    println!("Looking for output at: {:?}", actual_output);
    assert!(
        actual_output.exists(),
        "Output file should exist at {:?}",
        actual_output
    );

    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_png_rendering_with_format_flag() {
    let (temp_dir, input_path, output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let png_output = output_path.with_extension("png");

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args([
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        png_output.to_str().unwrap(),
        "-f",
        "png",
    ]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(png_output.exists(), "Output file should exist");
    let png_bytes = fs::read(&png_output).expect("Failed to read output");
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "Output should be valid PNG"
    );
}

#[test]
fn test_cli_png_rendering_by_extension() {
    let (temp_dir, input_path, output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let png_output = output_path.with_extension("png");

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args([
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        png_output.to_str().unwrap(),
    ]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(png_output.exists(), "Output file should exist");
    let png_bytes = fs::read(&png_output).expect("Failed to read output");
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
}

#[test]
fn test_cli_flowchart_diagram() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(
        &input_path,
        r#"flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[OK]
    B -->|No| D[Fail]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap()]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_pie_chart() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(
        &input_path,
        r#"pie title Test Pie Chart
    "Slice 1" : 30
    "Slice 2" : 70
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap()]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_sequence_diagram() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(
        &input_path,
        r#"sequenceDiagram
    Alice->>Bob: Hello
    Bob->>Alice: Hi!
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap()]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_gitgraph() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(
        &input_path,
        r#"gitGraph:
    commit "Initial"
    branch develop
    checkout develop
    commit "Feature work"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap()]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_with_theme() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap(), "-t", "dark"]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_with_background_color() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap(), "-b", "#ff0000"]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_with_width() {
    let (temp_dir, input_path, _output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["-i", input_path.to_str().unwrap(), "-w", "800"]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let actual_output = input_path.with_extension("svg");
    let svg_content = fs::read_to_string(&actual_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}

#[test]
fn test_cli_missing_input() {
    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("error")));
}

#[test]
fn test_cli_nonexistent_input_file() {
    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.args(["-i", "/nonexistent/path/file.mmd"]);
    cmd.assert().failure();
}

#[test]
fn test_cli_explicit_svg_format() {
    let (temp_dir, input_path, output_path) = setup_test_dir();
    fs::write(&input_path, "flowchart TD\n    A --> B\n").unwrap();

    let custom_output = output_path.with_extension("custom");

    let mut cmd = Command::cargo_bin("mermaid").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args([
        "-i",
        input_path.to_str().unwrap(),
        "-o",
        custom_output.to_str().unwrap(),
        "-f",
        "svg",
    ]);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let svg_content = fs::read_to_string(&custom_output).expect("Failed to read output");
    assert!(svg_content.contains("<svg"), "Output should be valid SVG");
}
