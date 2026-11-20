//! What the cursor says before it acts on an element: a short verb phrase
//! such as "Clicking Bold" or "Typing into Notes", shown before the cursor
//! sets off, so a person watching can trust what happens next.

use crate::{Action, Direction, RefEntry};

const MAX_NAME_CHARS: usize = 32;

pub(super) fn narration(entry: &RefEntry, action: &Action) -> String {
    let name = element_name(entry);
    let app = entry
        .source
        .source_app
        .as_deref()
        .map(str::trim)
        .filter(|app| !app.is_empty());
    let described = name
        .is_none()
        .then(|| unnamed(&entry.identity.role, app))
        .flatten();
    let target = name.as_deref().or(described.as_deref()).or(app);
    let phrase = |verb: &str| match target {
        Some(target) => format!("{verb} {target}"),
        None => verb.to_owned(),
    };
    match action {
        Action::Click => phrase("Clicking"),
        Action::DoubleClick => phrase("Double-clicking"),
        Action::RightClick => phrase("Right-clicking"),
        Action::TripleClick => phrase("Selecting the line in"),
        Action::TypeText(_) => into("Typing", target),
        Action::SetValue(_) => phrase("Setting"),
        Action::Clear => phrase("Clearing"),
        Action::SetFocus => phrase("Focusing"),
        Action::Expand => phrase("Opening"),
        Action::Collapse => phrase("Closing"),
        Action::Select(value) => format!("Choosing {}", clip(value.trim())),
        Action::Toggle => phrase("Switching"),
        Action::Check => phrase("Turning on"),
        Action::Uncheck => phrase("Turning off"),
        Action::Scroll(direction, _) => scrolling(direction, target),
        Action::ScrollTo => phrase("Scrolling to"),
        Action::PressKey(_) | Action::KeyDown(_) | Action::KeyUp(_) => phrase("Pressing keys in"),
        Action::Hover => phrase("Pointing at"),
        Action::Drag(_) => phrase("Dragging"),
    }
}

fn into(verb: &str, target: Option<&str>) -> String {
    target.map_or_else(|| verb.to_owned(), |target| format!("{verb} into {target}"))
}

fn scrolling(direction: &Direction, target: Option<&str>) -> String {
    let way = match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    };
    match target {
        Some(target) => format!("Scrolling {target} {way}"),
        None => format!("Scrolling {way}"),
    }
}

fn element_name(entry: &RefEntry) -> Option<String> {
    entry
        .identity
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(clip)
}

/// An element with no name is described by what kind of control it is, such
/// as "the menu button in TextEdit", so the label says what will be pressed
/// rather than only which app. Text fields and containers keep the app name:
/// "Typing into TextEdit" already says where the text goes.
fn unnamed(role: &str, app: Option<&str>) -> Option<String> {
    let kind = match role.trim().to_ascii_lowercase().as_str() {
        "button" => "button",
        "menubutton" => "menu button",
        "popupbutton" => "pop-up menu",
        "checkbox" => "checkbox",
        "radiobutton" => "option",
        "tab" => "tab",
        "link" => "link",
        "slider" => "slider",
        "combobox" => "combo box",
        "menuitem" => "menu item",
        "image" => "image",
        _ => return None,
    };
    Some(match app {
        Some(app) => format!("the {kind} in {app}"),
        None => format!("the {kind}"),
    })
}

fn clip(text: &str) -> String {
    let mut clipped: String = text.chars().take(MAX_NAME_CHARS).collect();
    if text.chars().count() > MAX_NAME_CHARS {
        clipped.push('…');
    }
    clipped
}

#[cfg(test)]
#[path = "announce_tests.rs"]
mod tests;
