//! Structured UI-element query for Windows, mirroring macOS `elements::query_*`.
//!
//! Uses the UIAutomation RawViewWalker to traverse the tree under the requested
//! scope (window / app PID / foreground) and applies the `ElementQuery` filter
//! semantics that the cross-platform CLI expects:
//!   * `role`   — abstract control-type (Button, Edit, Text, …); supports
//!                comma/pipe lists, and a Chinese/English LocalizedControlType
//!                fallback for apps whose UIA tree exposes Custom controls
//!                (Electron, Qt) with only a localized role description.
//!   * `text`   — substring (case-insensitive) match against Name /
//!                AutomationId / Value / HelpText / ClassName / LocalizedType.
//!                `text_exact = true` requires a literal full-string match.
//!   * `index`  — pick the Nth (0-based) result.
//!   * `max_depth` — cap walker depth.
//!   * `include_hidden` — by default off-screen / zero-rect / disabled-offscreen
//!                elements are dropped.
//!   * `no_hit_test` — accepted for API symmetry; Windows UIA tree is usually
//!                complete enough that we don't need a hit-test scan.

use crate::error::Result;
use crate::platform::Element;
use crate::platform::windows::system_info::ensure_com_initialized;
use crate::platform::windows::ui_object::WindowsElement;
use crate::platform::windows::wnd;
use crate::types::{ElementQuery, ElementScope, WindowHandle};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::*;
use windows::Win32::UI::Accessibility::*;
use windows::core::BOOL;

// ────────────────────────────────────────────────────────────────────────────
// Role mapping
// ────────────────────────────────────────────────────────────────────────────

/// Map an abstract role name (the cross-platform CLI vocabulary) to the
/// set of concrete UIA control-type IDs that should be considered a match.
fn abstract_role_to_ids(role: &str) -> Vec<UIA_CONTROLTYPE_ID> {
  match role.to_lowercase().as_str() {
    "button" | "splitbutton" => vec![UIA_ButtonControlTypeId, UIA_SplitButtonControlTypeId],
    "edit" | "textfield" | "textbox" | "input" => vec![UIA_EditControlTypeId],
    "text" | "label" | "statictext" => vec![UIA_TextControlTypeId],
    "checkbox" => vec![UIA_CheckBoxControlTypeId],
    "radiobutton" | "radio" => vec![UIA_RadioButtonControlTypeId],
    "combobox" | "popupbutton" => vec![UIA_ComboBoxControlTypeId],
    "list" => vec![UIA_ListControlTypeId, UIA_DataGridControlTypeId, UIA_TableControlTypeId],
    "listitem" | "row" | "dataitem" => vec![UIA_ListItemControlTypeId, UIA_DataItemControlTypeId],
    "menuitem" => vec![UIA_MenuItemControlTypeId],
    "menu" => vec![UIA_MenuControlTypeId],
    "menubar" => vec![UIA_MenuBarControlTypeId],
    "slider" => vec![UIA_SliderControlTypeId],
    "progressbar" => vec![UIA_ProgressBarControlTypeId],
    "image" => vec![UIA_ImageControlTypeId],
    "link" | "hyperlink" => vec![UIA_HyperlinkControlTypeId],
    "tab" => vec![UIA_TabControlTypeId],
    "tabitem" => vec![UIA_TabItemControlTypeId],
    "group" => vec![UIA_GroupControlTypeId],
    "window" => vec![UIA_WindowControlTypeId],
    "pane" => vec![UIA_PaneControlTypeId],
    "tree" => vec![UIA_TreeControlTypeId],
    "treeitem" => vec![UIA_TreeItemControlTypeId],
    "toolbar" => vec![UIA_ToolBarControlTypeId],
    "tooltip" => vec![UIA_ToolTipControlTypeId],
    "document" => vec![UIA_DocumentControlTypeId],
    "table" => vec![UIA_TableControlTypeId],
    "header" => vec![UIA_HeaderControlTypeId],
    "headeritem" => vec![UIA_HeaderItemControlTypeId],
    "separator" => vec![UIA_SeparatorControlTypeId],
    "appbar" => vec![UIA_AppBarControlTypeId],
    "titlebar" => vec![UIA_TitleBarControlTypeId],
    "statusbar" => vec![UIA_StatusBarControlTypeId],
    "calendar" => vec![UIA_CalendarControlTypeId],
    "spinner" => vec![UIA_SpinnerControlTypeId],
    "scrollbar" => vec![UIA_ScrollBarControlTypeId],
    "thumb" => vec![UIA_ThumbControlTypeId],
    _ => vec![],
  }
}

