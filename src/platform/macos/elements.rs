use accessibility::{AXAttribute, AXUIElement, action::AXUIElementActions};
use core_foundation::base::TCFType;

use crate::error::Result;
use crate::platform::Element;
use crate::types::*;

// AX role -> control type mapping
fn role_to_control_type(role: &str) -> &str {
  match role {
    "AXApplication" => "Application",
    "AXWindow" => "Window",
    "AXButton" => "Button",
    "AXCheckBox" => "CheckBox",
    "AXRadioButton" => "RadioButton",
    "AXTextField" => "Edit",
    "AXTextArea" => "Edit",
    "AXStaticText" => "Text",
    "AXSlider" => "Slider",
    "AXMenu" => "Menu",
    "AXMenuBar" => "MenuBar",
    "AXMenuItem" => "MenuItem",
    "AXMenuBarItem" => "MenuItem",
    "AXToolbar" => "Toolbar",
    "AXTable" => "Table",
    "AXRow" => "DataItem",
    "AXColumn" => "Header",
    "AXList" => "List",
    "AXGroup" => "Group",
    "AXTabGroup" => "Tab",
    "AXTab" => "TabItem",
    "AXPopUpButton" => "ComboBox",
    "AXComboBox" => "ComboBox",
    "AXDisclosureTriangle" => "Button",
    "AXProgressIndicator" => "ProgressBar",
    "AXBusyIndicator" => "ProgressBar",
    "AXImage" => "Image",
    "AXScrollArea" => "Pane",
    "AXSplitGroup" => "Group",
    "AXSplitter" => "Thumb",
    "AXToolbarButton" => "Button",
    "AXCloseButton" => "Button",
    "AXZoomButton" => "Button",
    "AXMinimizeButton" => "Button",
    "AXFullScreenButton" => "Button",
    _ => "Custom",
  }
}

fn cfstr(s: &str) -> core_foundation::string::CFString {
  core_foundation::string::CFString::new(s)
}

// --- MacElement wrapper ---

pub struct MacElement {
  element: AXUIElement,
  cached_name: String,
  cached_role: String,
  cached_value: Option<String>,
  cached_position: Option<(f64, f64)>,
  cached_size: Option<(f64, f64)>,
  cached_enabled: bool,
}

// SAFETY: AXUIElement wraps a *mut __AXUIElement which is a Core Foundation type.
// The macOS Accessibility API is thread-safe for AXUIElement operations.
unsafe impl Send for MacElement {}

impl MacElement {
  pub fn new(element: AXUIElement) -> Self {
    let cached_name = element
      .attribute(&AXAttribute::title())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();

    let cached_role = element
      .attribute(&AXAttribute::role())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();

    let cached_value = element
      .attribute(&AXAttribute::value())
      .ok()
      .and_then(|v| {
        if v.instance_of::<core_foundation::string::CFString>() {
          let s: core_foundation::string::CFString =
            unsafe { core_foundation::base::TCFType::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
          Some(s.to_string())
        } else {
          None
        }
      });

    let cached_position = element
      .attribute(&AXAttribute::new(&cfstr("AXPosition")))
      .ok()
      .and_then(|v| extract_point_from_axvalue(&v));

    let cached_size = element
      .attribute(&AXAttribute::new(&cfstr("AXSize")))
      .ok()
      .and_then(|v| extract_size_from_axvalue(&v));

    let cached_enabled = element
      .attribute(&AXAttribute::enabled())
      .ok()
      .map(|v| bool::from(v))
      .unwrap_or(false);

    MacElement {
      element,
      cached_name,
      cached_role,
      cached_value,
      cached_position,
      cached_size,
      cached_enabled,
    }
  }
}

fn extract_point_from_axvalue(val: &core_foundation::base::CFType) -> Option<(f64, f64)> {
  use accessibility_sys::{AXValueGetValue, kAXValueTypeCGPoint};
  use core_graphics::geometry::CGPoint;
  unsafe {
    let mut point = CGPoint { x: 0.0, y: 0.0 };
    let ok = AXValueGetValue(
      val.as_CFTypeRef() as *mut _,
      kAXValueTypeCGPoint,
      &mut point as *mut _ as *mut _,
    );
    if ok {
      Some((point.x, point.y))
    } else {
      None
    }
  }
}

fn extract_size_from_axvalue(val: &core_foundation::base::CFType) -> Option<(f64, f64)> {
  use accessibility_sys::{AXValueGetValue, kAXValueTypeCGSize};
  use core_graphics::geometry::CGSize;
  unsafe {
    let mut size = CGSize {
      width: 0.0,
      height: 0.0,
    };
    let ok = AXValueGetValue(
      val.as_CFTypeRef() as *mut _,
      kAXValueTypeCGSize,
      &mut size as *mut _ as *mut _,
    );
    if ok {
      Some((size.width, size.height))
    } else {
      None
    }
  }
}

fn get_children(element: &AXUIElement) -> Vec<AXUIElement> {
  let mut children = Vec::new();
  if let Ok(arr) = element.attribute(&AXAttribute::children()) {
    for i in 0..arr.len() {
      if let Some(item) = arr.get(i) {
        let child: AXUIElement = item.clone();
        children.push(child);
      }
    }
  }
  children
}

impl Element for MacElement {
  fn automation_id(&self) -> String {
    self
      .element
      .attribute(&AXAttribute::identifier())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default()
  }

