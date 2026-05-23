#![allow(non_upper_case_globals)]

use crate::platform::windows::elements::find::find_elements_by_handle_xpath_internal;
use crate::platform::windows::ui_object::WindowsElement;
use anyhow::Result;
use windows::Win32::UI::Accessibility::*;

pub fn find_first_element_by_xpath(hwnd: i64, xpath: &str) -> Result<WindowsElement> {
  let mut elements = find_elements_by_handle_xpath_internal(hwnd, xpath)?;
  if elements.is_empty() {
    return Err(anyhow::anyhow!("Element with xpath '{}' not found", xpath));
  }
  Ok(elements.swap_remove(0))
}

pub fn get_control_type_name(id: UIA_CONTROLTYPE_ID) -> String {
  match id {
    UIA_ButtonControlTypeId => "Button",
    UIA_CalendarControlTypeId => "Calendar",
    UIA_CheckBoxControlTypeId => "CheckBox",
    UIA_ComboBoxControlTypeId => "ComboBox",
    UIA_EditControlTypeId => "Edit",
    UIA_HyperlinkControlTypeId => "Hyperlink",
    UIA_ImageControlTypeId => "Image",
    UIA_ListItemControlTypeId => "ListItem",
    UIA_ListControlTypeId => "List",
    UIA_MenuControlTypeId => "Menu",
    UIA_MenuBarControlTypeId => "MenuBar",
    UIA_MenuItemControlTypeId => "MenuItem",
    UIA_ProgressBarControlTypeId => "ProgressBar",
    UIA_RadioButtonControlTypeId => "RadioButton",
    UIA_ScrollBarControlTypeId => "ScrollBar",
    UIA_SliderControlTypeId => "Slider",
    UIA_SpinnerControlTypeId => "Spinner",
    UIA_StatusBarControlTypeId => "StatusBar",
    UIA_TabControlTypeId => "Tab",
    UIA_TabItemControlTypeId => "TabItem",
    UIA_TextControlTypeId => "Text",
    UIA_ToolBarControlTypeId => "ToolBar",
    UIA_ToolTipControlTypeId => "ToolTip",
    UIA_TreeControlTypeId => "Tree",
    UIA_TreeItemControlTypeId => "TreeItem",
    UIA_CustomControlTypeId => "Custom",
    UIA_GroupControlTypeId => "Group",
    UIA_ThumbControlTypeId => "Thumb",
    UIA_DataGridControlTypeId => "DataGrid",
    UIA_DataItemControlTypeId => "DataItem",
    UIA_DocumentControlTypeId => "Document",
    UIA_SplitButtonControlTypeId => "SplitButton",
    UIA_WindowControlTypeId => "Window",
    UIA_PaneControlTypeId => "Pane",
    UIA_HeaderControlTypeId => "Header",
    UIA_HeaderItemControlTypeId => "HeaderItem",
    UIA_TableControlTypeId => "Table",
    UIA_TitleBarControlTypeId => "TitleBar",
    UIA_SeparatorControlTypeId => "Separator",
    UIA_SemanticZoomControlTypeId => "SemanticZoom",
    UIA_AppBarControlTypeId => "AppBar",
    _ => "Unknown",
  }
  .to_string()
}

pub fn get_control_type_id_by_name(name: &str) -> Option<UIA_CONTROLTYPE_ID> {
  match name.to_lowercase().as_str() {
    "button" => Some(UIA_ButtonControlTypeId),
    "calendar" => Some(UIA_CalendarControlTypeId),
    "checkbox" => Some(UIA_CheckBoxControlTypeId),
    "combobox" => Some(UIA_ComboBoxControlTypeId),
    "edit" => Some(UIA_EditControlTypeId),
    "hyperlink" => Some(UIA_HyperlinkControlTypeId),
    "image" => Some(UIA_ImageControlTypeId),
    "listitem" => Some(UIA_ListItemControlTypeId),
    "list" => Some(UIA_ListControlTypeId),
    "menu" => Some(UIA_MenuControlTypeId),
    "menubar" => Some(UIA_MenuBarControlTypeId),
    "menuitem" => Some(UIA_MenuItemControlTypeId),
    "progressbar" => Some(UIA_ProgressBarControlTypeId),
    "radiobutton" => Some(UIA_RadioButtonControlTypeId),
    "scrollbar" => Some(UIA_ScrollBarControlTypeId),
    "slider" => Some(UIA_SliderControlTypeId),
    "spinner" => Some(UIA_SpinnerControlTypeId),
    "statusbar" => Some(UIA_StatusBarControlTypeId),
    "tab" => Some(UIA_TabControlTypeId),
    "tabitem" => Some(UIA_TabItemControlTypeId),
    "text" => Some(UIA_TextControlTypeId),
    "toolbar" => Some(UIA_ToolBarControlTypeId),
    "tooltip" => Some(UIA_ToolTipControlTypeId),
    "tree" => Some(UIA_TreeControlTypeId),
    "treeitem" => Some(UIA_TreeItemControlTypeId),
    "custom" => Some(UIA_CustomControlTypeId),
    "group" => Some(UIA_GroupControlTypeId),
    "thumb" => Some(UIA_ThumbControlTypeId),
    "datagrid" => Some(UIA_DataGridControlTypeId),
    "dataitem" => Some(UIA_DataItemControlTypeId),
    "document" => Some(UIA_DocumentControlTypeId),
    "splitbutton" => Some(UIA_SplitButtonControlTypeId),
    "window" => Some(UIA_WindowControlTypeId),
    "pane" => Some(UIA_PaneControlTypeId),
    "header" => Some(UIA_HeaderControlTypeId),
    "headeritem" => Some(UIA_HeaderItemControlTypeId),
    "table" => Some(UIA_TableControlTypeId),
    "titlebar" => Some(UIA_TitleBarControlTypeId),
    "separator" => Some(UIA_SeparatorControlTypeId),
    "semanticzoom" => Some(UIA_SemanticZoomControlTypeId),
    "appbar" => Some(UIA_AppBarControlTypeId),
    _ => None,
  }
}
