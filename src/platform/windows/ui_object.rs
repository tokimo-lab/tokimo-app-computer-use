use crate::platform::Element;
use crate::platform::windows::elements::utils::get_control_type_name;
use crate::platform::windows::keyboard;
use crate::platform::windows::wnd::get_wnd_rect;
use crate::types::ElementPosition;
use anyhow::{Context, Result, anyhow};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::core::Interface;

#[derive(Debug)]
pub struct WindowsElement {
  pub depth: Option<i32>,
  pub selector: String,
  pub element: IUIAutomationElement,
  pub stable_id: String,
}

unsafe impl Send for WindowsElement {}

impl WindowsElement {
  pub fn automation_id_str(&self) -> String {
    unsafe { self.element.CurrentAutomationId().unwrap_or_default().to_string() }
  }
  pub fn name_str(&self) -> String {
    unsafe { self.element.CurrentName().unwrap_or_default().to_string() }
  }
  pub fn class_name_str(&self) -> String {
    unsafe { self.element.CurrentClassName().unwrap_or_default().to_string() }
  }
  pub fn help_text_str(&self) -> String {
    unsafe { self.element.CurrentHelpText().unwrap_or_default().to_string() }
  }
  pub fn control_type_str(&self) -> String {
    let id = unsafe { self.element.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) };
    get_control_type_name(id)
  }
  pub fn is_enabled_val(&self) -> bool {
    unsafe {
      self
        .element
        .CurrentIsEnabled()
        .unwrap_or(windows::core::BOOL::from(false))
        .as_bool()
    }
  }
  pub fn is_clickable_val(&self) -> bool {
    unsafe {
      self
        .element
        .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
        .is_ok()
    }
  }
  pub fn bounding_rect(&self) -> windows::Win32::Foundation::RECT {
    self.get_dpi_aware_bounding_rect().unwrap_or_default()
  }
  pub fn x_val(&self) -> i32 {
    self.bounding_rect().left
  }
  pub fn y_val(&self) -> i32 {
    self.bounding_rect().top
  }
  pub fn width_val(&self) -> i32 {
    let r = self.bounding_rect();
    r.right - r.left
  }
  pub fn height_val(&self) -> i32 {
    let r = self.bounding_rect();
    r.bottom - r.top
  }
  pub fn text_val(&self) -> String {
    self.get_text_content().unwrap_or_default()
  }

  pub fn pos(&self, hwnd: Option<i64>) -> std::result::Result<ElementPosition, String> {
    let rect = self.bounding_rect();
    let cx = rect.left + (rect.right - rect.left) / 2;
    let cy = rect.top + (rect.bottom - rect.top) / 2;
    let (rx, ry, ww, wh) = match hwnd {
      Some(h) => {
        let r = get_wnd_rect(h).map_err(|e| format!("{}", e))?;
        (cx - r.x, cy - r.y, r.w, r.h)
      }
      None => (0, 0, 0, 0),
    };
    Ok(ElementPosition {
      left: rect.left,
      top: rect.top,
      right: rect.right,
      bottom: rect.bottom,
      center_x: cx,
      center_y: cy,
      relative_center_x: rx,
      relative_center_y: ry,
      window_width: ww,
      window_height: wh,
    })
  }

  fn get_dpi_aware_bounding_rect(&self) -> Result<windows::Win32::Foundation::RECT> {
    unsafe {
      let rect = self.element.CurrentBoundingRectangle().context("BoundingRectangle")?;
      if rect.right < rect.left || rect.bottom < rect.top {
        return Err(anyhow!("Invalid rect"));
      }
      Ok(rect)
    }
  }

  fn get_text_content(&self) -> Result<String> {
    if let Ok(vp) = unsafe {
      self
        .element
        .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
    } && let Ok(v) = unsafe { vp.CurrentValue() }
      && !v.is_empty()
    {
      return Ok(v.to_string());
    }
    if let Ok(n) = unsafe { self.element.CurrentName() }
      && !n.is_empty()
    {
      return Ok(n.to_string());
    }
    Ok(String::new())
  }

  pub fn get_value(&self) -> Result<String> {
    unsafe {
      if let Ok(p) = self.element.GetCurrentPattern(UIA_ValuePatternId)
        && let Ok(vp) = p.cast::<IUIAutomationValuePattern>()
      {
        return Ok(vp.CurrentValue().context("GetValue")?.to_string());
      }
      Err(anyhow!("No ValuePattern"))
    }
  }

  pub fn set_value_str(&self, value: &str) -> bool {
    if !self.can_set_value().unwrap_or(false) {
      return false;
    }
    unsafe {
      if self.element.SetFocus().is_err() {
        return false;
      }
      std::thread::sleep(std::time::Duration::from_millis(50));
      if !self.is_focused_val().unwrap_or(false) {
        return false;
      }
      if keyboard::send_keys(vec![crate::types::KeyCode::A], Some(vec![crate::types::KeyCode::Ctrl])).is_err() {
        return false;
      }
      std::thread::sleep(std::time::Duration::from_millis(50));
      if self.send_unicode_text(value).is_err() {
        return false;
      }
      true
    }
  }

  pub fn can_set_value(&self) -> Result<bool> {
    unsafe {
      if !self.element.CurrentIsEnabled().context("enabled")?.as_bool() {
        return Ok(false);
      }
      if let Ok(p) = self.element.GetCurrentPattern(UIA_ValuePatternId)
        && let Ok(vp) = p.cast::<IUIAutomationValuePattern>()
      {
        return Ok(!vp.CurrentIsReadOnly().context("readonly")?.as_bool());
      }
      Ok(false)
    }
  }

  pub fn set_range_value(&self, value: f64) -> Result<()> {
    unsafe {
      if !self.element.CurrentIsEnabled().context("enabled")?.as_bool() {
        return Err(anyhow!("Not enabled"));
      }
      if let Ok(p) = self.element.GetCurrentPattern(UIA_RangeValuePatternId)
        && let Ok(rp) = p.cast::<IUIAutomationRangeValuePattern>()
      {
        if rp.CurrentIsReadOnly().context("readonly")?.as_bool() {
          return Err(anyhow!("Read-only"));
        }
        let min = rp.CurrentMinimum().context("min")?;
        let max = rp.CurrentMaximum().context("max")?;
        if value < min || value > max {
          return Err(anyhow!("Out of range {}-{}", min, max));
        }
        rp.SetValue(value).context("SetValue")?;
        return Ok(());
      }
      Err(anyhow!("No RangeValuePattern"))
    }
  }

  pub fn focus_element(&self) -> Result<()> {
    unsafe { self.element.SetFocus().context("SetFocus") }
  }

  pub fn is_focused_val(&self) -> Result<bool> {
    unsafe { Ok(self.element.CurrentHasKeyboardFocus().context("HasFocus")?.as_bool()) }
  }

  fn send_unicode_text(&self, text: &str) -> Result<()> {
    for ch in text.chars() {
      let uv = ch as u16;
      unsafe {
        let inputs = [
          INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
              ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: uv,
                dwFlags: KEYBD_EVENT_FLAGS(4),
                time: 0,
                dwExtraInfo: 0,
              },
            },
          },
          INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
              ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: uv,
                dwFlags: KEYBD_EVENT_FLAGS(6),
                time: 0,
                dwExtraInfo: 0,
              },
            },
          },
        ];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
  }

  pub fn to_xml_string(&self, indent: usize) -> String {
    let ind = "  ".repeat(indent);
    format!(
      "{}<{} Name=\"{}\" AutomationId=\"{}\" ClassName=\"{}\" HelpText=\"{}\" Enabled=\"{}\" Clickable=\"{}\" Width=\"{}\" Height=\"{}\" Text=\"{}\">",
      ind,
      escape_xml(&self.control_type_str()),
      escape_xml(&self.name_str()),
      escape_xml(&self.automation_id_str()),
      escape_xml(&self.class_name_str()),
      escape_xml(&self.help_text_str()),
      self.is_enabled_val(),
      self.is_clickable_val(),
      self.width_val(),
      self.height_val(),
      escape_xml(&self.text_val())
    )
  }
}