/// Match an abstract role against the element's LocalizedControlType
/// (mainly to support Chinese/English custom controls in Electron/Qt apps).
fn matches_localized_role(localized: &str, query_role: &str) -> bool {
  let desc = localized.to_lowercase();
  let candidates: &[&str] = match query_role.to_lowercase().as_str() {
    "button" => &["button", "按钮", "ボタン", "버튼"],
    "edit" | "textfield" | "textbox" | "input" => &[
      "edit",
      "textbox",
      "text field",
      "text box",
      "search box",
      "search field",
      "编辑",
      "文本框",
      "输入框",
      "搜索框",
    ],
    "text" | "label" | "statictext" => &["static text", "label", "静态文本", "标签"],
    "checkbox" => &["check", "checkbox", "复选"],
    "radiobutton" | "radio" => &["radio", "单选"],
    "combobox" | "popupbutton" => &["combo", "下拉", "弹出"],
    "list" => &["list", "table", "列表", "表格"],
    "listitem" | "row" => &["row", "item", "行", "项"],
    "menuitem" => &["menu item", "menuitem", "菜单"],
    "image" => &["image", "图像", "图片"],
    "link" | "hyperlink" => &["link", "链接"],
    "tab" => &["tab", "标签页"],
    "group" => &["group", "组"],
    _ => &[],
  };
  candidates.iter().any(|c| desc.contains(c))
}

// ────────────────────────────────────────────────────────────────────────────
// Scope → root IUIAutomationElement
// ────────────────────────────────────────────────────────────────────────────

/// Resolve an ElementScope to a UIA root element. For Application scope we use
/// the first top-level window of that PID; for Foreground we use the OS
/// foreground window.
fn scope_to_root(automation: &IUIAutomation, scope: &ElementScope) -> Result<IUIAutomationElement> {
  let hwnd: i64 = match scope {
    ElementScope::Window(h) => h.0,
    ElementScope::Application(pid) => {
      let mut wins = wnd::get_all_windows_by_process_id_internal(*pid)?;
      // Prefer visible non-tiny windows, but fall back to anything if needed.
      let mut filtered: Vec<_> = wins
        .iter()
        .filter(|w| w.is_visible && w.width > 50 && w.height > 50)
        .cloned()
        .collect();
      if filtered.is_empty() {
        filtered = wins.drain(..).filter(|w| w.width > 50 && w.height > 50).collect();
      }
      filtered.sort_by_key(|w| -((w.width as i64) * (w.height as i64)));
      filtered
        .first()
        .map(|w| w.hwnd)
        .ok_or_else(|| anyhow::anyhow!("no top-level window for pid {pid}"))?
    }
    ElementScope::Foreground => {
      use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
      let h = unsafe { GetForegroundWindow() };
      if h.is_invalid() {
        return Err(anyhow::anyhow!("no foreground window"));
      }
      h.0 as isize as i64
    }
  };
  let h = HWND(hwnd as *mut core::ffi::c_void);
  let root = unsafe { automation.ElementFromHandle(h)? };
  Ok(root)
}

// ────────────────────────────────────────────────────────────────────────────
// Matcher
// ────────────────────────────────────────────────────────────────────────────

struct MatchCtx {
  role_ids: Vec<UIA_CONTROLTYPE_ID>,
  role_terms: Vec<String>, // for LocalizedControlType fallback
  text: Option<String>,
  text_exact: bool,
  include_hidden: bool,
}

impl MatchCtx {
  fn from_query(q: &ElementQuery) -> Self {
    let (role_ids, role_terms) = match &q.role {
      Some(r) => {
        let mut ids = Vec::new();
        let mut terms = Vec::new();
        for raw in r.split(|c| c == ',' || c == '|') {
          let t = raw.trim();
          if t.is_empty() {
            continue;
          }
          let mapped = abstract_role_to_ids(t);
          if !mapped.is_empty() {
            ids.extend(mapped);
          }
          terms.push(t.to_string());
        }
        (ids, terms)
      }
      None => (Vec::new(), Vec::new()),
    };
    Self {
      role_ids,
      role_terms,
      text: q.text.clone(),
      text_exact: q.text_exact,
      include_hidden: q.include_hidden,
    }
  }

