use crate::platform::windows::elements::utils::get_control_type_id_by_name;
use crate::platform::windows::system_info::ensure_com_initialized;
use crate::platform::windows::ui_object::WindowsElement;
use anyhow::Result;
use windows::Win32::{System::Com::*, UI::Accessibility::*};
use windows::core::BOOL;

pub fn find_elements_by_handle_xpath_internal(hwnd: i64, xpath: &str) -> Result<Vec<WindowsElement>> {
  ensure_com_initialized();
  let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };
  let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut core::ffi::c_void);
  let window_element = unsafe { automation.ElementFromHandle(hwnd)? };
  parse_xpath(&automation, &window_element, xpath)
}

fn compute_runtime_id(el: &IUIAutomationElement) -> String {
  unsafe {
    el.CurrentRuntimeId()
      .map(|ids| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" "))
      .unwrap_or_default()
  }
}

fn find_elements_with_raw_walker(
  walker: &IUIAutomationTreeWalker,
  start: &IUIAutomationElement,
  matcher: &mut dyn FnMut(&IUIAutomationElement) -> bool,
  found: &mut Vec<WindowsElement>,
  depth: i32,
  include_offscreen: bool,
) -> Result<()> {
  if depth > 50 {
    return Ok(());
  }
  if !include_offscreen && unsafe { start.CurrentIsOffscreen().unwrap_or(BOOL::from(true)).as_bool() } {
    return Ok(());
  }
  if matcher(start) {
    found.push(WindowsElement {
      depth: None,
      selector: String::new(),
      stable_id: compute_runtime_id(start),
      element: start.clone(),
    });
  }
  if let Ok(child) = unsafe { walker.GetFirstChildElement(start) } {
    let mut current = Some(child);
    while let Some(c) = current {
      find_elements_with_raw_walker(walker, &c, matcher, found, depth + 1, include_offscreen)?;
      current = unsafe { walker.GetNextSiblingElement(&c).ok() };
    }
  }
  Ok(())
}

fn find_elements_by_control_type_name(
  automation: &IUIAutomation,
  root: &IUIAutomationElement,
  type_name: &str,
) -> Result<Vec<WindowsElement>> {
  if let Some(target_id) = get_control_type_id_by_name(type_name) {
    let walker = unsafe { automation.RawViewWalker()? };
    let mut found = Vec::new();
    let mut matcher = |el: &IUIAutomationElement| -> bool {
      unsafe { el.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) == target_id }
    };
    find_elements_with_raw_walker(&walker, root, &mut matcher, &mut found, 0, false)?;
    Ok(found)
  } else {
    Ok(Vec::new())
  }
}

fn parse_xpath(automation: &IUIAutomation, root: &IUIAutomationElement, xpath: &str) -> Result<Vec<WindowsElement>> {
  if let Some(stripped) = xpath.strip_prefix("//") {
    parse_descendant_xpath(automation, root, stripped)
  } else if let Some(stripped) = xpath.strip_prefix('/') {
    parse_descendant_xpath(automation, root, stripped)
  } else if xpath == "*" {
    find_all_elements(automation, root)
  } else {
    Ok(Vec::new())
  }
}

fn parse_descendant_xpath(
  automation: &IUIAutomation,
  root: &IUIAutomationElement,
  xpath: &str,
) -> Result<Vec<WindowsElement>> {
  if xpath == "*" {
    return find_all_elements(automation, root);
  }
  if let Some(b) = xpath.find('[') {
    let et = &xpath[..b];
    let rest = &xpath[b..];
    if et == "*" {
      parse_predicate_for_all(automation, root, rest)
    } else {
      parse_element_with_predicates(automation, root, et, rest)
    }
  } else {
    find_elements_by_control_type_name(automation, root, xpath)
  }
}

fn find_all_elements(automation: &IUIAutomation, root: &IUIAutomationElement) -> Result<Vec<WindowsElement>> {
  let walker = unsafe { automation.RawViewWalker()? };
  let mut found = Vec::new();
  find_elements_with_raw_walker(&walker, root, &mut |_| true, &mut found, 0, false)?;
  Ok(found)
}