  fn name(&self) -> String {
    self.cached_name.clone()
  }

  fn class_name(&self) -> String {
    self
      .element
      .attribute(&AXAttribute::role_description())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_else(|| self.cached_role.clone())
  }

  fn control_type(&self) -> String {
    role_to_control_type(&self.cached_role).to_string()
  }

  fn help_text(&self) -> String {
    self
      .element
      .attribute(&AXAttribute::help())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default()
  }

  fn is_enabled(&self) -> bool {
    self.cached_enabled
  }

  fn is_clickable(&self) -> bool {
    matches!(
      self.cached_role.as_str(),
      "AXButton"
        | "AXCheckBox"
        | "AXRadioButton"
        | "AXMenuItem"
        | "AXMenuBarItem"
        | "AXPopUpButton"
        | "AXComboBox"
        | "AXLink"
    )
  }

  fn x(&self) -> i32 {
    self.cached_position.map(|(x, _)| x as i32).unwrap_or(0)
  }

  fn y(&self) -> i32 {
    self.cached_position.map(|(_, y)| y as i32).unwrap_or(0)
  }

  fn width(&self) -> i32 {
    self.cached_size.map(|(w, _)| w as i32).unwrap_or(0)
  }

  fn height(&self) -> i32 {
    self.cached_size.map(|(_, h)| h as i32).unwrap_or(0)
  }

  fn text(&self) -> String {
    self.cached_value.clone().unwrap_or_else(|| self.cached_name.clone())
  }

  fn pos(&self, _window: Option<&WindowHandle>) -> Result<ElementPosition> {
    let (px, py) = self.cached_position.unwrap_or((0.0, 0.0));
    let (pw, ph) = self.cached_size.unwrap_or((0.0, 0.0));
    Ok(ElementPosition {
      left: px as i32,
      top: py as i32,
      right: (px + pw) as i32,
      bottom: (py + ph) as i32,
      center_x: (px + pw / 2.0) as i32,
      center_y: (py + ph / 2.0) as i32,
      relative_center_x: (pw / 2.0) as i32,
      relative_center_y: (ph / 2.0) as i32,
      window_width: pw as i32,
      window_height: ph as i32,
    })
  }

  fn get_value(&self) -> Result<String> {
    Ok(self.cached_value.clone().unwrap_or_default())
  }

  fn set_value(&self, value: &str) -> bool {
    let cf_val = cfstr(value);
    let cf_type: core_foundation::base::CFType = unsafe { TCFType::wrap_under_get_rule(cf_val.as_CFTypeRef()) };
    self
      .element
      .set_attribute(&AXAttribute::new(&cfstr("AXValue")), cf_type)
      .is_ok()
  }

  fn can_set_value(&self) -> Result<bool> {
    Ok(matches!(
      self.cached_role.as_str(),
      "AXTextField" | "AXTextArea" | "AXSlider" | "AXComboBox" | "AXDateField"
    ))
  }

  fn set_range_value(&self, value: f64) -> Result<()> {
    use core_foundation::number::CFNumber;
    let num = CFNumber::from(value);
    let cf_type: core_foundation::base::CFType = unsafe { TCFType::wrap_under_get_rule(num.as_CFTypeRef()) };
    self
      .element
      .set_attribute(&AXAttribute::new(&cfstr("AXValue")), cf_type)
      .map_err(|e| anyhow::anyhow!("set_range_value failed: {e:?}"))
  }

  fn focus(&self) -> Result<()> {
    self
      .element
      .raise()
      .map_err(|e| anyhow::anyhow!("focus failed: {e:?}"))
  }

  fn confirm(&self) -> Result<()> {
    // Try confirm first, fall back to press
    if self.element.confirm().is_ok() {
      return Ok(());
    }
    self
      .element
      .press()
      .map_err(|e| anyhow::anyhow!("confirm/press failed: {e:?}"))
  }

