use accessibility::{AXAttribute, AXUIElement, action::AXUIElementActions};
use core_foundation::base::TCFType;

use crate::error::Result;
use crate::platform::Element;
use crate::types::*;

/// Attributes that may contain child-like elements for apps that don't expose AXChildren.
const CHILD_CANDIDATE_ATTRS: &[&str] = &[
  "AXChildren",
  "AXVisibleChildren",
  "AXRows",
  "AXColumns",
  "AXTabs",
  "AXContents",
  "AXSections",
  "AXLabelUIElements",
  "AXLinkedUIElements",
  "AXServesAsTitleForUIElements",
  "AXSharedTextElements",
  "AXSharedCharacterElements",
  "AXSplitters",
  "AXLayoutItems",
  "AXLayoutAreas",
  "AXDisclosedRows",
  "AXDisclosedByRow",
  "AXSelectedRows",
  "AXVisibleRows",
  "AXVisibleColumns",
  "AXVisibleTabs",
  "AXHeader",
  "AXRowHeaders",
  "AXColumnHeaders",
];

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
  // Use raw FFI for children access — the accessibility crate's attribute() wrapper
  // can cause SIGBUS on SwiftUI apps due to incorrect reference counting semantics.
  // This approach (from kortix-ai/agent-computer-use) uses wrap_under_create_rule
  // which correctly matches AXUIElementCopyAttributeValue's Create Rule return.
  let children = get_children_via_ffi(element, "AXChildren");
  if !children.is_empty() {
    return children;
  }

  // Fallback: try alternative attributes for apps that don't expose AXChildren (Qt/Electron).
  for attr_name in &CHILD_CANDIDATE_ATTRS[1..] {
    let children = get_children_via_ffi(element, attr_name);
    if !children.is_empty() {
      return children;
    }
  }

  Vec::new()
}

/// Get children using raw FFI with proper reference counting.
/// Uses wrap_under_create_rule to match AXUIElementCopyAttributeValue's return semantics.
fn get_children_via_ffi(element: &AXUIElement, attr_name: &str) -> Vec<AXUIElement> {
  use accessibility_sys::{AXUIElementCopyAttributeValue, kAXErrorSuccess};

  let elem_ref = element.as_concrete_TypeRef();
  let cf_attr = cfstr(attr_name);
  let mut value: core_foundation::base::CFTypeRef = std::ptr::null_mut();

  let err = unsafe { AXUIElementCopyAttributeValue(elem_ref, cf_attr.as_concrete_TypeRef(), &mut value) };
  if err != kAXErrorSuccess || value.is_null() {
    return Vec::new();
  }

  // wrap_under_create_rule: the API returns a +1 retained object, we own it.
  let cf_array: core_foundation::array::CFArray<core_foundation::base::CFType> =
    unsafe { TCFType::wrap_under_create_rule(value as *const _) };

  let count = cf_array.len();
  if count == 0 {
    return Vec::new();
  }

  let ax_type_id = unsafe { accessibility_sys::AXUIElementGetTypeID() };
  let mut children = Vec::with_capacity(count as usize);

  for i in 0..count {
    if let Some(item) = cf_array.get(i) {
      let item_ref = item.as_CFTypeRef();
      if item_ref.is_null() {
        continue;
      }
      let item_type = unsafe { core_foundation::base::CFGetTypeID(item_ref as *const _) };
      if item_type == ax_type_id {
        // Each child from the array is retained by the array.
        // wrap_under_get_rule calls CFRetain, giving us an independent reference.
        let ax_ref = item_ref as accessibility_sys::AXUIElementRef;
        let elem: AXUIElement = unsafe { TCFType::wrap_under_get_rule(ax_ref) };
        children.push(elem);
      }
    }
  }

  children
}

/// Get children with crash protection.
fn get_children_safely(element: &AXUIElement) -> Vec<AXUIElement> {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| get_children(element))).unwrap_or_default()
}

