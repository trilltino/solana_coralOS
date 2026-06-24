//! Launches the side-car Python agents in `agent_demo/coral-agents/` as child
//! processes and streams their JSON stdout lines to the frontend as
//! `python-agent-event` Tauri events.
//!
//! One agent runs at a time; the running child is held in [`PythonAgentState`]
//! so it can be stopped. Each stdout line emitted by the Python agent is a JSON
//! object (see `agent.py`); we forward it verbatim, wrapping non-JSON lines as
//! `{"type":"log","message":…}`.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// Configuration sent from the UI to launch an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PythonAgentConfig {
    /// Agent identifier — maps to the folder `coral-agents/<agent>/agent.py`
    /// (dashes are converted to underscores). E.g. `"helius-monitor"`.
    pub agent: String,
    pub wallet: String,
    pub amount_sol: f64,
    #[serde(default)]
    pub rpc_url: String,
    #[serde(default)]
    pub ws_url: String,
    #[serde(default)]
    pub helius_api_key: String,
    /// `"standalone"` or `"coral"`.
    pub mode: String,
    #[serde(default)]
    pub coral_url: String,
}

/// Holds the currently-running child process, if any.
#[derive(Default)]
pub struct PythonAgentState {
    child: Mutex<Option<Child>>,
}

/// Locate `coral-agents/<agent>/agent.py`. Honours `PAY_AGENTS_DIR`, otherwise
/// probes paths relative to the working directory (dev) and the executable.
fn resolve_script(agent: &str) -> Result<PathBuf, String> {
    let folder = agent.replace('-', "_");
    let rel = PathBuf::from(&folder).join("agent.py");

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("PAY_AGENTS_DIR") {
        roots.push(PathBuf::from(dir));
    }
    // `cargo tauri dev` runs from src-tauri/ → ../coral-agents
    roots.push(PathBuf::from("../coral-agents"));
    roots.push(PathBuf::from("coral-agents"));
    roots.push(PathBuf::from("agent_demo/coral-agents"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("coral-agents"));
        }
    }

    for root in &roots {
        let candidate = root.join(&rel);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not find agent script for '{agent}' (looked for {}). \
         Set PAY_AGENTS_DIR to the coral-agents directory.",
        rel.display()
    ))
}

/// A runnable interpreter: program plus any leading args (e.g. `py -3`).
type Interp = (String, Vec<String>);

/// Probe `<prog> <prefix...> -c "<code>"` and report whether it exits 0.
fn probe(prog: &str, prefix: &[String], code: &str) -> bool {
    Command::new(prog)
        .args(prefix)
        .arg("-c")
        .arg(code)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Pick a working Python. Honours `$PYTHON`, otherwise tries `py -3`,
/// `python3`, `python`. Prefers an interpreter that already has `websockets`;
/// falls back to any that merely runs (so the agent can emit its own clear
/// missing-dependency error). This dodges broken shim pythons (e.g. MSYS2).
fn resolve_python() -> Result<Interp, String> {
    let mut candidates: Vec<Interp> = Vec::new();
    if let Ok(p) = std::env::var("PYTHON") {
        if !p.trim().is_empty() {
            candidates.push((p, vec![]));
        }
    }
    candidates.push(("py".into(), vec!["-3".into()]));
    candidates.push(("python3".into(), vec![]));
    candidates.push(("python".into(), vec![]));

    // First pass: an interpreter that actually has the websockets dependency.
    for (prog, prefix) in &candidates {
        if probe(prog, prefix, "import websockets") {
            return Ok((prog.clone(), prefix.clone()));
        }
    }
    // Second pass: any interpreter that runs at all.
    for (prog, prefix) in &candidates {
        if probe(prog, prefix, "import sys") {
            return Ok((prog.clone(), prefix.clone()));
        }
    }
    Err("no working Python interpreter found (tried $PYTHON, py -3, python3, \
         python). Install Python 3 and `pip install websockets`, or set the \
         PYTHON env var."
        .into())
}

#[tauri::command]
pub fn python_agent_start(
    app: AppHandle,
    state: State<PythonAgentState>,
    config: PythonAgentConfig,
) -> Result<bool, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("A Python agent is already running — stop it first.".into());
    }
    if config.wallet.trim().is_empty() {
        return Err("wallet is required".into());
    }

    let script = resolve_script(&config.agent)?;
    let (prog, prefix) = resolve_python()?;

    let mut cmd = Command::new(&prog);
    cmd.args(&prefix)
        .arg(&script)
        .arg("--wallet")
        .arg(&config.wallet)
        .arg("--amount")
        .arg(config.amount_sol.to_string())
        .arg("--mode")
        .arg(&config.mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if !config.rpc_url.trim().is_empty() {
        cmd.arg("--rpc-url").arg(&config.rpc_url);
    }
    if !config.ws_url.trim().is_empty() {
        cmd.arg("--ws-url").arg(&config.ws_url);
    }
    if !config.coral_url.trim().is_empty() {
        cmd.arg("--coral-url").arg(&config.coral_url);
    }
    if !config.helius_api_key.trim().is_empty() {
        cmd.env("HELIUS_API_KEY", &config.helius_api_key);
        cmd.arg("--helius-api-key").arg(&config.helius_api_key);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn '{prog}': {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let app = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let payload = serde_json::from_str::<serde_json::Value>(&line)
                    .unwrap_or_else(|_| serde_json::json!({ "type": "log", "message": line }));
                let _ = app.emit("python-agent-event", payload);
            }
            let _ = app.emit("python-agent-event", serde_json::json!({ "type": "exited" }));
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                let _ = app.emit(
                    "python-agent-event",
                    serde_json::json!({ "type": "stderr", "message": line }),
                );
            }
        });
    }

    *guard = Some(child);
    Ok(true)
}

#[tauri::command]
pub fn python_agent_stop(state: State<PythonAgentState>) -> Result<bool, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Returns `true` if a child is alive; reaps it and returns `false` if it exited.
#[tauri::command]
pub fn python_agent_status(state: State<PythonAgentState>) -> Result<bool, String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    match guard.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                Ok(false)
            }
            Ok(None) => Ok(true),
            Err(e) => Err(e.to_string()),
        },
        None => Ok(false),
    }
}
