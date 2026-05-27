/// Score how well `needle` matches a single `haystack` string.
///
/// Returns 0 when there is no match at all.  Higher is better.
///
/// | Match kind                         | Score |
/// |------------------------------------|-------|
/// | Exact (case-insensitive)           | 1000  |
/// | Prefix                             |  800  |
/// | Substring                          |  100  |
/// | Space-stripped exact               |  600  |
/// | Space-stripped prefix              |  400  |
/// | Space-stripped subsequence         |   30  |
pub fn name_match_score(needle: &str, haystack: &str) -> i32 {
  if needle.is_empty() || haystack.is_empty() {
    return 0;
  }
  let n = needle.to_lowercase();
  let h = haystack.to_lowercase();

  // exact
  if h == n {
    return 1000;
  }
  // prefix
  if h.starts_with(&n) {
    return 800;
  }
  // substring
  if h.contains(&n) {
    return 100;
  }

  // space-stripped variants ("vscode" vs "visual studio code")
  let n_ns: String = n.chars().filter(|c| !c.is_whitespace()).collect();
  let h_ns: String = h.chars().filter(|c| !c.is_whitespace()).collect();
  if h_ns == n_ns {
    return 600;
  }
  if h_ns.starts_with(&n_ns) {
    return 400;
  }
  // subsequence: all chars of needle appear in haystack in order
  // e.g. "vscode" is a subsequence of "visualstudiocode"
  // Require needle >= 20% of haystack length to avoid false positives
  // (e.g. "vscode" matching "devtools-localhost:5274/..." by coincidence)
  if n_ns.len() * 5 >= h_ns.len() && is_subsequence(&n_ns, &h_ns) {
    return 30;
  }

  0
}

/// Check if `needle` is a subsequence of `haystack` (all chars appear in order).
fn is_subsequence(needle: &str, haystack: &str) -> bool {
  let mut hi = haystack.chars();
  for nc in needle.chars() {
    loop {
      match hi.next() {
        Some(hc) if hc == nc => break,
        Some(_) => continue,
        None => return false,
      }
    }
  }
  true
}