  fn role_ok(&self, el: &IUIAutomationElement) -> bool {
    if self.role_terms.is_empty() {
      return true;
    }
    let cid = unsafe { el.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) };
    if self.role_ids.iter().any(|id| *id == cid) {
      return true;
    }
    // Fallback: LocalizedControlType (English or Chinese description).
    let localized = unsafe { el.CurrentLocalizedControlType().unwrap_or_default().to_string() };
    if localized.is_empty() {
      return false;
    }
    self.role_terms.iter().any(|t| matches_localized_role(&localized, t))
  }

  fn text_ok(&self, el: &IUIAutomationElement) -> bool {
    let Some(t) = &self.text else {
      return true;
    };
    let name = unsafe { el.CurrentName().unwrap_or_default().to_string() };
    let aid = unsafe { el.CurrentAutomationId().unwrap_or_default().to_string() };
    let cls = unsafe { el.CurrentClassName().unwrap_or_default().to_string() };
    let help = unsafe { el.CurrentHelpText().unwrap_or_default().to_string() };
    let localized = unsafe { el.CurrentLocalizedControlType().unwrap_or_default().to_string() };
    let value = current_value(el).unwrap_or_default();

    if self.text_exact {
      [&name, &aid, &cls, &help, &localized, &value]
        .iter()
        .any(|s| s.as_str() == t.as_str())
    } else {
      let tl = t.to_lowercase();
      [&name, &aid, &cls, &help, &localized, &value]
        .iter()
        .any(|s| s.to_lowercase().contains(&tl))
    }
  }

  fn visible_ok(&self, el: &IUIAutomationElement) -> bool {
    if self.include_hidden {
      return true;
    }
    let offscreen = unsafe { el.CurrentIsOffscreen().unwrap_or(BOOL::from(true)).as_bool() };
    if offscreen {
      return false;
    }
    let rect: RECT = unsafe { el.CurrentBoundingRectangle().unwrap_or_default() };
    rect.right > rect.left && rect.bottom > rect.top
  }
}

fn current_value(el: &IUIAutomationElement) -> Option<String> {
  unsafe {
    if let Ok(vp) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
      && let Ok(v) = vp.CurrentValue()
    {
      let s = v.to_string();
      if !s.is_empty() {
        return Some(s);
      }
    }
  }
  None
}

// ────────────────────────────────────────────────────────────────────────────
// Walk
// ────────────────────────────────────────────────────────────────────────────

fn walk(
  walker: &IUIAutomationTreeWalker,
  el: &IUIAutomationElement,
  depth: usize,
  max_depth: usize,
  ctx: &MatchCtx,
  out: &mut Vec<WindowsElement>,
) {
  // Match against current element first.
  if ctx.role_ok(el) && ctx.text_ok(el) && ctx.visible_ok(el) {
    out.push(WindowsElement {
      depth: Some(depth as i32),
      selector: String::new(),
      element: el.clone(),
    });
  }
  if depth >= max_depth {
    return;
  }
  let mut child = match unsafe { walker.GetFirstChildElement(el) } {
    Ok(c) => Some(c),
    Err(_) => None,
  };
  while let Some(c) = child {
    walk(walker, &c, depth + 1, max_depth, ctx, out);
    child = unsafe { walker.GetNextSiblingElement(&c).ok() };
  }
}

// ────────────────────────────────────────────────────────────────────────────
// Public API
// ────────────────────────────────────────────────────────────────────────────

