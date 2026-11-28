use std::process::Command;

pub fn available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn run(input: &str, output: &str) -> Result<String, String> {
    let out = Command::new("ffmpeg")
        .args(["-y", "-i", input, output])
        .output()
        .map_err(|err| format!("ffmpeg: {err}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: String = stderr.chars().rev().take(400).collect::<String>().chars().rev().collect();
        return Err(format!("ffmpeg failed: {tail}"));
    }
    Ok(output.to_string())
}
