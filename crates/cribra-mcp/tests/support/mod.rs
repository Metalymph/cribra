//! Shared process harness for JSONL integration tests.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::LazyLock;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Value, json};
use wait_timeout::ChildExt;

static MODERN_SCHEMA: &str = include_str!("../schemas/2026-07-28.json");
static LEGACY_SCHEMA: &str = include_str!("../schemas/2025-11-25.json");

static MODERN_VALIDATOR: LazyLock<jsonschema::Validator> =
    LazyLock::new(|| message_validator(MODERN_SCHEMA).expect("modern schema compiles"));
static LEGACY_VALIDATOR: LazyLock<jsonschema::Validator> =
    LazyLock::new(|| message_validator(LEGACY_SCHEMA).expect("legacy schema compiles"));

fn message_validator(source: &str) -> Result<jsonschema::Validator, String> {
    let mut schema: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    schema
        .as_object_mut()
        .ok_or_else(|| "schema root is not an object".to_owned())?
        .insert("$ref".to_owned(), json!("#/$defs/JSONRPCMessage"));
    jsonschema::validator_for(&schema).map_err(|error| error.to_string())
}

pub fn validate_official(version: &str, value: &Value) {
    let validator = if version == "2026-07-28" {
        &*MODERN_VALIDATOR
    } else {
        &*LEGACY_VALIDATOR
    };
    if validator.is_valid(value) {
        return;
    }
    let mut details = String::new();
    for error in validator.iter_errors(value) {
        details.push_str(&format!("\n  {error} at {}", error.instance_path()));
    }
    panic!("message violates {version} schema: {value}{details}");
}

pub struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    stderr: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
    waited: Option<ExitStatus>,
}

impl ServerProcess {
    pub fn spawn() -> Self {
        Self::spawn_with_env(&[])
    }

    pub fn spawn_with_env(env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("spawn cribra-mcp");

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout pipe");
        let stderr_pipe = child.stderr.take().expect("stderr pipe");

        let (line_tx, lines) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if line_tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let stderr = Arc::new(Mutex::new(String::new()));
        let stderr_copy = Arc::clone(&stderr);
        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr_pipe);
            let mut buffer = String::new();
            let _ = reader.read_to_string(&mut buffer);
            if let Ok(mut target) = stderr_copy.lock() {
                *target = buffer;
            }
        });

        Self {
            child,
            stdin,
            lines,
            stderr,
            stderr_thread: Some(stderr_thread),
            waited: None,
        }
    }

    pub fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin still open");
        stdin.write_all(line.as_bytes()).expect("write request");
        stdin.write_all(b"\n").expect("write newline");
        stdin.flush().expect("flush request");
    }

    pub fn send(&mut self, value: &Value) {
        self.send_raw(&value.to_string());
    }

    pub fn next_line(&mut self, timeout: Duration) -> Option<String> {
        self.lines.recv_timeout(timeout).ok()
    }

    pub fn next_json(&mut self, timeout: Duration) -> Value {
        let line = self.next_line(timeout).expect("response before deadline");
        serde_json::from_str(&line).expect("stdout line is JSON")
    }

    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    pub fn wait(&mut self, timeout: Duration) -> Option<ExitStatus> {
        if let Some(status) = self.waited {
            return Some(status);
        }
        let status = self.child.wait_timeout(timeout).expect("wait child");
        if status.is_some() {
            self.waited = status;
        }
        status
    }

    pub fn stderr_text(&mut self) -> String {
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
        self.stderr
            .lock()
            .map_or_else(|_| String::new(), |text| text.clone())
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stdin.take();
        if self.waited.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
    }
}