  fn is_focused(&self) -> Result<bool> {
    self
      .element
      .attribute(&AXAttribute::focused())
      .ok()
      .map(|v| bool::from(v))
      .ok_or_else(|| anyhow::anyhow!("AXFocused not available"))
  }

  fn to_xml(&self, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let aid = self.automation_id();
    let mut xml = format!(
      "{pad}<{} Name=\"{}\"",
      self.control_type(),
      xml_escape(&self.cached_name),
    );
    if !aid.is_empty() {
      xml.push_str(&format!(" AutomationId=\"{}\"", xml_escape(&aid)));
    }
    if let Some(ref val) = self.cached_value {
      xml.push_str(&format!(" Value=\"{}\"", xml_escape(val)));
    }
    xml.push_str(&format!(
      " Enabled=\"{}\" X=\"{}\" Y=\"{}\" Width=\"{}\" Height=\"{}\"",
      self.cached_enabled,
      self.x(),
      self.y(),
      self.width(),
      self.height(),
    ));

    let children = get_children(&self.element);
    if children.is_empty() {
      xml.push_str(" />");
    } else {
      xml.push('>');
      xml.push('\n');
      for child in children {
        let child_elem = MacElement::new(child);
        xml.push_str(&child_elem.to_xml(indent + 1));
        xml.push('\n');
      }
      xml.push_str(&format!("{pad}</{}>", self.control_type()));
    }
    xml
  }
}

fn xml_escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

// --- XPath-like selector engine ---

/// Parse a simple XPath-like selector and find matching elements.
/// Supported syntax:
///   "Button" - match by role
///   "Button[@name=\"OK\"]" - match by role and name
///   "Button[1]" - match by role and index
///   "*/Button" - match Button under any parent
///   "Window[1]/Button[@name=\"OK\"]" - nested path
fn find_elements_by_selector(root: &AXUIElement, xpath: &str) -> Vec<MacElement> {
  let parts = split_xpath(xpath);
  if parts.is_empty() {
    return Vec::new();
  }

  if parts.len() == 1 {
    // Single-part selector: BFS the entire tree
    let (target_role, name_filter, _) = parse_selector_part(&parts[0]);
    return find_all_matching(root, &target_role, &name_filter);
  }

  // Multi-part selector: walk intermediate parts, BFS for leaf
  let leaf_part = &parts[parts.len() - 1];
  let (leaf_role, leaf_name, _) = parse_selector_part(leaf_part);

  // Find all intermediate containers matching the path prefix
  let mut candidates = vec![root.clone()];
  for (i, part) in parts[..parts.len() - 1].iter().enumerate() {
    let (target_role, name_filter, _) = parse_selector_part(part);
    let mut next = Vec::new();
    for parent in &candidates {
      let mut queue = std::collections::VecDeque::new();
      if i == 0 {
        queue.push_back(parent.clone());
      } else {
        if let Ok(children) = parent.attribute(&AXAttribute::children()) {
          for j in 0..children.len() {
            if let Some(child) = children.get(j) {
              queue.push_back(child.clone());
            }
          }
        }
      }
      while let Some(elem) = queue.pop_front() {
        let role = elem
          .attribute(&AXAttribute::role())
          .ok()
          .map(|s| s.to_string())
          .unwrap_or_default();
        let title = elem
          .attribute(&AXAttribute::title())
          .ok()
          .map(|s| s.to_string())
          .unwrap_or_default();
        let value = get_ax_value_as_string(&elem);
        let searchable = if title.is_empty() { value } else { title };
        let ct = role_to_control_type(&role);
        let matches = target_role == "*"
          || role == target_role
          || ct.eq_ignore_ascii_case(&target_role);
        let name_ok = match &name_filter {
          Some(n) => searchable.to_lowercase().contains(&n.to_lowercase()),
          None => true,
        };
        if matches && name_ok {
          next.push(elem.clone());
        } else {
          // Keep searching deeper for intermediate parts
          if let Ok(children) = elem.attribute(&AXAttribute::children()) {
            for j in 0..children.len() {
              if let Some(child) = children.get(j) {
                queue.push_back(child.clone());
              }
            }
          }
        }
      }
    }
    candidates = next;
  }

  // BFS for leaf part within each candidate container
  let mut results = Vec::new();
  for container in &candidates {
    results.extend(find_all_matching(container, &leaf_role, &leaf_name));
  }
  results
}

fn split_xpath(xpath: &str) -> Vec<String> {
  // Split by '/' but not inside brackets
  let mut parts = Vec::new();
  let mut current = String::new();
  let mut in_bracket = 0;
  for ch in xpath.chars() {
    match ch {
      '[' => {
        in_bracket += 1;
        current.push(ch);
      }
      ']' => {
        in_bracket -= 1;
        current.push(ch);
      }
      '/' if in_bracket == 0 => {
        if !current.is_empty() {
          parts.push(current.clone());
          current.clear();
        }
      }
      _ => current.push(ch),
    }
  }
  if !current.is_empty() {
    parts.push(current);
  }
  parts
}

/// Find all elements matching a single selector part (role + optional name filter)
/// by doing a full BFS traversal of the tree.
fn find_all_matching(root: &AXUIElement, target_role: &str, name_filter: &Option<String>) -> Vec<MacElement> {
  let mut results = Vec::new();
  let mut queue = std::collections::VecDeque::new();
  queue.push_back(root.clone());

  while let Some(elem) = queue.pop_front() {
    let role = elem
      .attribute(&AXAttribute::role())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();

    // Check both title and value for name matching (web content uses value)
    let title = elem
      .attribute(&AXAttribute::title())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();
    let value = get_ax_value_as_string(&elem);
    let searchable = if title.is_empty() { value } else { title };

    let control_type = role_to_control_type(&role);
    let matches_role = target_role == "*"
      || role == target_role
      || control_type.eq_ignore_ascii_case(target_role);
    let matches_name = match name_filter {
      Some(n) => searchable.to_lowercase().contains(&n.to_lowercase()),
      None => true,
    };

    if matches_role && matches_name {
      results.push(MacElement::new(elem.clone()));
    }

    if let Ok(children) = elem.attribute(&AXAttribute::children()) {
      for i in 0..children.len() {
        if let Some(child) = children.get(i) {
          queue.push_back(child.clone());
        }
      }
    }
  }

  results
}

/// Extract AXValue as a string, if it is a CFString.
fn get_ax_value_as_string(elem: &AXUIElement) -> String {
  elem
    .attribute(&AXAttribute::value())
    .ok()
    .and_then(|v| {
      if v.instance_of::<core_foundation::string::CFString>() {
        let s: core_foundation::string::CFString =
          unsafe { core_foundation::base::TCFType::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
        Some(s.to_string())
      } else {
        None
      }
    })
    .unwrap_or_default()
}

fn parse_selector_part(part: &str) -> (String, Option<String>, Option<usize>) {
  // Parse "Role[@name=\"value\"]" or "Role[index]"
  let (role, rest) = if let Some(bracket_pos) = part.find('[') {
    (&part[..bracket_pos], Some(&part[bracket_pos..]))
  } else {
    (part.as_ref(), None)
  };

  let role = if role == "*" { "*" } else { role };

  let mut name_filter = None;
  let mut index_filter = None;

  if let Some(rest) = rest {
    let inner = rest.trim_start_matches('[').trim_end_matches(']');
    if let Some(eq_pos) = inner.find("=\"") {
      // @name="value" or @name='value'
      let value = inner[eq_pos + 2..].trim_end_matches('"').trim_end_matches('\'');
      name_filter = Some(value.to_string());
    } else if inner.chars().all(|c| c.is_ascii_digit()) {
      index_filter = inner.parse().ok();
    }
  }

  (role.to_string(), name_filter, index_filter)
}

// --- Public API for ElementFinder and UiTreeInspector ---

pub fn find_elements_by_xpath(handle: &WindowHandle, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
  let app = app_for_window_handle(handle)?;
  let results = find_elements_by_selector(&app, xpath);
  Ok(results.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
}

pub fn find_first_element_by_xpath(handle: &WindowHandle, xpath: &str) -> Result<Box<dyn Element>> {
  let app = app_for_window_handle(handle)?;
  let results = find_elements_by_selector(&app, xpath);
  results
    .into_iter()
    .next()
    .map(|e| Box::new(e) as Box<dyn Element>)
    .ok_or_else(|| anyhow::anyhow!("element not found: {xpath}"))
}

pub fn get_page_source(handle: &WindowHandle) -> Result<String> {
  let app = app_for_window_handle(handle)?;
  let elem = MacElement::new(app);
  Ok(elem.to_xml(0))
}

/// Get the AXUIElement for the application owning a window.
fn app_for_window_handle(handle: &WindowHandle) -> Result<AXUIElement> {
  let wins = crate::platform::macos::window::list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;
  Ok(AXUIElement::application(win.process_id as i32))
}