fn parse_predicate_for_all(
  automation: &IUIAutomation,
  root: &IUIAutomationElement,
  pred_str: &str,
) -> Result<Vec<WindowsElement>> {
  let walker = unsafe { automation.RawViewWalker()? };
  let mut found = Vec::new();
  let predicates = parse_predicates(pred_str)?;
  let mut matcher = |el: &IUIAutomationElement| -> bool { evaluate_predicates(el, &predicates) };
  find_elements_with_raw_walker(&walker, root, &mut matcher, &mut found, 0, false)?;
  Ok(found)
}

fn parse_element_with_predicates(
  automation: &IUIAutomation,
  root: &IUIAutomationElement,
  element_type: &str,
  pred_str: &str,
) -> Result<Vec<WindowsElement>> {
  let target_id = match get_control_type_id_by_name(element_type) {
    Some(id) => id,
    None => return Ok(Vec::new()),
  };
  let walker = unsafe { automation.RawViewWalker()? };
  let mut found = Vec::new();
  let predicates = parse_predicates(pred_str)?;
  let pos_preds: Vec<&Predicate> = predicates
    .iter()
    .filter(|p| matches!(p, Predicate::Position(_)))
    .collect();
  let other_preds: Vec<&Predicate> = predicates
    .iter()
    .filter(|p| !matches!(p, Predicate::Position(_)))
    .collect();
  let mut matcher = |el: &IUIAutomationElement| -> bool {
    if unsafe { el.CurrentControlType().unwrap_or(UIA_CustomControlTypeId) } != target_id {
      return false;
    }
    other_preds.iter().all(|p| evaluate_predicate(el, p))
  };
  find_elements_with_raw_walker(&walker, root, &mut matcher, &mut found, 0, false)?;
  for pp in &pos_preds {
    if let Predicate::Position(pos) = pp {
      if *pos > 0 && (*pos as usize) <= found.len() {
        let idx = (*pos as usize) - 1;
        let sel = found.swap_remove(idx);
        found = vec![sel];
        break;
      } else {
        found.clear();
        break;
      }
    }
  }
  Ok(found)
}

#[derive(Debug, Clone)]
enum Predicate {
  AttributeEquals(String, String),
  AttributeContains(String, String),
  AttributeStartsWith(String, String),
  AttributeExists(String),
  Position(i32),
  And(Box<Predicate>, Box<Predicate>),
  Or(Box<Predicate>, Box<Predicate>),
}

fn parse_predicates(s: &str) -> Result<Vec<Predicate>> {
  let content = s.trim_start_matches('[').trim_end_matches(']');
  let parts = if content.contains(" and ") {
    content.split(" and ").map(|s| s.trim().to_string()).collect()
  } else {
    vec![content.to_string()]
  };
  let mut preds = Vec::new();
  for part in parts {
    if let Some(p) = parse_single_predicate(&part)? {
      preds.push(p);
    }
  }
  Ok(preds)
}

