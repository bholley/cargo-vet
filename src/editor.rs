//! Helper utilities for opening files in the editor.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::str;

use tempfile::NamedTempFile;
use tracing::warn;

use crate::VetError;

#[cfg(windows)]
fn git_sh_path() -> Option<PathBuf> {
    // Locate the `git` binary using the windows `where` command.
    let output = Command::new("where").arg("git").output().ok()?;
    if !output.status.success() {
        return None;
    }

    // The git binary path should be either in the `cmd` or `bin` subdirectory
    // of the git-for-windows install path, while the `sh.exe` binary is located
    // in the `bin` subdirectory.
    Path::new(str::from_utf8(&output.stdout).ok()?.trim())
        .canonicalize()
        .ok()?
        .parent()?
        .parent()?
        .join(r"bin\sh.exe")
        .canonicalize()
        .ok()
}

#[cfg(not(windows))]
fn git_sh_path() -> Option<PathBuf> {
    Some("/bin/sh".into())
}

/// Read the git configuration to determine the value for GIT_EDITOR.
fn git_editor() -> Option<String> {
    // Testing environment variable to force using the fallback editor instead
    // of GIT_EDITOR.
    if std::env::var("CARGO_VET_USE_FALLBACK_EDITOR").unwrap_or_default() == "1" {
        return None;
    }

    let output = Command::new("git")
        .arg("var")
        .arg("GIT_EDITOR")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(str::from_utf8(&output.stdout).ok()?.trim().to_owned())
}

#[cfg(windows)]
const FALLBACK_EDITOR: &str = "notepad.exe";

// NOTE: This is probably not as reliably available as `vi`, but is easier to
// quit from for users who aren't familiar with vi.
#[cfg(not(windows))]
const FALLBACK_EDITOR: &str = "nano";

/// Get a Command which can be used to invoke the user's EDITOR to edit a
/// document when passed an argument. This will try to use the user's configured
/// GIT_EDITOR when possible.
pub fn editor_command() -> Command {
    // Try to use the user's configured editor if we're able to locate their git
    // install. If this fails, invoke the default editor instead.
    //
    // XXX: If we end up with commands which invoke the editor many times, it
    // may eventually be worth adding some form of caching here.
    match (git_sh_path(), git_editor()) {
        (Some(git_sh), Some(git_editor)) => {
            let mut cmd = Command::new(git_sh);
            cmd.arg("-c")
                .arg(format!("{} \"$@\"", git_editor))
                .arg(git_editor);
            return cmd;
        }
        (_, None) => {
            warn!("Unable to determine user's GIT_EDITOR");
        }
        (None, Some(_)) => {
            warn!("Unable to locate user's git install to invoke GIT_EDITOR");
        }
    }
    warn!("Falling back to running '{}' directly", FALLBACK_EDITOR);
    Command::new(FALLBACK_EDITOR)
}

/// Run the default editor configured through git (GIT_EDITOR) and use it to
/// edit the given file path.
pub fn run_editor(path: &Path) -> io::Result<ExitStatus> {
    editor_command().arg(path).status()
}

// On windows some editors (notably notepad pre-windows 11) don't handle
// unix line endings very well, so make sure to give them windows line
// endings.
#[cfg(windows)]
const LINE_ENDING: &str = "\r\n";

#[cfg(not(windows))]
const LINE_ENDING: &str = "\n";

pub struct Editor<'a> {
    tempfile: NamedTempFile,
    comment_char: char,
    run_editor: Box<dyn FnOnce(&Path) -> io::Result<bool> + 'a>,
}

impl<'a> Editor<'a> {
    /// Create a new editor for a temporary file.
    pub fn new(name: &str) -> io::Result<Self> {
        let tempfile = tempfile::Builder::new()
            .prefix(&format!("{}.", name))
            .tempfile()?;
        Ok(Editor {
            tempfile,
            comment_char: '#',
            run_editor: Box::new(|p| run_editor(p).map(|s| s.success())),
        })
    }

    /// Set the character to be used for comments.
    pub fn set_comment_char(&mut self, comment_char: char) {
        self.comment_char = comment_char;
    }

    /// Attempt to pick a comment character which does not appear at the start
    /// of any line in the given text.
    pub fn select_comment_char(&mut self, text: &str) {
        let mut comment_chars = ['#', ';', '@', '!', '$', '%', '^', '&', '|', ':', '"', ';'];
        for line in text.lines() {
            for cc in &mut comment_chars {
                if line.starts_with(*cc) {
                    *cc = '\0';
                    break;
                }
            }
        }
        self.set_comment_char(
            comment_chars
                .into_iter()
                .find(|cc| *cc != '\0')
                .expect("couldn't find a viable comment character"),
        );
    }

    #[cfg(test)]
    /// Test-only method to mock out the actual invocation of the editor.
    pub fn set_run_editor(&mut self, run_editor: impl FnOnce(&Path) -> io::Result<bool> + 'a) {
        self.run_editor = Box::new(run_editor);
    }

    /// Add comment lines to the editor. Any newlines in the input will be
    /// normalized to the current platform, and a comment character will be
    /// added.
    pub fn add_comments(&mut self, text: &str) -> io::Result<()> {
        let text = text.trim();
        if text.is_empty() {
            write!(self.tempfile, "{}{}", self.comment_char, LINE_ENDING)?;
        }
        for line in text.lines() {
            if line.is_empty() {
                write!(self.tempfile, "{}{}", self.comment_char, LINE_ENDING)?;
            } else {
                write!(
                    self.tempfile,
                    "{} {}{}",
                    self.comment_char, line, LINE_ENDING
                )?;
            }
        }
        Ok(())
    }

    /// Add non-comment lines to the editor. These lines must not start with
    /// comment_character.
    pub fn add_text(&mut self, text: &str) -> io::Result<()> {
        let text = text.trim();
        if text.is_empty() {
            write!(self.tempfile, "{}", LINE_ENDING)?;
        }
        for line in text.lines() {
            assert!(
                !line.starts_with(self.comment_char),
                "non-comment lines cannot start with a '{}' comment character",
                self.comment_char
            );
            write!(self.tempfile, "{}{}", line, LINE_ENDING)?;
        }
        Ok(())
    }

    /// Run the editor, collecting and filtering the resulting file, and
    /// returning it as a string.
    pub fn edit(self) -> Result<String, VetError> {
        // Close our handle on the file to allow other programs like the editor
        // to modify it on Windows.
        let path = self.tempfile.into_temp_path();
        (self.run_editor)(&path)?;

        // Read in the result, filtering lines, and restoring unix line endings.
        // This is roughly based on git's logic for cleaning up commit message
        // files.
        let mut lines: Vec<String> = Vec::new();
        for line in BufReader::new(File::open(&path)?).lines() {
            let line = line?;
            // Ignore lines starting with a comment character.
            if line.starts_with(self.comment_char) {
                continue;
            }
            // Trim any trailing whitespace from each line, but leave leading
            // whitespace untouched to avoid breaking formatted text.
            let line = line.trim_end();
            // Don't record 2 consecutive empty lines or empty lines at the
            // start of the file.
            if line.is_empty() && lines.last().map_or(true, |l| l.is_empty()) {
                continue;
            }
            lines.push(line.to_owned());
        }

        // Ensure there's a trailing newline for non-empty files.
        match lines.last() {
            None => return Ok(String::new()),
            Some(line) if !line.is_empty() => lines.push(String::new()),
            _ => {}
        }

        Ok(lines.join("\n"))
    }
}
