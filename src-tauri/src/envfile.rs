//! Reading a `.env` file to import its keys, and rewriting the imported lines
//! as `handoff:` pointers. Every other byte of the file stays as it was:
//! comments, blank lines, other variables, quotes and line endings.

use zeroize::Zeroizing;

const PREFIX: &str = "handoff:";

/// One `VAR=value` line worth importing: not empty, not already a pointer.
pub struct Entry {
    pub var: String,
    pub value: Zeroizing<String>,
    /// Which line it's on (0-based), for the rewrite.
    line: usize,
}

/// The variables in `text` that hold a value, in file order. A variable set
/// twice counts once, as its last line (what loaders use).
pub fn entries(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for (line, raw) in text.split('\n').enumerate() {
        let Some((var, value)) = parse(raw) else { continue };
        if value.is_empty() || value.starts_with(PREFIX) {
            continue;
        }
        out.retain(|e| e.var != var);
        out.push(Entry { var: var.to_string(), value: Zeroizing::new(value.to_string()), line });
    }
    out
}

/// `text` with each `(entry, key name)` line replaced by a pointer to that
/// key. The variable keeps its name and any `export `; the line keeps its
/// `\r` if it had one. A trailing comment on the line goes with the value.
pub fn with_pointers(text: &str, done: &[(&Entry, &str)]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(text.len()));
    for (i, raw) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match done.iter().find(|(e, _)| e.line == i) {
            Some((entry, name)) => {
                let cr = if raw.ends_with('\r') { "\r" } else { "" };
                let export = if raw.trim_start().starts_with("export ") { "export " } else { "" };
                out.push_str(&format!("{export}{}={}{cr}", entry.var, pointer(name)));
            }
            None => out.push_str(raw),
        }
    }
    out
}

/// `handoff:Name`, quoted when the name has characters a .env reader might split on.
pub fn pointer(name: &str) -> String {
    let p = format!("{PREFIX}{name}");
    if p.chars().all(|c| c.is_ascii_alphanumeric() || "_-.:".contains(c)) {
        p
    } else {
        format!("\"{}\"", p.replace('"', ""))
    }
}

/// Whether a variable looks like it holds a key, by its name and value:
/// what the import preselects. A guess you can change, nothing more.
pub fn looks_secret(var: &str, value: &str) -> bool {
    let name = var.to_ascii_uppercase();
    let named = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PWD", "AUTH", "CREDENTIAL", "PRIVATE", "DSN", "WEBHOOK"]
        .iter()
        .any(|w| name.contains(w));
    let plain = (value.starts_with("http://") || value.starts_with("https://")) && !value.contains('@');
    named && !plain && value.len() >= 8
}

/// `VAR` and its value from one line, read the way the loaders read it.
fn parse(raw: &str) -> Option<(&str, &str)> {
    let mut line = raw.trim();
    if let Some(rest) = line.strip_prefix("export ") {
        line = rest.trim_start();
    }
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (var, value) = line.split_once('=')?;
    let (var, mut value) = (var.trim(), value.trim());
    if var.is_empty() || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let b = value.as_bytes();
    if b.len() >= 2 && b[0] == b[b.len() - 1] && (b[0] == b'"' || b[0] == b'\'') {
        value = &value[1..value.len() - 1];
    } else if let Some((before, _)) = value.split_once(" #") {
        value = before.trim_end();
    }
    Some((var, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# tp-dialpad\r\nDIALPAD_API_KEY=dp_synthetic_0123456789abcdef\r\nFORMSITE_API_BASE=https://fs1.formsite.com/api/v2\r\n\r\nexport ANTHROPIC_API_KEY=\"sk-ant-synthetic-abcdef0123456789\"\r\nJEV_API_KEY=handoff:Jev\r\nPIPEDRIVE_API_TOKEN=pd_synthetic_9876543210 # rotated in May\r\nEMPTY=\r\n";

    #[test]
    fn finds_values_and_skips_pointers_comments_and_empties() {
        let found: Vec<_> = entries(FILE).into_iter().map(|e| (e.var, e.value.to_string())).collect();
        assert_eq!(
            found,
            [
                ("DIALPAD_API_KEY".to_string(), "dp_synthetic_0123456789abcdef".to_string()),
                ("FORMSITE_API_BASE".into(), "https://fs1.formsite.com/api/v2".into()),
                ("ANTHROPIC_API_KEY".into(), "sk-ant-synthetic-abcdef0123456789".into()),
                ("PIPEDRIVE_API_TOKEN".into(), "pd_synthetic_9876543210".into()),
            ]
        );
        let guess: Vec<_> = entries(FILE).iter().map(|e| looks_secret(&e.var, &e.value)).collect();
        assert_eq!(guess, [true, false, true, true]);
    }

    #[test]
    fn rewrites_only_the_imported_lines() {
        let found = entries(FILE);
        let done = [(&found[0], "DIALPAD_API_KEY"), (&found[2], "Anthropic main")];
        let out = with_pointers(FILE, &done);
        assert_eq!(
            out.as_str(),
            "# tp-dialpad\r\nDIALPAD_API_KEY=handoff:DIALPAD_API_KEY\r\nFORMSITE_API_BASE=https://fs1.formsite.com/api/v2\r\n\r\nexport ANTHROPIC_API_KEY=\"handoff:Anthropic main\"\r\nJEV_API_KEY=handoff:Jev\r\nPIPEDRIVE_API_TOKEN=pd_synthetic_9876543210 # rotated in May\r\nEMPTY=\r\n"
        );
        // Read back, the rewritten lines are pointers and nothing else moved.
        let again: Vec<_> = entries(&out).into_iter().map(|e| e.var).collect();
        assert_eq!(again, ["FORMSITE_API_BASE", "PIPEDRIVE_API_TOKEN"]);
    }
}