fn parse_single_predicate(p: &str) -> Result<Option<Predicate>> {
  let p = p.trim();
  if p.contains(" or ") {
    let parts: Vec<&str> = p.split(" or ").collect();
    if parts.len() == 2
      && let (Some(l), Some(r)) = (parse_single_predicate(parts[0])?, parse_single_predicate(parts[1])?)
    {
      return Ok(Some(Predicate::Or(Box::new(l), Box::new(r))));
    }
  }
  if p.contains(" and ") {
    let parts: Vec<&str> = p.split(" and ").collect();
    if parts.len() == 2
      && let (Some(l), Some(r)) = (parse_single_predicate(parts[0])?, parse_single_predicate(parts[1])?)
    {
      return Ok(Some(Predicate::And(Box::new(l), Box::new(r))));
    }
  }
  if let Some(stripped) = p.strip_prefix("position()=") {
    if let Ok(pos) = stripped.parse::<i32>() {
      return Ok(Some(Predicate::Position(pos)));
    }
  } else if let Ok(pos) = p.parse::<i32>() {
    return Ok(Some(Predicate::Position(pos)));
  }
  if p.starts_with("contains(") && p.ends_with(')') {
    let inner = &p[9..p.len() - 1];
    if let Some(c) = inner.find(',') {
      let attr = inner[..c].trim().trim_start_matches('@');
      let val = inner[c + 1..].trim().trim_matches('"').trim_matches('\'');
      return Ok(Some(Predicate::AttributeContains(attr.to_string(), val.to_string())));
    }
  }
  if p.starts_with("starts-with(") && p.ends_with(')') {
    let inner = &p[12..p.len() - 1];
    if let Some(c) = inner.find(',') {
      let attr = inner[..c].trim().trim_start_matches('@');
      let val = inner[c + 1..].trim().trim_matches('"').trim_matches('\'');
      return Ok(Some(Predicate::AttributeStartsWith(attr.to_string(), val.to_string())));
    }
  }
  if let Some(stripped) = p.strip_prefix('@') {
    if let Some(eq) = stripped.find('=') {
      let attr = stripped[..eq].trim();
      let val = stripped[eq + 1..].trim().trim_matches('"').trim_matches('\'');
      return Ok(Some(Predicate::AttributeEquals(attr.to_string(), val.to_string())));
    } else {
      return Ok(Some(Predicate::AttributeExists(stripped.trim().to_string())));
    }
  }
  Ok(None)
}

fn evaluate_predicates(el: &IUIAutomationElement, preds: &[Predicate]) -> bool {
  preds.iter().all(|p| evaluate_predicate(el, p))
}

#[allow(clippy::cmp_owned)]
fn evaluate_predicate(el: &IUIAutomationElement, pred: &Predicate) -> bool {
  match pred {
    Predicate::AttributeEquals(attr, val) => match attr.to_lowercase().as_str() {
      "name" => unsafe { el.CurrentName() }.is_ok_and(|v| v.to_string() == *val),
      "automationid" => unsafe { el.CurrentAutomationId() }.is_ok_and(|v| v.to_string() == *val),
      "classname" => unsafe { el.CurrentClassName() }.is_ok_and(|v| v.to_string() == *val),
      _ => false,
    },
    Predicate::AttributeContains(attr, val) => match attr.to_lowercase().as_str() {
      "name" => unsafe { el.CurrentName() }.is_ok_and(|v| v.to_string().contains(val)),
      "automationid" => unsafe { el.CurrentAutomationId() }.is_ok_and(|v| v.to_string().contains(val)),
      "classname" => unsafe { el.CurrentClassName() }.is_ok_and(|v| v.to_string().contains(val)),
      _ => false,
    },
    Predicate::AttributeStartsWith(attr, val) => match attr.to_lowercase().as_str() {
      "name" => unsafe { el.CurrentName() }.is_ok_and(|v| v.to_string().starts_with(val)),
      "automationid" => unsafe { el.CurrentAutomationId() }.is_ok_and(|v| v.to_string().starts_with(val)),
      "classname" => unsafe { el.CurrentClassName() }.is_ok_and(|v| v.to_string().starts_with(val)),
      _ => false,
    },
    Predicate::AttributeExists(attr) => match attr.to_lowercase().as_str() {
      "name" => unsafe { el.CurrentName() }.is_ok(),
      "automationid" => unsafe { el.CurrentAutomationId() }.is_ok(),
      "classname" => unsafe { el.CurrentClassName() }.is_ok(),
      _ => false,
    },
    Predicate::Position(_) => true,
    Predicate::And(l, r) => evaluate_predicate(el, l) && evaluate_predicate(el, r),
    Predicate::Or(l, r) => evaluate_predicate(el, l) || evaluate_predicate(el, r),
  }
}