impl Element for WindowsElement {
  fn stable_id(&self) -> String {
    self.stable_id.clone()
  }

  fn automation_id(&self) -> String {
    self.automation_id_str()
  }
  fn name(&self) -> String {
    self.name_str()
  }
  fn class_name(&self) -> String {
    self.class_name_str()
  }
  fn control_type(&self) -> String {
    self.control_type_str()
  }
  fn help_text(&self) -> String {
    self.help_text_str()
  }
  fn is_enabled(&self) -> bool {
    self.is_enabled_val()
  }
  fn is_clickable(&self) -> bool {
    self.is_clickable_val()
  }
  fn x(&self) -> i32 {
    self.x_val()
  }
  fn y(&self) -> i32 {
    self.y_val()
  }
  fn width(&self) -> i32 {
    self.width_val()
  }
  fn height(&self) -> i32 {
    self.height_val()
  }
  fn text(&self) -> String {
    self.text_val()
  }
  fn pos(&self, window: Option<&crate::types::WindowHandle>) -> Result<ElementPosition> {
    self.pos(window.map(|h| h.0)).map_err(|e| anyhow::anyhow!(e))
  }
  fn get_value(&self) -> Result<String> {
    self.get_value()
  }
  fn set_value(&self, value: &str) -> bool {
    self.set_value_str(value)
  }
  fn can_set_value(&self) -> Result<bool> {
    self.can_set_value()
  }
  fn set_range_value(&self, value: f64) -> Result<()> {
    self.set_range_value(value)
  }
  fn focus(&self) -> Result<()> {
    self.focus_element()
  }
  fn set_focused(&self) -> Result<()> {
    self.focus_element()
  }
  fn is_focused(&self) -> Result<bool> {
    self.is_focused_val()
  }
  fn confirm(&self) -> Result<()> {
    use windows::Win32::UI::Accessibility::*;
    unsafe {
      if let Ok(p) = self
        .element
        .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
      {
        return p.Invoke().map_err(|e| anyhow::anyhow!("Invoke failed: {e}"));
      }
      if let Ok(p) = self
        .element
        .GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
      {
        return p.Toggle().map_err(|e| anyhow::anyhow!("Toggle failed: {e}"));
      }
      if let Ok(p) = self
        .element
        .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
      {
        return p.Select().map_err(|e| anyhow::anyhow!("Select failed: {e}"));
      }
      if let Ok(p) = self
        .element
        .GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
      {
        return p.Expand().map_err(|e| anyhow::anyhow!("Expand failed: {e}"));
      }
    }
    // Fallback: focus + Enter
    let _ = self.focus_element();
    keyboard::send_keys(vec![crate::types::KeyCode::Return], None)
  }
  fn to_xml(&self, indent: usize) -> String {
    self.to_xml_string(indent)
  }
}

fn escape_xml(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}