pub fn query_elements(scope: &ElementScope, query: &ElementQuery) -> Result<Vec<Box<dyn Element>>> {
  ensure_com_initialized();
  let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
  let root = scope_to_root(&automation, scope)?;
  // ControlViewWalker filters out the noise of intermediate "content view" nodes
  // that pure RawViewWalker exposes — for Electron/CEF apps this cuts the
  // walk from many seconds to milliseconds.
  let walker: IUIAutomationTreeWalker = unsafe { automation.RawViewWalker()? };
  let ctx = MatchCtx::from_query(query);
  let max_depth = query.max_depth.unwrap_or(40);
  let mut found = Vec::new();
  walk(&walker, &root, 0, max_depth, &ctx, &mut found);
  Ok(found.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
}

pub fn query_one(scope: &ElementScope, query: &ElementQuery) -> Result<Box<dyn Element>> {
  let mut all = query_elements(scope, query)?;
  if all.is_empty() {
    return Err(anyhow::anyhow!("no element matches query: {query:?}"));
  }
  if let Some(idx) = query.index {
    if idx >= all.len() {
      return Err(anyhow::anyhow!(
        "element index {idx} out of range (found {})",
        all.len()
      ));
    }
    return Ok(all.swap_remove(idx));
  }
  Ok(all.remove(0))
}

// ────────────────────────────────────────────────────────────────────────────
// Tree render
// ────────────────────────────────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

fn render_node(
  walker: &IUIAutomationTreeWalker,
  el: &IUIAutomationElement,
  depth: usize,
  max_depth: usize,
  has_filter: bool,
  ctx: &MatchCtx,
  out: &mut String,
) -> bool {
  if depth > max_depth {
    return false;
  }
  let visible = ctx.visible_ok(el);
  let matches_self = visible && ctx.role_ok(el) && ctx.text_ok(el);

  // Compute children output first to decide whether to emit this node.
  let mut child_out = String::new();
  let mut any_child = false;
  if depth < max_depth {
    let mut child = unsafe { walker.GetFirstChildElement(el).ok() };
    while let Some(c) = child {
      if render_node(walker, &c, depth + 1, max_depth, has_filter, ctx, &mut child_out) {
        any_child = true;
      }
      child = unsafe { walker.GetNextSiblingElement(&c).ok() };
    }
  }

  let emit = if has_filter {
    matches_self || any_child
  } else {
    visible || any_child
  };
  if !emit {
    return false;
  }

  let pad = "  ".repeat(depth);
  let cid = unsafe { el.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) };
  let ct = crate::platform::windows::elements::utils::get_control_type_name(cid);
  let name = unsafe { el.CurrentName().unwrap_or_default().to_string() };
  let aid = unsafe { el.CurrentAutomationId().unwrap_or_default().to_string() };
  let cls = unsafe { el.CurrentClassName().unwrap_or_default().to_string() };
  let localized = unsafe { el.CurrentLocalizedControlType().unwrap_or_default().to_string() };
  let value = current_value(el).unwrap_or_default();
  let rect: RECT = unsafe { el.CurrentBoundingRectangle().unwrap_or_default() };
  let (x, y, w, h) = (rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top);

  out.push_str(&format!("{pad}<{ct}"));
  if !name.is_empty() {
    out.push_str(&format!(" Name=\"{}\"", xml_escape(&name)));
  }
  if !aid.is_empty() {
    out.push_str(&format!(" AutomationId=\"{}\"", xml_escape(&aid)));
  }
  if !cls.is_empty() {
    out.push_str(&format!(" ClassName=\"{}\"", xml_escape(&cls)));
  }
  if !localized.is_empty() && !localized.eq_ignore_ascii_case(&ct) {
    out.push_str(&format!(" LocalizedType=\"{}\"", xml_escape(&localized)));
  }
  if !value.is_empty() {
    out.push_str(&format!(" Value=\"{}\"", xml_escape(&value)));
  }
  out.push_str(&format!(" X=\"{x}\" Y=\"{y}\" Width=\"{w}\" Height=\"{h}\""));

  if child_out.is_empty() {
    out.push_str(" />\n");
  } else {
    out.push_str(">\n");
    out.push_str(&child_out);
    out.push_str(&format!("{pad}</{ct}>\n"));
  }
  matches_self || any_child
}

pub fn render_tree(scope: &ElementScope, query: &ElementQuery) -> Result<String> {
  ensure_com_initialized();
  let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
  let root = scope_to_root(&automation, scope)?;
  let walker: IUIAutomationTreeWalker = unsafe { automation.RawViewWalker()? };
  let ctx = MatchCtx::from_query(query);
  let max_depth = query.max_depth.unwrap_or(40);
  let has_filter = query.role.is_some() || query.text.is_some();
  let mut out = String::new();
  render_node(&walker, &root, 0, max_depth, has_filter, &ctx, &mut out);
  Ok(out)
}

// ────────────────────────────────────────────────────────────────────────────
// Probe (ElementFromPoint)
// ────────────────────────────────────────────────────────────────────────────