/// Dump all AX attributes of an element as a JSON-like string (for diagnostics).
pub fn dump_attributes(element: &AXUIElement) -> String {
  use accessibility_sys::{AXUIElementCopyAttributeNames, kAXErrorSuccess};
  use core_foundation::array::CFArray;
  use core_foundation::string::CFString;

  let mut names_ref: *const core_foundation::array::__CFArray = std::ptr::null();
  let err = unsafe { AXUIElementCopyAttributeNames(element.as_concrete_TypeRef(), &mut names_ref) };
  if err != kAXErrorSuccess || names_ref.is_null() {
    return String::from("{}");
  }
  let names: CFArray<CFString> = unsafe { TCFType::wrap_under_create_rule(names_ref as *const _) };

  let mut out = String::from("{\n");
  for i in 0..names.len() {
    let Some(name_cf) = names.get(i) else { continue };
    let name = name_cf.to_string();

    // Get the raw value
    match element.attribute(&AXAttribute::new(&cfstr(&name))) {
      Ok(val) => {
        // Try to extract as string
        if val.instance_of::<core_foundation::string::CFString>() {
          let s: CFString = unsafe { TCFType::wrap_under_get_rule(val.as_CFTypeRef() as *const _) };
          out.push_str(&format!("  \"{}\": \"{}\",\n", name, s.to_string().replace('"', "\\\"")));
        }
        // Try to extract as number
        else if val.instance_of::<core_foundation::number::CFNumber>() {
          let n: core_foundation::number::CFNumber =
            unsafe { TCFType::wrap_under_get_rule(val.as_CFTypeRef() as *const _) };
          if let Some(v) = n.to_i64() {
            out.push_str(&format!("  \"{}\": {},\n", name, v));
          } else if let Some(v) = n.to_f64() {
            out.push_str(&format!("  \"{}\": {},\n", name, v));
          }
        }
        // Try to extract as boolean
        else if val.instance_of::<core_foundation::boolean::CFBoolean>() {
          let b: core_foundation::boolean::CFBoolean =
            unsafe { TCFType::wrap_under_get_rule(val.as_CFTypeRef() as *const _) };
          out.push_str(&format!("  \"{}\": {},\n", name, bool::from(b)));
        }
        // Try to extract as array — show count
        else if val.instance_of::<core_foundation::array::CFArray>() {
          let arr: core_foundation::array::CFArray<core_foundation::base::CFType> =
            unsafe { TCFType::wrap_under_get_rule(val.as_CFTypeRef() as *const _) };
          out.push_str(&format!("  \"{}\": [{} items],\n", name, arr.len() as usize));
        }
        // Try AXValue (point/size/rect)
        else {
          use accessibility_sys::{AXValueGetValue, kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXValueTypeCGRect};
          use core_graphics::geometry::{CGPoint, CGRect, CGSize};

          let mut rect = CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 0.0, height: 0.0 } };
          let ok = unsafe {
            AXValueGetValue(val.as_CFTypeRef() as *mut _, kAXValueTypeCGRect, &mut rect as *mut _ as *mut _)
          };
          if ok {
            out.push_str(&format!(
              "  \"{}\": [{},{},{},{}],\n",
              name, rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
            ));
          } else {
            // Try point
            let mut point = CGPoint { x: 0.0, y: 0.0 };
            let ok = unsafe {
              AXValueGetValue(val.as_CFTypeRef() as *mut _, kAXValueTypeCGPoint, &mut point as *mut _ as *mut _)
            };
            if ok {
              out.push_str(&format!("  \"{}\": [{},{}],\n", name, point.x, point.y));
            } else {
              // Try size
              let mut size = CGSize { width: 0.0, height: 0.0 };
              let ok = unsafe {
                AXValueGetValue(val.as_CFTypeRef() as *mut _, kAXValueTypeCGSize, &mut size as *mut _ as *mut _)
              };
              if ok {
                out.push_str(&format!("  \"{}\": [{},{}],\n", name, size.width, size.height));
              } else {
                // Unknown type, show the CFType description
                out.push_str(&format!("  \"{}\": <unknown>,\n", name));
              }
            }
          }
        }
      }
      Err(_) => {
        // Attribute exists but can't be read — likely not applicable to this element
        out.push_str(&format!("  \"{}\": <not applicable>,\n", name));
      }
    }
  }
  out.push('}');
  out
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

  /// Set AXFocused=true on this element to give it keyboard focus.
  fn set_focused(&self) -> Result<()> {
    let cf_type: core_foundation::base::CFType =
      unsafe { TCFType::wrap_under_get_rule(core_foundation::boolean::CFBoolean::true_value().as_CFTypeRef()) };
    self
      .element
      .set_attribute(&AXAttribute::new(&cfstr("AXFocused")), cf_type)
      .map_err(|e| anyhow::anyhow!("set_focused failed: {e:?}"))
  }

  fn confirm(&self) -> Result<()> {
    // Try multiple actions in order of preference:
    // 1. AXConfirm (standard confirm action)
    // 2. AXPress (standard press action for buttons)
    // 3. AXPick (for menu items)
    // 4. AXShowMenu (for context menus)
    // 5. AXOpen (for links/files)
    // 6. Click at center as last resort

    // Try AXConfirm first
    if self.element.confirm().is_ok() {
      return Ok(());
    }

    // Try AXPress
    if self.element.press().is_ok() {
      return Ok(());
    }

    // Try AXPick for menu items
    if self.cached_role == "AXMenuItem" || self.cached_role == "AXMenuBarItem" {
      if self.element.perform_action(&cfstr("AXPick")).is_ok() {
        return Ok(());
      }
    }

    // Try AXShowMenu for context menus
    if self.element.perform_action(&cfstr("AXShowMenu")).is_ok() {
      return Ok(());
    }

    // Try AXOpen for links/files
    if self.element.perform_action(&cfstr("AXOpen")).is_ok() {
      return Ok(());
    }

    // Last resort: click at the center of the element
    if let Some((x, y)) = self.cached_position {
      if let Some((w, h)) = self.cached_size {
        let center_x = x + w / 2.0;
        let center_y = y + h / 2.0;
        // Use CGEvent to click at the center
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
          let point = core_graphics::geometry::CGPoint::new(center_x, center_y);
          if let Ok(click_down) = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::LeftMouseDown,
            point,
            CGMouseButton::Left,
          ) {
            click_down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));

            if let Ok(click_up) = CGEvent::new_mouse_event(
              source.clone(),
              CGEventType::LeftMouseUp,
              point,
              CGMouseButton::Left,
            ) {
              click_up.post(CGEventTapLocation::HID);
              return Ok(());
            }
          }
        }
      }
    }

    Err(anyhow::anyhow!(
      "confirm failed: element does not support any known activation actions"
    ))
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
    element_to_xml(self, indent)
  }
}