/// Best string-match score across multiple candidate fields.
pub fn best_name_score(needle: &str, candidates: &[&str]) -> i32 {
  candidates.iter().map(|c| name_match_score(needle, c)).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
  use super::*;

  // ── Unit: name_match_score ───────────────────────────────────────────

  #[test]
  fn exact_match() {
    assert_eq!(name_match_score("code", "Code"), 1000);
    assert_eq!(name_match_score("Code.exe", "code.exe"), 1000);
  }

  #[test]
  fn prefix_match() {
    assert_eq!(name_match_score("cod", "Code.exe"), 800);
  }

  #[test]
  fn substring_match() {
    assert_eq!(name_match_score("ode", "Code.exe"), 100);
  }

  #[test]
  fn space_stripped_exact() {
    assert_eq!(name_match_score("vscode", "VS Code"), 600);
    assert_eq!(name_match_score("visual studio code", "VisualStudioCode"), 600);
  }

  #[test]
  fn subsequence_match() {
    let score = name_match_score("vscode", "Visual Studio Code");
    assert_eq!(score, 30);
  }

  #[test]
  fn subsequence_no_match() {
    assert_eq!(name_match_score("xyz", "abc"), 0);
  }

  #[test]
  fn no_match() {
    assert_eq!(name_match_score("firefox", "Code.exe"), 0);
  }

  // ── Integration: best_name_score against real process names ──────────
  // Simulates resolve_app_pid(exe, stem) and find_windows_by_process(proc, title)
  // using actual process names from a Windows machine.

  #[test]
  fn vscode_via_window_title() {
    // resolve_app_pid path: exe="Code.exe", stem="Code" → no match for "vscode"
    assert_eq!(best_name_score("vscode", &["Code.exe", "Code"]), 0);
    // find_windows_by_process path: title has "Visual Studio Code" → subsequence
    let s = best_name_score("vscode", &["Code", "tokimo - Visual Studio Code"]);
    assert_eq!(s, 30, "vscode should match via subsequence on title");
  }

  #[test]
  fn chrome_exact() {
    assert_eq!(best_name_score("chrome", &["chrome.exe", "chrome"]), 1000);
    assert_eq!(best_name_score("Chrome", &["chrome.exe", "chrome"]), 1000);
  }

  #[test]
  fn docker_prefix() {
    // "Docker Desktop.exe" stem = "Docker Desktop"
    assert_eq!(best_name_score("docker", &["Docker Desktop.exe", "Docker Desktop"]), 800);
    // com.docker.backend → substring
    assert_eq!(best_name_score("docker", &["com.docker.backend.exe", "com.docker.backend"]), 100);
  }

  #[test]
  fn explorer_exact() {
    assert_eq!(best_name_score("explorer", &["explorer.exe", "explorer"]), 1000);
  }

  #[test]
  fn qq_exact_not_prefix_of_qqmusic() {
    // "QQ" exact match on stem "QQ" → 1000 (wins over QQMusic's prefix 800)
    assert_eq!(best_name_score("qq", &["QQ.exe", "QQ"]), 1000);
    // QQMusic: "qq" is prefix → 800
    assert_eq!(best_name_score("qq", &["QQMusic.exe", "QQMusic"]), 800);
  }

  #[test]
  fn qqmusic_exact() {
    assert_eq!(best_name_score("qqmusic", &["QQMusic.exe", "QQMusic"]), 1000);
  }

  #[test]
  fn steam_exact() {
    assert_eq!(best_name_score("steam", &["steam.exe", "steam"]), 1000);
  }

  #[test]
  fn terminal_substring() {
    // "terminal" is substring of "WindowsTerminal"
    assert_eq!(best_name_score("terminal", &["WindowsTerminal.exe", "WindowsTerminal"]), 100);
  }

  #[test]
  fn nvidia_prefix() {
    assert_eq!(best_name_score("nvidia", &["NVIDIA Overlay.exe", "NVIDIA Overlay"]), 800);
  }

  #[test]
  fn weixin_exact() {
    assert_eq!(best_name_score("weixin", &["Weixin.exe", "Weixin"]), 1000);
  }

  #[test]
  fn wechat_no_match_on_weixin() {
    // "wechat" ≠ "weixin" — different app
    assert_eq!(best_name_score("wechat", &["Weixin.exe", "Weixin"]), 0);
    // but WeChatAppEx matches
    assert_eq!(best_name_score("wechat", &["WeChatAppEx.exe", "WeChatAppEx"]), 800);
  }

  #[test]
  fn taskmgr_prefix() {
    assert_eq!(best_name_score("task", &["Taskmgr.exe", "Taskmgr"]), 800);
  }

  #[test]
  fn codex_prefix() {
    // "code" is prefix of "Codex" → 800
    assert_eq!(best_name_score("code", &["Codex.exe", "Codex"]), 800);
    // "codex" exact → 1000
    assert_eq!(best_name_score("codex", &["Codex.exe", "Codex"]), 1000);
  }

  #[test]
  fn notepad_no_match() {
    assert_eq!(best_name_score("notepad", &["Code.exe", "Code"]), 0);
    assert_eq!(best_name_score("notepad", &["explorer.exe", "explorer"]), 0);
  }

  #[test]
  fn partial_typo_no_match() {
    // "chrom" prefix → 800, but "chrmo" has wrong char order → subsequence might match
    assert_eq!(best_name_score("chrmo", &["chrome.exe", "chrome"]), 0);
  }

  #[test]
  fn edge_empty_needle() {
    assert_eq!(best_name_score("", &["Code.exe"]), 0);
  }

  #[test]
  fn edge_empty_haystack() {
    assert_eq!(best_name_score("code", &["", "Code"]), 1000);
  }

  // ── Scoring priority: exact > prefix > substring > subsequence ───────

  #[test]
  fn scoring_priority() {
    // Simulates resolve_app_pid which passes (exe, stem)
    let resolve = |needle: &str| best_name_score(needle, &["Code.exe", "Code"]);
    assert_eq!(resolve("code"), 1000, "exact on stem");
    assert_eq!(resolve("cod"), 800, "prefix on exe");
    assert_eq!(resolve("ode"), 100, "substring on exe");
    assert_eq!(resolve("vscode"), 0, "no match on exe/stem");

    // Simulates find_windows_by_process which passes (process_name, title)
    let find_win = |needle: &str| best_name_score(needle, &["Code", "tokimo - Visual Studio Code"]);
    assert_eq!(find_win("code"), 1000, "exact on proc name");
    assert_eq!(find_win("visual"), 100, "substring on title");
    assert_eq!(find_win("vscode"), 30, "subsequence on title");
    assert_eq!(find_win("studio"), 100, "substring on title");
  }

  #[test]
  fn subsequence_rejects_long_haystack_false_positive() {
    // DevTools window from chrome should NOT match "vscode"
    let devtools_title = "DevTools - localhost:5274/game/sword1/share/Rx-2eZZ3";
    let s = best_name_score("vscode", &["chrome.exe", devtools_title]);
    assert_eq!(s, 0, "DevTools window should not match vscode, got {s}");
  }
}