pub fn probe_at_position(x: i32, y: i32) -> Result<String> {
  ensure_com_initialized();
  let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
  let pt = windows::Win32::Foundation::POINT { x, y };
  let el = unsafe { automation.ElementFromPoint(pt)? };

  let cid = unsafe { el.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) };
  let ct = crate::platform::windows::elements::utils::get_control_type_name(cid);
  let name = unsafe { el.CurrentName().unwrap_or_default().to_string() };
  let aid = unsafe { el.CurrentAutomationId().unwrap_or_default().to_string() };
  let cls = unsafe { el.CurrentClassName().unwrap_or_default().to_string() };
  let help = unsafe { el.CurrentHelpText().unwrap_or_default().to_string() };
  let localized = unsafe { el.CurrentLocalizedControlType().unwrap_or_default().to_string() };
  let acc_name = unsafe { el.CurrentAcceleratorKey().unwrap_or_default().to_string() };
  let access_key = unsafe { el.CurrentAccessKey().unwrap_or_default().to_string() };
  let proc_id = unsafe { el.CurrentProcessId().unwrap_or(0) };
  let is_enabled = unsafe { el.CurrentIsEnabled().unwrap_or(BOOL::from(false)).as_bool() };
  let is_offscreen = unsafe { el.CurrentIsOffscreen().unwrap_or(BOOL::from(true)).as_bool() };
  let is_keyboard_focusable = unsafe { el.CurrentIsKeyboardFocusable().unwrap_or(BOOL::from(false)).as_bool() };
  let has_keyboard_focus = unsafe { el.CurrentHasKeyboardFocus().unwrap_or(BOOL::from(false)).as_bool() };
  let rect: RECT = unsafe { el.CurrentBoundingRectangle().unwrap_or_default() };
  let value = current_value(&el).unwrap_or_default();

  let mut patterns = Vec::<&'static str>::new();
  unsafe {
    if el
      .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
      .is_ok()
    {
      patterns.push("Invoke");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
      .is_ok()
    {
      patterns.push("Value");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
      .is_ok()
    {
      patterns.push("Toggle");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
      .is_ok()
    {
      patterns.push("SelectionItem");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
      .is_ok()
    {
      patterns.push("ExpandCollapse");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationRangeValuePattern>(UIA_RangeValuePatternId)
      .is_ok()
    {
      patterns.push("RangeValue");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
      .is_ok()
    {
      patterns.push("Text");
    }
    if el
      .GetCurrentPatternAs::<IUIAutomationScrollPattern>(UIA_ScrollPatternId)
      .is_ok()
    {
      patterns.push("Scroll");
    }
  }

  let mut out = String::new();
  out.push_str(&format!("Probe at ({x}, {y})\n"));
  out.push_str(&format!("  ControlType: {ct} ({cid:?})\n"));
  if !localized.is_empty() {
    out.push_str(&format!("  LocalizedType: {localized}\n"));
  }
  out.push_str(&format!("  Name: {name}\n"));
  if !aid.is_empty() {
    out.push_str(&format!("  AutomationId: {aid}\n"));
  }
  if !cls.is_empty() {
    out.push_str(&format!("  ClassName: {cls}\n"));
  }
  if !help.is_empty() {
    out.push_str(&format!("  HelpText: {help}\n"));
  }
  if !value.is_empty() {
    out.push_str(&format!("  Value: {value}\n"));
  }
  if !acc_name.is_empty() {
    out.push_str(&format!("  AcceleratorKey: {acc_name}\n"));
  }
  if !access_key.is_empty() {
    out.push_str(&format!("  AccessKey: {access_key}\n"));
  }
  out.push_str(&format!("  ProcessId: {proc_id}\n"));
  out.push_str(&format!("  Enabled: {is_enabled}\n"));
  out.push_str(&format!("  Offscreen: {is_offscreen}\n"));
  out.push_str(&format!("  KeyboardFocusable: {is_keyboard_focusable}\n"));
  out.push_str(&format!("  HasKeyboardFocus: {has_keyboard_focus}\n"));
  out.push_str(&format!(
    "  Rect: ({}, {}, {}, {}) = {}x{}\n",
    rect.left,
    rect.top,
    rect.right,
    rect.bottom,
    rect.right - rect.left,
    rect.bottom - rect.top
  ));
  out.push_str(&format!("  SupportedPatterns: [{}]\n", patterns.join(", ")));
  Ok(out)
}

// ────────────────────────────────────────────────────────────────────────────
// XPath (wires existing find.rs helper through ElementScope)
// ────────────────────────────────────────────────────────────────────────────

pub fn find_by_xpath(scope: &ElementScope, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
  let hwnd: i64 = match scope {
    ElementScope::Window(WindowHandle(h)) => *h,
    ElementScope::Application(pid) => {
      let mut wins = wnd::get_all_windows_by_process_id_internal(*pid)?;
      let mut filtered: Vec<_> = wins
        .iter()
        .filter(|w| w.is_visible && w.width > 50 && w.height > 50)
        .cloned()
        .collect();
      if filtered.is_empty() {
        filtered = wins.drain(..).filter(|w| w.width > 50 && w.height > 50).collect();
      }
      filtered.sort_by_key(|w| -((w.width as i64) * (w.height as i64)));
      filtered
        .first()
        .map(|w| w.hwnd)
        .ok_or_else(|| anyhow::anyhow!("no window for pid {pid}"))?
    }
    ElementScope::Foreground => {
      use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
      let h = unsafe { GetForegroundWindow() };
      if h.is_invalid() {
        return Err(anyhow::anyhow!("no foreground window"));
      }
      h.0 as isize as i64
    }
  };
  let found = crate::platform::windows::elements::find::find_elements_by_handle_xpath_internal(hwnd, xpath)?;
  Ok(found.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
}
