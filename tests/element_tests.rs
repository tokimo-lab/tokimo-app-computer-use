#![cfg(windows)]
use std::thread;
use std::time::Duration;
use tokimo_app_computer_use::WindowHandle;
use tokimo_app_computer_use::create_platform;
use tokimo_app_computer_use::platform::*;

fn setup() -> impl PlatformProvider + Send + Sync {
  create_platform()
}

fn launch_and_get_handle(platform: &dyn PlatformProvider, path: &str, title_pattern: &str) -> (u32, WindowHandle) {
  let _launcher_pid = platform.launch_app(path, 3000).expect("launch app");
  thread::sleep(Duration::from_millis(3000));
  let windows = platform
    .find_windows_by_title(title_pattern, None)
    .expect("find windows by title");
  assert!(!windows.is_empty());
  let win = &windows[0];
  (win.process_id, WindowHandle(win.hwnd))
}

fn with_calculator<F: FnOnce(&dyn PlatformProvider, WindowHandle, u32)>(f: F) {
  let platform = setup();
  let (pid, handle) = launch_and_get_handle(&platform, r"C:\Windows\System32\calc.exe", "Calculator");
  f(&platform, handle, pid);
  let _ = platform.terminate_app(pid);
}

#[test]
fn test_find_elements_by_xpath_button() {
  with_calculator(|platform, handle, _| {
    let elements = platform
      .find_elements_by_xpath(&handle, "//Button")
      .expect("find buttons");
    assert!(!elements.is_empty(), "Should find at least one Button");
    for el in &elements {
      assert_eq!(el.control_type(), "Button");
    }
  });
}

#[test]
fn test_find_first_element_by_xpath() {
  with_calculator(|platform, handle, _| {
    let el = platform.find_first_element_by_xpath(&handle, "//Button[@Name='One']");
    assert!(el.is_ok(), "Should find button with Name='One'");
    let el = el.unwrap();
    assert_eq!(el.name(), "One");
  });
}

#[test]
fn test_find_element_by_name_attribute() {
  with_calculator(|platform, handle, _| {
    let elements = platform
      .find_elements_by_xpath(&handle, "//Button[@Name='Five']")
      .expect("find");
    assert!(!elements.is_empty(), "Should find button 'Five'");
    assert_eq!(elements[0].name(), "Five");
  });
}

#[test]
fn test_find_element_by_automation_id() {
  with_calculator(|platform, handle, _| {
    let elements = platform.find_elements_by_xpath(&handle, "//Button[@AutomationId='num5Button']");
    if let Ok(elems) = elements
      && !elems.is_empty()
    {
      assert_eq!(elems[0].automation_id(), "num5Button");
    }
  });
}

#[test]
fn test_find_element_contains() {
  with_calculator(|platform, handle, _| {
    let elements = platform.find_elements_by_xpath(&handle, "//Button[contains(@Name,'One')]");
    assert!(elements.is_ok(), "contains() xpath should work");
    let elements = elements.unwrap();
    assert!(!elements.is_empty(), "Should find button containing 'One'");
  });
}

#[test]
fn test_element_properties() {
  with_calculator(|platform, handle, _| {
    let el = platform
      .find_first_element_by_xpath(&handle, "//Button[@Name='One']")
      .expect("find");
    let _ = el.name();
    let _ = el.class_name();
    let _ = el.control_type();
    let _ = el.automation_id();
    let _ = el.help_text();
    let _ = el.is_enabled();
    let _ = el.is_clickable();
    let _ = el.x();
    let _ = el.y();
    let _ = el.width();
    let _ = el.height();
    let _ = el.text();
  });
}

#[test]
fn test_element_position() {
  with_calculator(|platform, handle, _| {
    let el = platform
      .find_first_element_by_xpath(&handle, "//Button[@Name='One']")
      .expect("find");
    let pos = el.pos(Some(&handle));
    assert!(pos.is_ok(), "pos() should succeed");
    let pos = pos.unwrap();
    assert!(pos.right > pos.left, "Right should be greater than left");
    assert!(pos.bottom > pos.top, "Bottom should be greater than top");
  });
}

#[test]
fn test_element_focus() {
  with_calculator(|platform, handle, _| {
    let el = platform
      .find_first_element_by_xpath(&handle, "//Button[@Name='One']")
      .expect("find");
    let result = el.focus();
    assert!(result.is_ok(), "focus() should succeed");
  });
}

#[test]
fn test_element_to_xml() {
  with_calculator(|platform, handle, _| {
    let el = platform
      .find_first_element_by_xpath(&handle, "//Button[@Name='One']")
      .expect("find");
    let xml = el.to_xml(0);
    assert!(!xml.is_empty(), "to_xml should return non-empty string");
    assert!(xml.contains("Button"), "XML should contain control type");
  });
}

#[test]
fn test_get_page_source() {
  with_calculator(|platform, handle, _| {
    let xml = platform.get_page_source(&handle).expect("get page source");
    assert!(!xml.is_empty(), "Page source should not be empty");
    assert!(xml.contains("<?xml"), "Should start with XML declaration");
    assert!(xml.contains("UIAutomationTree"), "Should contain root element");
  });
}

#[test]
fn test_find_edit_element() {
  with_calculator(|platform, handle, _| {
    let elements = platform.find_elements_by_xpath(&handle, "//Edit");
    if let Ok(edits) = elements
      && !edits.is_empty()
    {
      assert_eq!(edits[0].control_type(), "Edit");
    }
  });
}
