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

    let cached_value = element.attribute(&AXAttribute::value()).ok().and_then(|v| {
      if v.instance_of::<core_foundation::string::CFString>() {
        let s: core_foundation::string::CFString =
          unsafe { core_foundation::base::TCFType::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
        Some(s.to_string())
      } else if v.instance_of::<core_foundation::number::CFNumber>() {
        // BUG-17: handle CFNumber values (sliders, progress bars, etc.)
        let n: core_foundation::number::CFNumber =
          unsafe { core_foundation::base::TCFType::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
        n.to_i64()
          .map(|i| i.to_string())
          .or_else(|| n.to_f64().map(|f| f.to_string()))
      } else if v.instance_of::<core_foundation::boolean::CFBoolean>() {
        // BUG-17: handle CFBoolean values (checkboxes, toggles, etc.)
        let b: core_foundation::boolean::CFBoolean =
          unsafe { core_foundation::base::TCFType::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
        Some(bool::from(b).to_string())
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
    if ok { Some((point.x, point.y)) } else { None }
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
    if ok { Some((size.width, size.height)) } else { None }
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
/// Handles both CFArray returns and single-element returns (BUG-11).
fn get_children_via_ffi(element: &AXUIElement, attr_name: &str) -> Vec<AXUIElement> {
  use accessibility_sys::{AXUIElementCopyAttributeValue, AXUIElementGetTypeID, kAXErrorSuccess};
  use core_foundation::array::CFArray;
  use core_foundation::base::CFGetTypeID;

  let elem_ref = element.as_concrete_TypeRef();
  let cf_attr = cfstr(attr_name);
  let mut value: core_foundation::base::CFTypeRef = std::ptr::null_mut();

  let err = unsafe { AXUIElementCopyAttributeValue(elem_ref, cf_attr.as_concrete_TypeRef(), &mut value) };
  if err != kAXErrorSuccess || value.is_null() {
    return Vec::new();
  }

  let ax_type_id = unsafe { AXUIElementGetTypeID() };
  let arr_type_id = unsafe { core_foundation::array::CFArray::<core_foundation::base::CFType>::type_id() };

  let value_type = unsafe { CFGetTypeID(value as *const _) };

  // BUG-11: Distinguish between single AXUIElement and CFArray of elements.
  if value_type == ax_type_id {
    // Single element — wrap it and return as a one-element Vec.
    // We own the value (Create Rule), so use wrap_under_create_rule.
    let elem: AXUIElement = unsafe { TCFType::wrap_under_create_rule(value as accessibility_sys::AXUIElementRef) };
    return vec![elem];
  }

  if value_type != arr_type_id {
    // Not an array, not an AXUIElement — release and skip.
    unsafe { core_foundation::base::CFRelease(value as *const _) };
    return Vec::new();
  }

  // It's a CFArray — wrap with create rule (we own it).
  let cf_array: CFArray<core_foundation::base::CFType> = unsafe { TCFType::wrap_under_create_rule(value as *const _) };

  let count = cf_array.len();
  if count == 0 {
    return Vec::new();
  }

  let mut children = Vec::with_capacity(count as usize);

  for i in 0..count {
    if let Some(item) = cf_array.get(i) {
      let item_ref = item.as_CFTypeRef();
      if item_ref.is_null() {
        continue;
      }
      let item_type = unsafe { CFGetTypeID(item_ref as *const _) };
      if item_type == ax_type_id {
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
          out.push_str(&format!(
            "  \"{}\": \"{}\",\n",
            name,
            s.to_string().replace('"', "\\\"")
          ));
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
          use accessibility_sys::{AXValueGetValue, kAXValueTypeCGPoint, kAXValueTypeCGRect, kAXValueTypeCGSize};
          use core_graphics::geometry::{CGPoint, CGRect, CGSize};

          let mut rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
              width: 0.0,
              height: 0.0,
            },
          };
          let ok = unsafe {
            AXValueGetValue(
              val.as_CFTypeRef() as *mut _,
              kAXValueTypeCGRect,
              &mut rect as *mut _ as *mut _,
            )
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
              AXValueGetValue(
                val.as_CFTypeRef() as *mut _,
                kAXValueTypeCGPoint,
                &mut point as *mut _ as *mut _,
              )
            };
            if ok {
              out.push_str(&format!("  \"{}\": [{},{}],\n", name, point.x, point.y));
            } else {
              // Try size
              let mut size = CGSize {
                width: 0.0,
                height: 0.0,
              };
              let ok = unsafe {
                AXValueGetValue(
                  val.as_CFTypeRef() as *mut _,
                  kAXValueTypeCGSize,
                  &mut size as *mut _ as *mut _,
                )
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
    // Surface the most useful auxiliary text for a "blind" agent picking
    // elements via CLI: AXDescription (label) > AXHelp (tooltip) >
    // AXPlaceholderValue (input hint). Concatenated when several exist.
    let desc = self
      .element
      .attribute(&AXAttribute::description())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    let help = self
      .element
      .attribute(&AXAttribute::help())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();
    let placeholder = self
      .element
      .attribute(&AXAttribute::new(&cfstr("AXPlaceholderValue")))
      .ok()
      .and_then(|v: core_foundation::base::CFType| {
        v.downcast::<core_foundation::string::CFString>().map(|s| s.to_string())
      })
      .unwrap_or_default();
    let mut parts: Vec<&str> = vec![];
    if !desc.is_empty() {
      parts.push(&desc);
    }
    if !help.is_empty() && help != desc {
      parts.push(&help);
    }
    if !placeholder.is_empty() && placeholder != desc && placeholder != help {
      parts.push(&placeholder);
    }
    parts.join(" | ")
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

  fn pos(&self, window: Option<&WindowHandle>) -> Result<ElementPosition> {
    let (px, py) = self.cached_position.unwrap_or((0.0, 0.0));
    let (pw, ph) = self.cached_size.unwrap_or((0.0, 0.0));

    // BUG-15: Fill window_width/height and compute relative coords using window origin.
    let (wx, wy, ww, wh) = if let Some(handle) = window {
      crate::platform::macos::window::list_windows()
        .ok()
        .and_then(|wins| wins.into_iter().find(|w| w.hwnd == handle.0))
        .map(|w| (w.x, w.y, w.width, w.height))
        .unwrap_or((0, 0, 0, 0))
    } else {
      (0, 0, 0, 0)
    };

    Ok(ElementPosition {
      left: px as i32,
      top: py as i32,
      right: (px + pw) as i32,
      bottom: (py + ph) as i32,
      center_x: (px + pw / 2.0) as i32,
      center_y: (py + ph / 2.0) as i32,
      relative_center_x: (px + pw / 2.0) as i32 - wx,
      relative_center_y: (py + ph / 2.0) as i32 - wy,
      window_width: ww,
      window_height: wh,
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
    self.element.raise().map_err(|e| anyhow::anyhow!("focus failed: {e:?}"))
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
          if let Ok(click_down) =
            CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
          {
            click_down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));

            if let Ok(click_up) =
              CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseUp, point, CGMouseButton::Left)
            {
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
    let (target_role, name_filter, id_filter, index_filter) = parse_selector_part(&parts[0]);
    let mut results = find_all_matching(root, &target_role, &name_filter, &id_filter);
    // BUG-04: Apply 1-based index filter (XPath[1] = first)
    if let Some(idx) = index_filter {
      let i = if idx > 0 { idx - 1 } else { 0 };
      results = results.into_iter().skip(i).take(1).collect();
    }
    return results;
  }

  // Multi-part selector: walk intermediate parts, BFS for leaf
  let leaf_part = &parts[parts.len() - 1];
  let (leaf_role, leaf_name, leaf_id, leaf_index) = parse_selector_part(leaf_part);

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
        let matches = target_role == "*" || role == target_role || ct.eq_ignore_ascii_case(&target_role);
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
          for child in get_children_safely(&elem) {
            queue.push_back(child);
          }
        }
      }
    }
    candidates = next;
  }

  let mut results = Vec::new();
  for container in &candidates {
    results.extend(find_all_matching(container, &leaf_role, &leaf_name, &leaf_id));
  }
  // BUG-04: Apply 1-based index to leaf part
  if let Some(idx) = leaf_index {
    let i = if idx > 0 { idx - 1 } else { 0 };
    results = results.into_iter().skip(i).take(1).collect();
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
    let matches_role = target_role == "*" || role == target_role || control_type.eq_ignore_ascii_case(target_role);
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
        aid.to_lowercase().contains(&id.to_lowercase()) || role_desc.to_lowercase().contains(&id.to_lowercase())
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

/// Render a hierarchical tree of the elements under `scope`, applying the same
/// filters and hit-test discovery used by `query_elements`. When a filter is
/// present, the tree is pruned to only show paths leading to a matching node.
pub fn render_tree(scope: &ElementScope, query: &ElementQuery) -> Result<String> {
  ensure_ax_trusted()?;
  let (root, extras) = scope_to_roots(scope, query.no_hit_test)?;
  let max_depth = query.max_depth.unwrap_or(usize::MAX);
  let has_filter = query.role.is_some() || query.text.is_some();
  let mut out = String::new();
  // Main root
  render_tree_node(&root, query, 0, max_depth, has_filter, &mut out);
  // Extra detached roots discovered via hit-test. Only emit the marker comment
  // when the subtree actually produces output (otherwise we'd dump empty
  // <!-- detached subtree #N --> markers when filters prune everything).
  let mut emitted_extras = 0usize;
  for extra in &extras {
    let mut buf = String::new();
    if render_tree_node(extra, query, 0, max_depth, has_filter, &mut buf) {
      emitted_extras += 1;
      out.push_str(&format!("<!-- detached subtree #{} -->\n", emitted_extras));
      out.push_str(&buf);
    }
  }
  Ok(out)
}

/// Recursive helper for `render_tree`. Returns true if this subtree contains
/// any visible node (after filter+hidden pruning) — used by the parent to
/// decide whether to emit itself when a filter is active.
fn render_tree_node(
  elem: &AXUIElement,
  query: &ElementQuery,
  depth: usize,
  max_depth: usize,
  has_filter: bool,
  out: &mut String,
) -> bool {
  if depth > max_depth {
    return false;
  }
  let mac = MacElement::new(elem.clone());
  let visible = query.include_hidden || (mac.x() >= 0 && mac.y() >= 0 && mac.width() > 0 && mac.height() > 0);
  let matches_self = visible && matches_filter(elem, query);

  // Build children output first so we know if any descendant matches.
  let mut child_out = String::new();
  let mut any_child_match = false;
  if depth < max_depth {
    for child in get_children_safely(elem) {
      if render_tree_node(&child, query, depth + 1, max_depth, has_filter, &mut child_out) {
        any_child_match = true;
      }
    }
  }

  // Decide whether to emit this node.
  // Tree should appear iff this node or any descendant is "interesting":
  // - filtered mode: interesting = matches_filter
  // - unfiltered mode: interesting = visible (on-screen, nonzero size)
  let emit = if has_filter {
    matches_self || any_child_match
  } else {
    visible || any_child_match
  };
  if !emit {
    return false;
  }

  let pad = "  ".repeat(depth);
  let aid = mac.automation_id();
  out.push_str(&format!("{pad}<{}", mac.control_type()));
  if !mac.cached_name.is_empty() {
    out.push_str(&format!(" Name=\"{}\"", xml_escape(&mac.cached_name)));
  }
  if !aid.is_empty() {
    out.push_str(&format!(" AutomationId=\"{}\"", xml_escape(&aid)));
  }
  if let Some(ref val) = mac.cached_value {
    if !val.is_empty() {
      out.push_str(&format!(" Value=\"{}\"", xml_escape(val)));
    }
  }
  out.push_str(&format!(
    " X=\"{}\" Y=\"{}\" Width=\"{}\" Height=\"{}\"",
    mac.x(),
    mac.y(),
    mac.width(),
    mac.height(),
  ));
  if child_out.is_empty() {
    out.push_str(" />\n");
  } else {
    out.push_str(">\n");
    out.push_str(&child_out);
    out.push_str(&format!("{pad}</{}>\n", mac.control_type()));
  }
  matches_self || any_child_match
}

fn matches_filter(elem: &AXUIElement, query: &ElementQuery) -> bool {
  let role: String = elem
    .attribute(&AXAttribute::role())
    .ok()
    .map(|s: core_foundation::string::CFString| s.to_string())
    .unwrap_or_default();
  if let Some(ref r) = query.role {
    let role_desc: String = elem
      .attribute(&AXAttribute::role_description())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    // Multi-role: agent can pass `--role Button,Edit` to match either.
    let wanted: Vec<&str> = r
      .split(|c| c == ',' || c == '|')
      .map(|s| s.trim())
      .filter(|s| !s.is_empty())
      .collect();
    let any_match = wanted
      .iter()
      .any(|w| matches_query_role(&role, w) || (!role_desc.is_empty() && matches_role_description(&role_desc, w)));
    if !any_match {
      return false;
    }
  }
  if let Some(ref t) = query.text {
    let title: String = elem
      .attribute(&AXAttribute::title())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    let value = get_ax_value_as_string(elem);
    let desc: String = elem
      .attribute(&AXAttribute::description())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    let help: String = elem
      .attribute(&AXAttribute::help())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    let ident: String = elem
      .attribute(&AXAttribute::identifier())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    let placeholder: String = elem
      .attribute(&AXAttribute::new(&cfstr("AXPlaceholderValue")))
      .ok()
      .and_then(|v: core_foundation::base::CFType| {
        v.downcast::<core_foundation::string::CFString>().map(|s| s.to_string())
      })
      .unwrap_or_default();
    let ok = if query.text_exact {
      [&title, &value, &desc, &help, &ident, &placeholder]
        .iter()
        .any(|h| h.as_str() == t.as_str())
    } else {
      let tl = t.to_lowercase();
      title.to_lowercase().contains(&tl)
        || value.to_lowercase().contains(&tl)
        || desc.to_lowercase().contains(&tl)
        || help.to_lowercase().contains(&tl)
        || ident.to_lowercase().contains(&tl)
        || placeholder.to_lowercase().contains(&tl)
    };
    if !ok {
      return false;
    }
  }
  true
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
  let err =
    unsafe { AXUIElementCopyElementAtPosition(system_wide.as_concrete_TypeRef(), x as f32, y as f32, &mut elem_ref) };
  if err != kAXErrorSuccess || elem_ref.is_null() {
    return Ok(format!("No element found at ({}, {}), AXError: {}", x, y, err));
  }
  let elem: AXUIElement = unsafe { TCFType::wrap_under_create_rule(elem_ref) };
  Ok(dump_attributes(&elem))
}

/// Get the AXWindow element for the window handle (BUG-09: scope to window, not whole app).
fn app_for_window_handle(handle: &WindowHandle) -> Result<AXUIElement> {
  let wins = crate::platform::macos::window::list_windows()?;
  let win = wins
    .iter()
    .find(|w| w.hwnd == handle.0)
    .ok_or_else(|| anyhow::anyhow!("window not found: {}", handle.0))?;
  let pid = win.process_id;
  let hwnd = win.hwnd;
  // BUG-09: Return the specific AXWindow (scoped to this window), not the app root
  crate::platform::macos::window::ax_window_for_id(pid, hwnd as i64)
}

/// Ensure the Accessibility API is trusted for this process (BUG-13).
/// Returns an error if not trusted, prompting the user to grant access.
fn ensure_ax_trusted() -> Result<()> {
  use accessibility_sys::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
  use core_foundation::base::TCFType;
  use core_foundation::boolean::CFBoolean;
  use core_foundation::dictionary::CFMutableDictionary;
  use core_foundation::string::CFString;

  let key: CFString = unsafe { TCFType::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
  let val = CFBoolean::true_value();
  let mut opts: CFMutableDictionary<CFString, CFBoolean> = CFMutableDictionary::new();
  opts.add(&key, &val);
  let trusted = unsafe { AXIsProcessTrustedWithOptions(opts.as_CFTypeRef() as *const _) };
  if trusted {
    Ok(())
  } else {
    Err(anyhow::anyhow!(crate::error::PlatformError::AxPermissionDenied))
  }
}

/// Map abstract role names to one or more AX role strings.
fn abstract_role_to_ax_roles(role: &str) -> Vec<&'static str> {
  match role {
    "Button" => vec![
      "AXButton",
      "AXToolbarButton",
      "AXMenuButton",
      "AXDisclosureTriangle",
      "AXCloseButton",
      "AXZoomButton",
      "AXMinimizeButton",
      "AXFullScreenButton",
    ],
    "Edit" | "TextField" | "TextBox" => vec!["AXTextField", "AXTextArea"],
    "Text" | "Label" | "StaticText" => vec!["AXStaticText"],
    "CheckBox" => vec!["AXCheckBox"],
    "RadioButton" => vec!["AXRadioButton"],
    "ComboBox" => vec!["AXComboBox"],
    "List" => vec!["AXList", "AXTable"],
    "ListItem" | "Row" => vec!["AXRow"],
    "MenuItem" => vec!["AXMenuItem"],
    "Menu" => vec!["AXMenu"],
    "MenuBar" => vec!["AXMenuBar"],
    "Slider" => vec!["AXSlider"],
    "ProgressBar" => vec!["AXProgressIndicator"],
    "Image" => vec!["AXImage"],
    "WebView" | "Browser" => vec!["AXWebArea"],
    "Toolbar" => vec!["AXToolbar"],
    "Tab" => vec!["AXTab"],
    "TabBar" | "TabGroup" => vec!["AXTabGroup"],
    "Group" => vec!["AXGroup"],
    "Window" => vec!["AXWindow"],
    "Dialog" => vec!["AXSheet", "AXDialog"],
    _ => vec![],
  }
}

/// Check whether an AX role matches the abstract role from an ElementQuery.
fn matches_query_role(ax_role: &str, query_role: &str) -> bool {
  let ax_roles = abstract_role_to_ax_roles(query_role);
  if ax_roles.is_empty() {
    // Unknown abstract role — fall back to direct or control-type match
    return ax_role.eq_ignore_ascii_case(query_role) || role_to_control_type(ax_role).eq_ignore_ascii_case(query_role);
  }
  ax_roles.contains(&ax_role)
}

/// Match abstract role against AXRoleDescription strings (mainly Chinese/English).
/// Used as a fallback when AXRole is AXUnknown (common in Qt/Electron apps).
fn matches_role_description(role_desc: &str, query_role: &str) -> bool {
  let desc = role_desc.to_lowercase();
  let candidates: &[&str] = match query_role {
    "Button" => &["button", "按钮", "ボタン", "버튼"],
    "Edit" | "TextField" | "TextBox" => &[
      "text field",
      "search field",
      "text area",
      "textbox",
      "文本栏",
      "搜索文本栏",
      "文本字段",
      "搜索字段",
      "输入框",
      "文本框",
    ],
    "Text" | "Label" | "StaticText" => &["static text", "label", "文本", "标签"],
    "CheckBox" => &["check box", "checkbox", "复选框"],
    "RadioButton" => &["radio button", "单选按钮"],
    "ComboBox" => &["combo box", "pop up button", "下拉框", "弹出按钮"],
    "List" => &["list", "table", "列表", "表格"],
    "ListItem" | "Row" => &["row", "list item", "行", "列表项"],
    "MenuItem" => &["menu item", "菜单项", "菜单栏项"],
    "Image" => &["image", "图像", "图片"],
    "Link" => &["link", "链接"],
    "Tab" => &["tab", "标签页"],
    "Group" => &["group", "组"],
    _ => &[],
  };
  candidates.iter().any(|c| desc.contains(c))
}

/// BFS-traverse an AXUIElement tree, applying role + text filters from ElementQuery.
fn bfs_query(root: &AXUIElement, query: &ElementQuery, max_depth: usize) -> Vec<MacElement> {
  bfs_query_with_extra(root, query, max_depth, &[])
}

/// Like bfs_query but also walks additional roots discovered via hit-test grid scan
/// (for apps that expose elements via independent NSAccessibilityElement roots, not
/// the main window's AXChildren tree — e.g. QQ Music search results panel).
fn bfs_query_with_extra(
  root: &AXUIElement,
  query: &ElementQuery,
  max_depth: usize,
  extra_roots: &[AXUIElement],
) -> Vec<MacElement> {
  use std::collections::VecDeque;

  let mut results = Vec::new();
  // Dedup visited elements by CFHashCode (AXUIElement implements CFEqual/CFHash).
  let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
  let mut queue: VecDeque<(AXUIElement, usize)> = VecDeque::new();

  let mut push =
    |q: &mut VecDeque<(AXUIElement, usize)>, v: &mut std::collections::HashSet<usize>, e: AXUIElement, d: usize| {
      let key = ax_identity_key(&e);
      if v.insert(key) {
        q.push_back((e, d));
      }
    };

  push(&mut queue, &mut visited, root.clone(), 0);
  for e in extra_roots {
    push(&mut queue, &mut visited, e.clone(), 0);
  }

  while let Some((elem, depth)) = queue.pop_front() {
    let role = elem
      .attribute(&AXAttribute::role())
      .ok()
      .map(|s| s.to_string())
      .unwrap_or_default();

    let role_ok = match &query.role {
      Some(r) => {
        let wanted: Vec<&str> = r
          .split(|c| c == ',' || c == '|')
          .map(|s| s.trim())
          .filter(|s| !s.is_empty())
          .collect();
        let role_desc: String = elem
          .attribute(&AXAttribute::role_description())
          .ok()
          .map(|s: core_foundation::string::CFString| s.to_string())
          .unwrap_or_default();
        wanted
          .iter()
          .any(|w| matches_query_role(&role, w) || (!role_desc.is_empty() && matches_role_description(&role_desc, w)))
      }
      None => true,
    };

    if role_ok {
      // Check text match against title, value, description, help, identifier
      let title = elem
        .attribute(&AXAttribute::title())
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();
      let value = get_ax_value_as_string(&elem);
      let desc = elem
        .attribute(&AXAttribute::description())
        .ok()
        .map(|s: core_foundation::string::CFString| s.to_string())
        .unwrap_or_default();
      let help = elem
        .attribute(&AXAttribute::help())
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();
      let ident = elem
        .attribute(&AXAttribute::identifier())
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();

      let placeholder = elem
        .attribute(&AXAttribute::new(&cfstr("AXPlaceholderValue")))
        .ok()
        .and_then(|v: core_foundation::base::CFType| {
          v.downcast::<core_foundation::string::CFString>().map(|s| s.to_string())
        })
        .unwrap_or_default();

      let text_ok = match &query.text {
        Some(t) => {
          let tl = t.to_lowercase();
          title.to_lowercase().contains(&tl)
            || value.to_lowercase().contains(&tl)
            || desc.to_lowercase().contains(&tl)
            || help.to_lowercase().contains(&tl)
            || ident.to_lowercase().contains(&tl)
            || placeholder.to_lowercase().contains(&tl)
        }
        None => true,
      };

      if text_ok {
        results.push(MacElement::new(elem.clone()));
      }
    }

    if depth < max_depth {
      for child in get_children_safely(&elem) {
        let key = ax_identity_key(&child);
        if visited.insert(key) {
          queue.push_back((child, depth + 1));
        }
      }
    }
  }

  results
}

/// Identity key for AX dedup. AXUIElement implements CFEqual/CFHash, so the
/// CoreFoundation hash code uniquely identifies the referenced UI element.
fn ax_identity_key(e: &AXUIElement) -> usize {
  use core_foundation::base::CFHash;
  unsafe { CFHash(e.as_concrete_TypeRef() as *const _) as usize }
}

/// Hit-test the system AX at a screen position. Returns the deepest element under (x, y),
/// or None if AX reports no element.
fn hit_test_at(x: f32, y: f32) -> Option<AXUIElement> {
  use accessibility_sys::{AXUIElementCopyElementAtPosition, kAXErrorSuccess};
  let sys = AXUIElement::system_wide();
  let mut out: *mut accessibility_sys::__AXUIElement = std::ptr::null_mut();
  let err = unsafe { AXUIElementCopyElementAtPosition(sys.as_concrete_TypeRef(), x, y, &mut out) };
  if err != kAXErrorSuccess || out.is_null() {
    return None;
  }
  Some(unsafe { TCFType::wrap_under_create_rule(out) })
}

/// Walk the AXParent chain until reaching an element whose parent is None, the same
/// element as `app_root`, or an AXApplication. Returns the topmost meaningful root.
/// Walk up from `elem` and return the highest ancestor that is NOT in `reachable`.
/// If `elem` itself is reachable, returns None. Otherwise returns the deepest unreachable
/// ancestor (its parent IS reachable, or it has no parent / is a window/app).
fn walk_up_until_reachable(elem: AXUIElement, reachable: &std::collections::HashSet<usize>) -> Option<AXUIElement> {
  if reachable.contains(&ax_identity_key(&elem)) {
    return None;
  }
  let mut cur = elem;
  for _ in 0..64 {
    let role = cur
      .attribute(&AXAttribute::role())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    if role == "AXApplication" || role == "AXWindow" {
      return Some(cur);
    }
    match cur.attribute(&AXAttribute::parent()).ok() {
      Some(p) => {
        if reachable.contains(&ax_identity_key(&p)) {
          return Some(cur);
        }
        cur = p;
      }
      None => return Some(cur),
    }
  }
  Some(cur)
}

#[allow(dead_code)]
fn walk_up_to_root(elem: AXUIElement, app_root_key: usize) -> AXUIElement {
  let mut cur = elem;
  for _ in 0..64 {
    let role = cur
      .attribute(&AXAttribute::role())
      .ok()
      .map(|s: core_foundation::string::CFString| s.to_string())
      .unwrap_or_default();
    if role == "AXApplication" || role == "AXWindow" {
      return cur;
    }
    let parent = cur.attribute(&AXAttribute::parent()).ok();
    match parent {
      Some(p) => {
        if ax_identity_key(&p) == app_root_key {
          return cur;
        }
        cur = p;
      }
      None => return cur,
    }
  }
  cur
}

/// Scan the window rect with hit-tests on a coarse grid (default 48px) to discover
/// AX roots that are NOT reachable from the main window's AXChildren tree.
/// Returns deduplicated additional roots (walked up just below the AXWindow/AXApplication).
fn discover_hidden_roots(app_root: &AXUIElement, window_rect: (f64, f64, f64, f64), step: i32) -> Vec<AXUIElement> {
  let (wx, wy, ww, wh) = window_rect;
  let app_key = ax_identity_key(app_root);

  // Pre-collect all elements reachable from app_root for dedup vs already-visited.
  let mut reachable: std::collections::HashSet<usize> = std::collections::HashSet::new();
  reachable.insert(app_key);
  let mut q = std::collections::VecDeque::new();
  q.push_back(app_root.clone());
  let mut budget = 5000usize;
  while let Some(e) = q.pop_front() {
    if budget == 0 {
      break;
    }
    budget -= 1;
    for c in get_children_safely(&e) {
      let k = ax_identity_key(&c);
      if reachable.insert(k) {
        q.push_back(c);
      }
    }
  }

  let mut extras: Vec<AXUIElement> = Vec::new();
  let mut extra_keys: std::collections::HashSet<usize> = std::collections::HashSet::new();

  let x0 = wx as i32;
  let y0 = wy as i32;
  let x1 = (wx + ww) as i32;
  let y1 = (wy + wh) as i32;
  let mut y = y0;
  while y < y1 {
    let mut x = x0;
    while x < x1 {
      if let Some(hit) = hit_test_at(x as f32, y as f32) {
        let hit_key = ax_identity_key(&hit);
        if !reachable.contains(&hit_key) && !extra_keys.contains(&hit_key) {
          if let Some(top) = walk_up_until_reachable(hit, &reachable) {
            let top_key = ax_identity_key(&top);
            // Skip AXWindow/AXApplication roots — they're the parent of everything
            // and would re-walk the entire reachable subtree.
            let role = top
              .attribute(&AXAttribute::role())
              .ok()
              .map(|s: core_foundation::string::CFString| s.to_string())
              .unwrap_or_default();
            if role != "AXWindow" && role != "AXApplication" && extra_keys.insert(top_key) {
              extras.push(top);
            }
          }
        }
      }
      x += step;
    }
    y += step;
  }

  extras
}

/// Compute the screen rect of the given AX element (Position+Size). Returns None on failure.
fn ax_screen_rect(elem: &AXUIElement) -> Option<(f64, f64, f64, f64)> {
  use accessibility_sys::{AXValueGetValue, kAXValueTypeCGPoint, kAXValueTypeCGSize};
  use core_graphics::geometry::{CGPoint, CGSize};
  let pos_v = elem.attribute(&AXAttribute::new(&cfstr("AXPosition"))).ok()?;
  let sz_v = elem.attribute(&AXAttribute::new(&cfstr("AXSize"))).ok()?;
  let mut p = CGPoint { x: 0.0, y: 0.0 };
  let mut s = CGSize {
    width: 0.0,
    height: 0.0,
  };
  let ok_p = unsafe {
    AXValueGetValue(
      pos_v.as_CFTypeRef() as *mut _,
      kAXValueTypeCGPoint,
      &mut p as *mut _ as *mut _,
    )
  };
  let ok_s = unsafe {
    AXValueGetValue(
      sz_v.as_CFTypeRef() as *mut _,
      kAXValueTypeCGSize,
      &mut s as *mut _ as *mut _,
    )
  };
  if !ok_p || !ok_s || s.width <= 0.0 || s.height <= 0.0 {
    return None;
  }
  Some((p.x, p.y, s.width, s.height))
}

/// Resolve an ElementScope to the root AXUIElement to search from.
fn scope_to_root(scope: &ElementScope) -> Result<AXUIElement> {
  match scope {
    ElementScope::Window(handle) => app_for_window_handle(handle),
    ElementScope::Application(pid) => Ok(AXUIElement::application(*pid as i32)),
    ElementScope::Foreground => {
      // Get frontmost app's PID via NSWorkspace
      use objc2_app_kit::NSWorkspace;
      let ws = unsafe { NSWorkspace::sharedWorkspace() };
      let front = unsafe { ws.frontmostApplication() };
      let pid = front.map(|app| unsafe { app.processIdentifier() }).unwrap_or(0);
      if pid == 0 {
        Ok(AXUIElement::system_wide())
      } else {
        Ok(AXUIElement::application(pid))
      }
    }
  }
}

/// Returns (root, extra_roots) — extra_roots are AX subtrees discovered via hit-test
/// grid scan inside the scope's window bounds (catches QQ Music-style floating panels
/// that aren't in the window's AXChildren).
fn scope_to_roots(scope: &ElementScope, no_hit_test: bool) -> Result<(AXUIElement, Vec<AXUIElement>)> {
  let root = scope_to_root(scope)?;
  if no_hit_test {
    return Ok((root, Vec::new()));
  }
  let rect: Option<(f64, f64, f64, f64)> = match scope {
    ElementScope::Window(handle) => {
      let wins = crate::platform::macos::window::list_windows()?;
      wins
        .iter()
        .find(|w| w.hwnd == handle.0)
        .map(|w| (w.x as f64, w.y as f64, w.width as f64, w.height as f64))
    }
    ElementScope::Application(_) | ElementScope::Foreground => ax_screen_rect(&root).or_else(|| {
      // Fall back to the focused window's rect.
      let focused: AXUIElement = root.attribute(&AXAttribute::focused_window()).ok()?;
      ax_screen_rect(&focused)
    }),
  };
  let extras = match rect {
    Some(r) if r.2 > 100.0 && r.3 > 100.0 => discover_hidden_roots(&root, r, 48),
    _ => Vec::new(),
  };
  Ok((root, extras))
}

/// Public query_elements: BFS search scoped to a window (or desktop).
pub fn query_elements(scope: &ElementScope, query: &ElementQuery) -> Result<Vec<Box<dyn Element>>> {
  ensure_ax_trusted()?;
  let (root, extras) = scope_to_roots(scope, query.no_hit_test)?;
  let max_depth = query.max_depth.unwrap_or(usize::MAX);
  let mut results = bfs_query_with_extra(&root, query, max_depth, &extras);
  if !query.include_hidden {
    results.retain(|e| e.x() >= 0 && e.y() >= 0 && e.width() > 0 && e.height() > 0);
  }
  Ok(results.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
}

/// Public query_one: like query_elements but enforces uniqueness.
pub fn query_one(scope: &ElementScope, query: &ElementQuery) -> Result<Box<dyn Element>> {
  ensure_ax_trusted()?;
  let (root, extras) = scope_to_roots(scope, query.no_hit_test)?;
  let max_depth = query.max_depth.unwrap_or(usize::MAX);
  let mut results = bfs_query_with_extra(&root, query, max_depth, &extras);
  if !query.include_hidden {
    results.retain(|e| e.x() >= 0 && e.y() >= 0 && e.width() > 0 && e.height() > 0);
  }

  match results.len() {
    0 => Err(anyhow::anyhow!(crate::error::PlatformError::ElementNotFound(format!(
      "{query:?}"
    )))),
    1 => Ok(Box::new(results.remove(0))),
    n => {
      if let Some(idx) = query.index {
        results
          .into_iter()
          .nth(idx)
          .map(|e| Box::new(e) as Box<dyn Element>)
          .ok_or_else(|| anyhow::anyhow!("element index {idx} out of range (found {n})"))
      } else {
        Err(anyhow::anyhow!(crate::error::PlatformError::AmbiguousMatch(n)))
      }
    }
  }
}

/// Public find_by_xpath: delegates to find_elements_by_selector (BUG-04 already fixed there).
pub fn find_by_xpath(scope: &ElementScope, xpath: &str) -> Result<Vec<Box<dyn Element>>> {
  ensure_ax_trusted()?;
  let root = scope_to_root(scope)?;
  let results = find_elements_by_selector(&root, xpath);
  Ok(results.into_iter().map(|e| Box::new(e) as Box<dyn Element>).collect())
}
