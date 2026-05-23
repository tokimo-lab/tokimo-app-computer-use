use crate::platform::windows::elements::utils::get_control_type_name;
use crate::platform::windows::system_info::ensure_com_initialized;
use crate::platform::windows::system_info::get_screen_size;
use anyhow::Result;
use std::io::{BufWriter, Write};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
use windows::Win32::{Foundation::*, System::Com::*, UI::Accessibility::*};
use windows::core::BOOL;

fn escape_xml(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

fn create_cache_request(automation: &IUIAutomation) -> Result<IUIAutomationCacheRequest> {
  unsafe {
    let cr = automation.CreateCacheRequest()?;
    cr.AddProperty(UIA_NamePropertyId)?;
    cr.AddProperty(UIA_AutomationIdPropertyId)?;
    cr.AddProperty(UIA_ClassNamePropertyId)?;
    cr.AddProperty(UIA_HelpTextPropertyId)?;
    cr.AddProperty(UIA_IsEnabledPropertyId)?;
    cr.AddProperty(UIA_BoundingRectanglePropertyId)?;
    cr.AddProperty(UIA_ControlTypePropertyId)?;
    cr.AddProperty(UIA_IsOffscreenPropertyId)?;
    cr.AddPattern(UIA_InvokePatternId)?;
    cr.AddPattern(UIA_ValuePatternId)?;
    cr.AddPattern(UIA_TogglePatternId)?;
    cr.AddPattern(UIA_ExpandCollapsePatternId)?;
    cr.AddPattern(UIA_RangeValuePatternId)?;
    cr.SetTreeScope(TreeScope_Subtree)?;
    cr.SetTreeFilter(&automation.RawViewCondition()?)?;
    cr.SetAutomationElementMode(AutomationElementMode_Full)?;
    Ok(cr)
  }
}

#[derive(Debug)]
struct CachedInfo {
  name: String,
  automation_id: String,
  class_name: String,
  help_text: String,
  is_enabled: bool,
  bounding_rect: RECT,
  control_type_id: UIA_CONTROLTYPE_ID,
}

fn get_cached_props(el: &IUIAutomationElement) -> Result<CachedInfo> {
  unsafe {
    Ok(CachedInfo {
      name: el.CachedName().unwrap_or_default().to_string(),
      automation_id: el.CachedAutomationId().unwrap_or_default().to_string(),
      class_name: el.CachedClassName().unwrap_or_default().to_string(),
      help_text: el.CachedHelpText().unwrap_or_default().to_string(),
      is_enabled: el.CachedIsEnabled().unwrap_or(BOOL::from(false)).as_bool(),
      bounding_rect: el.CachedBoundingRectangle()?,
      control_type_id: el.CachedControlType().unwrap_or(UIA_CustomControlTypeId),
    })
  }
}

fn is_interactive(el: &IUIAutomationElement) -> bool {
  unsafe {
    el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
      .is_ok()
      || el
        .GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
        .is_ok()
  }
}

fn get_cached_text(el: &IUIAutomationElement, fallback: &str) -> String {
  if let Ok(vp) = unsafe { el.GetCachedPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
    && let Ok(v) = unsafe { vp.CachedValue() }
    && !v.is_empty()
  {
    return v.to_string();
  }
  if !fallback.is_empty() {
    return fallback.to_string();
  }
  String::new()
}

fn create_cached_xml(info: &CachedInfo, el: &IUIAutomationElement, indent: usize, ox: i32, oy: i32) -> Result<String> {
  let ind = "  ".repeat(indent);
  let ct = get_control_type_name(info.control_type_id);
  let clickable = is_interactive(el);
  let (x, y, w, h) = (
    info.bounding_rect.left - ox,
    info.bounding_rect.top - oy,
    info.bounding_rect.right - info.bounding_rect.left,
    info.bounding_rect.bottom - info.bounding_rect.top,
  );
  let text = get_cached_text(el, &info.name);
  let mut attrs = format!("<{}", escape_xml(&ct));
  if !info.name.is_empty() {
    attrs.push_str(&format!(" Name=\"{}\"", escape_xml(&info.name)));
  }
  if !info.automation_id.is_empty() {
    attrs.push_str(&format!(" AutomationId=\"{}\"", escape_xml(&info.automation_id)));
  }
  if !info.class_name.is_empty() {
    attrs.push_str(&format!(" ClassName=\"{}\"", escape_xml(&info.class_name)));
  }
  if !info.help_text.is_empty() {
    attrs.push_str(&format!(" HelpText=\"{}\"", escape_xml(&info.help_text)));
  }
  attrs.push_str(&format!(
    " Enabled=\"{}\" Clickable=\"{}\" X=\"{}\" Y=\"{}\" Width=\"{}\" Height=\"{}\"",
    info.is_enabled, clickable, x, y, w, h
  ));
  if !text.is_empty() {
    attrs.push_str(&format!(" Text=\"{}\"", escape_xml(&text)));
  }
  attrs.push('>');
  Ok(format!("{}{}", ind, attrs))
}

#[allow(clippy::too_many_arguments)]
fn traverse_children(
  automation: &IUIAutomation,
  el: &IUIAutomationElement,
  indent: usize,
  writer: &mut BufWriter<&mut Vec<u8>>,
  depth: i32,
  ox: i32,
  oy: i32,
  sw: i32,
  sh: i32,
) -> Result<()> {
  if let Ok(children) = unsafe { el.GetCachedChildren() } {
    let count = unsafe { children.Length()? };
    for i in 0..count {
      if let Ok(child) = unsafe { children.GetElement(i) } {
        build_tree(automation, &child, indent, writer, depth, ox, oy, sw, sh)?;
      }
    }
  }
  Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_tree(
  automation: &IUIAutomation,
  el: &IUIAutomationElement,
  indent: usize,
  writer: &mut BufWriter<&mut Vec<u8>>,
  depth: i32,
  ox: i32,
  oy: i32,
  sw: i32,
  sh: i32,
) -> Result<()> {
  if depth > 30 {
    return Ok(());
  }
  let info = get_cached_props(el)?;
  let xml = create_cached_xml(&info, el, indent, ox, oy)?;
  writeln!(writer, "{}", xml)?;
  if info.control_type_id == UIA_DocumentControlTypeId {
    if let Ok(children) = unsafe { el.GetCachedChildren() } {
      let count = unsafe { children.Length()? };
      for i in 0..count {
        if let Ok(child) = unsafe { children.GetElement(i) } {
          build_tree(automation, &child, indent + 1, writer, depth + 1, ox, oy, sw, sh)?;
        }
      }
    }
  } else {
    traverse_children(automation, el, indent + 1, writer, depth + 1, ox, oy, sw, sh)?;
  }
  let ct = get_control_type_name(info.control_type_id);
  writeln!(writer, "{}</{}>", "  ".repeat(indent), ct)?;
  Ok(())
}

pub fn get_page_source_from_hwnd(hwnd_handle: i64) -> Result<String> {
  let hwnd = HWND(hwnd_handle as isize as *mut std::ffi::c_void);
  ensure_com_initialized();
  let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
  let cr = create_cache_request(&automation)?;
  let root = unsafe { automation.ElementFromHandleBuildCache(hwnd, &cr)? };
  let (sw, sh) = get_screen_size()?;
  let mut wr = RECT::default();
  unsafe {
    GetWindowRect(hwnd, &mut wr)?;
  }
  let (wsx, wsy, ww, wh) = (wr.left, wr.top, wr.right - wr.left, wr.bottom - wr.top);
  let mut buf = Vec::new();
  {
    let mut writer = BufWriter::new(&mut buf);
    writeln!(writer, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
    writeln!(
      writer,
      "<UIAutomationTree strategy=\"cached\" ScreenWidth=\"{}\" ScreenHeight=\"{}\" WindowScreenX=\"{}\" WindowScreenY=\"{}\" Width=\"{}\" Height=\"{}\">",
      sw, sh, wsx, wsy, ww, wh
    )?;
    let _raw_walker = unsafe { automation.RawViewWalker()? };
    build_tree(&automation, &root, 0, &mut writer, 0, wsx, wsy, sw, sh)?;
    writeln!(writer, "</UIAutomationTree>")?;
    writer.flush()?;
  }
  String::from_utf8(buf).map_err(|e| anyhow::anyhow!("UTF-8 error: {}", e))
}
