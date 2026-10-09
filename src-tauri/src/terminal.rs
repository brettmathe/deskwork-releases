//! One embedded terminal running the Claude CLI in the workspace root.
//!
//! Claude is launched through the user's login + interactive shell so PATH
//! comes from their dotfiles (nvm, Homebrew, ~/.local/bin): apps opened from
//! Finder don't inherit a shell's environment, and a bare `claude` would not be
//! found.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::Channel;

use crate::repo::{require_root, RepoState};

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TerminalEvent {
    Output { data: String },
    /// Claude (and its shell) ended; the UI offers a restart.
    Exit,
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[derive(Default)]
pub struct TerminalState(Mutex<Option<Session>>);

impl TerminalState {
    /// Kills the running session, if any (app exit, restart).
    pub fn stop(&self) {
        if let Ok(mut s) = self.0.lock() {
            s.take();
        }
    }
}

/// Variables a parent Claude process sets for its own children (e.g. when
/// Deskwork is started from a terminal inside the Claude desktop app). Passed on,
/// they make the embedded Claude behave like part of that host. Anything the
/// user sets in their dotfiles is re-applied when the shell starts.
pub fn is_inherited_claude_var(name: &str) -> bool {
    name.starts_with("CLAUDE_CODE_")
        || name.starts_with("CLAUDE_AGENT_SDK_")
        || matches!(name, "CLAUDECODE" | "ANTHROPIC_BASE_URL")
}

/// `$SHELL -l -i -c 'exec claude'`: login + interactive so both .zprofile and
/// .zshrc run (nvm usually lives in .zshrc); `exec` so Claude exiting ends the
/// session instead of leaving a shell behind.
pub fn claude_command(root: &Path, shell: Option<String>) -> CommandBuilder {
    let shell = shell.filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
    let mut cmd = CommandBuilder::new(shell);
    for (name, _) in std::env::vars_os() {
        if is_inherited_claude_var(&name.to_string_lossy()) {
            cmd.env_remove(name);
        }
    }
    cmd.args(["-l", "-i", "-c", "exec claude"]);
    cmd.cwd(root);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    if std::env::var_os("LANG").is_none() {
        cmd.env("LANG", "en_US.UTF-8");
    }
    cmd
}

/// Splits `buf` into the longest valid UTF-8 prefix and the bytes of a
/// multi-byte character cut off at the end of a read, which are kept for the
/// next read. Invalid bytes mid-stream are replaced rather than stalling output.
pub fn take_utf8(buf: &mut Vec<u8>) -> String {
    match std::str::from_utf8(buf) {
        Ok(s) => {
            let out = s.to_string();
            buf.clear();
            out
        }
        Err(e) if e.error_len().is_none() => {
            let valid = e.valid_up_to();
            let out = String::from_utf8_lossy(&buf[..valid]).into_owned();
            buf.drain(..valid);
            out
        }
        Err(_) => {
            let out = String::from_utf8_lossy(buf).into_owned();
            buf.clear();
            out
        }
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 }
}

// ---------- commands ----------

/// Starts Claude in the workspace root, replacing any running session.
/// Output and the exit status stream back over `on_event`.
#[tauri::command]
pub fn terminal_start(
    state: tauri::State<TerminalState>,
    repo: tauri::State<RepoState>,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> Result<(), String> {
    let root = require_root(&repo)?;
    state.stop();

    let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let child = pair
        .slave
        .spawn_command(claude_command(&root, std::env::var("SHELL").ok()))
        .map_err(|e| format!("failed to start Claude: {e}"))?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        let mut pending = Vec::new();
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&chunk[..n]);
                    let data = take_utf8(&mut pending);
                    if !data.is_empty() && on_event.send(TerminalEvent::Output { data }).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = on_event.send(TerminalEvent::Exit);
    });

    *state.0.lock().map_err(|e| e.to_string())? = Some(Session { master: pair.master, writer, child });
    Ok(())
}

#[tauri::command]
pub fn terminal_write(state: tauri::State<TerminalState>, data: String) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let s = guard.as_mut().ok_or("terminal is not running")?;
    s.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    s.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_resize(state: tauri::State<TerminalState>, cols: u16, rows: u16) -> Result<(), String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(s) => s.master.resize(size(cols, rows)).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn terminal_stop(state: tauri::State<TerminalState>) {
    state.stop();
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_utf8_keeps_a_split_character_for_the_next_read() {
        let mut buf = "héllo".as_bytes().to_vec();
        let tail = buf.split_off(2); // cut inside "é"
        assert_eq!(take_utf8(&mut buf), "h");
        assert_eq!(buf.len(), 1);
        buf.extend_from_slice(&tail);
        assert_eq!(take_utf8(&mut buf), "éllo");
        assert!(buf.is_empty());
    }

    #[test]
    fn take_utf8_replaces_invalid_bytes_instead_of_stalling() {
        let mut buf = vec![b'a', 0xff, b'b'];
        assert_eq!(take_utf8(&mut buf), "a\u{fffd}b");
        assert!(buf.is_empty());
    }

    #[test]
    fn claude_runs_through_a_login_interactive_shell_in_the_root() {
        let cmd = claude_command(Path::new("/tmp/ws"), Some("/bin/bash".into()));
        let argv: Vec<String> =
            cmd.get_argv().iter().map(|a| a.to_string_lossy().to_string()).collect();
        assert_eq!(argv, ["/bin/bash", "-l", "-i", "-c", "exec claude"]);
        assert_eq!(cmd.get_cwd().map(|c| c.to_string_lossy().to_string()), Some("/tmp/ws".into()));
        let fallback = claude_command(Path::new("/tmp/ws"), None);
        assert_eq!(fallback.get_argv()[0].to_string_lossy(), "/bin/zsh");
    }

    #[test]
    fn inherited_claude_host_vars_are_stripped() {
        assert!(is_inherited_claude_var("CLAUDE_CODE_ENTRYPOINT"));
        assert!(is_inherited_claude_var("CLAUDE_AGENT_SDK_VERSION"));
        assert!(is_inherited_claude_var("CLAUDECODE"));
        assert!(is_inherited_claude_var("ANTHROPIC_BASE_URL"));
        assert!(!is_inherited_claude_var("CLAUDE_CONFIG_DIR"));
        assert!(!is_inherited_claude_var("PATH"));

        std::env::set_var("CLAUDE_CODE_DESKWORK_TEST", "1");
        let cmd = claude_command(Path::new("/tmp/ws"), None);
        std::env::remove_var("CLAUDE_CODE_DESKWORK_TEST");
        assert!(cmd.get_env("CLAUDE_CODE_DESKWORK_TEST").is_none());
        assert!(cmd.get_env("TERM").is_some());
    }
}