fn element_to_xml(elem: &MacElement, indent: usize) -> String {
  let children = get_children_safely(&elem.element);
  let pad = "  ".repeat(indent);
  let aid = elem.automation_id();
  let mut xml = format!("{pad}<{}", elem.control_type());
  if !elem.cached_name.is_empty() {
    xml.push_str(&format!(" Name=\"{}\"", xml_escape(&elem.cached_name)));
  }
  if !aid.is_empty() {
    xml.push_str(&format!(" AutomationId=\"{}\"", xml_escape(&aid)));
  }
  if let Some(ref val) = elem.cached_value {
    xml.push_str(&format!(" Value=\"{}\"", xml_escape(val)));
  }
  xml.push_str(&format!(
    " Enabled=\"{}\" X=\"{}\" Y=\"{}\" Width=\"{}\" Height=\"{}\"",
    elem.cached_enabled,
    elem.x(),
    elem.y(),
    elem.width(),
    elem.height(),
  ));

  if children.is_empty() {
    xml.push_str(" />");
  } else {
    xml.push('>');
    xml.push('\n');
    for child in children {
      let child_elem = MacElement::new(child);
      xml.push_str(&element_to_xml(&child_elem, indent + 1));
      xml.push('\n');
    }
    xml.push_str(&format!("{pad}</{}>", elem.control_type()));
  }
  xml
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
    let (target_role, name_filter, id_filter, _) = parse_selector_part(&parts[0]);
    return find_all_matching(root, &target_role, &name_filter, &id_filter);
  }

  // Multi-part selector: walk intermediate parts, BFS for leaf
  let leaf_part = &parts[parts.len() - 1];
  let (leaf_role, leaf_name, leaf_id, _) = parse_selector_part(leaf_part);

  // Find all intermediate containers matching the path prefix
  let mut candidates = vec![root.clone()];
  for (i, part) in parts[..parts.len() - 1].iter().enumerate() {
    let (target_role, name_filter, id_filter, _) = parse_selector_part(part);
    let mut next = Vec::new();
    for parent in &candidates {
      let mut queue = std::collections::VecDeque::new();
      if i == 0 {
        queue.push_back(parent.clone());
      } else {
        for child in get_children_safely(parent) {
          queue.push_back(child);
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
        let id_ok = match &id_filter {
          Some(id) => {
            let aid = elem
              .attribute(&AXAttribute::identifier())
              .ok()
              .map(|s| s.to_string())
              .unwrap_or_default();
            aid.to_lowercase().contains(&id.to_lowercase())
          }
          None => true,
        };
        if matches && name_ok && id_ok {
          next.push(elem.clone());
        } else {
          // Keep searching deeper for intermediate parts
          for child in get_children_safely(&elem) {
            queue.push_back(child);
          }
        }
      }
    }
    candidates = next;
  }

  // BFS for leaf part within each candidate container
  let mut results = Vec::new();
  for container in &candidates {
    results.extend(find_all_matching(container, &leaf_role, &leaf_name, &leaf_id));
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
fn find_all_matching(
  root: &AXUIElement,
  target_role: &str,
  name_filter: &Option<String>,
  id_filter: &Option<String>,
) -> Vec<MacElement> {
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
    let matches_id = match id_filter {
      Some(id) => {
        // Check both AXIdentifier and AXRoleDescription for @id and @Class filters
        let aid = elem
          .attribute(&AXAttribute::identifier())
          .ok()
          .map(|s| s.to_string())
          .unwrap_or_default();
        let role_desc = elem
          .attribute(&AXAttribute::role_description())
          .ok()
          .map(|s| s.to_string())
          .unwrap_or_default();
        aid.to_lowercase().contains(&id.to_lowercase())
          || role_desc.to_lowercase().contains(&id.to_lowercase())
      }
      None => true,
    };

    if matches_role && matches_name && matches_id {
      results.push(MacElement::new(elem.clone()));
    }

    for child in get_children_safely(&elem) {
      queue.push_back(child);
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

fn parse_selector_part(part: &str) -> (String, Option<String>, Option<String>, Option<usize>) {
  // Parse "Role[@name=\"value\"]" or "Role[@id=\"value\"]" or "Role[@Class=\"value\"]" or "Role[index]"
  let (role, rest) = if let Some(bracket_pos) = part.find('[') {
    (&part[..bracket_pos], Some(&part[bracket_pos..]))
  } else {
    (part.as_ref(), None)
  };

  let role = if role == "*" { "*" } else { role };

  let mut name_filter = None;
  let mut id_filter = None;
  let mut index_filter = None;

  if let Some(rest) = rest {
    let inner = rest.trim_start_matches('[').trim_end_matches(']');
    if inner.starts_with("@id=\"") || inner.starts_with("@id='") {
      let value = inner[5..].trim_end_matches('"').trim_end_matches('\'');
      id_filter = Some(value.to_string());
    } else if inner.starts_with("@Class=\"") || inner.starts_with("@Class='") {
      // @Class="value" — match by class name (role description)
      let value = inner[8..].trim_end_matches('"').trim_end_matches('\'');
      id_filter = Some(value.to_string());
    } else if let Some(eq_pos) = inner.find("=\"") {
      // @name="value" or @name='value'
      let value = inner[eq_pos + 2..].trim_end_matches('"').trim_end_matches('\'');
      name_filter = Some(value.to_string());
    } else if inner.chars().all(|c| c.is_ascii_digit()) {
      index_filter = inner.parse().ok();
    }
  }

  (role.to_string(), name_filter, id_filter, index_filter)
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

pub fn get_page_source_verbose(handle: &WindowHandle) -> Result<String> {
  let app = app_for_window_handle(handle)?;
  Ok(dump_element_tree(&app, 0))
}

/// Recursively dump the full attribute set of every element in the tree.
fn dump_element_tree(element: &AXUIElement, indent: usize) -> String {
  let pad = "  ".repeat(indent);
  let mut out = String::new();
  out.push_str(&format!("{}{}\n", pad, dump_attributes(element)));
  for child in get_children_safely(element) {
    out.push_str(&dump_element_tree(&child, indent + 1));
  }
  out
}

/// Probe the element at a screen position and dump all its attributes.
pub fn probe_at_position(x: i32, y: i32) -> Result<String> {
  use accessibility_sys::{AXUIElementCopyElementAtPosition, kAXErrorSuccess};

  let system_wide = AXUIElement::system_wide();
  let mut elem_ref: *mut accessibility_sys::__AXUIElement = std::ptr::null_mut();
  let err = unsafe {
    AXUIElementCopyElementAtPosition(
      system_wide.as_concrete_TypeRef(),
      x as f32,
      y as f32,
      &mut elem_ref,
    )
  };
  if err != kAXErrorSuccess || elem_ref.is_null() {
    return Ok(format!("No element found at ({}, {}), AXError: {}", x, y, err));
  }
  let elem: AXUIElement = unsafe { TCFType::wrap_under_create_rule(elem_ref) };
  Ok(dump_attributes(&elem))
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
