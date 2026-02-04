use crate::ui::{theme, Icons};
use owo_colors::OwoColorize;

pub fn header(text: &str) {
    println!("{} {}", Icons::ROCKET, text.style(theme().header.clone()));
}

pub fn status(icon: &str, label: &str, value: &str) {
    println!("{} {}: {}", icon, label.style(theme().dim.clone()), value);
}

pub fn success(label: &str) {
    println!("{} {}", Icons::CHECK, label.style(theme().success.clone()));
}

pub fn error(label: &str) {
    eprintln!("{} {}", Icons::CROSS, label.style(theme().error.clone()));
}

pub fn warn(label: &str) {
    eprintln!("{} {}", Icons::WARN, label.style(theme().warn.clone()));
}

pub fn info(label: &str, value: &str) {
    println!(
        "{} {}: {}",
        Icons::INFO.style(theme().info.clone()),
        label.style(theme().dim.clone()),
        value
    );
}

pub fn section(title: &str) {
    println!();
    println!("━{}━", title.style(theme().header.clone()));
}

pub fn dim(text: &str) -> String {
    text.style(theme().dim.clone()).to_string()
}

pub fn muted(text: &str) -> String {
    text.style(theme().muted.clone()).to_string()
}

pub fn file_modified(path: &str) {
    println!("{} {}", Icons::MOD.style(theme().warn.clone()), path);
}

pub fn file_new(path: &str) {
    println!("{} {}", Icons::NEW.style(theme().success.clone()), path);
}

pub fn file_deleted(path: &str) {
    println!("{} {}", Icons::DEL.style(theme().error.clone()), path);
}

pub fn file_unchanged(path: &str) {
    println!("  {}", path.style(theme().muted.clone()));
}

pub fn phase(name: &str) {
    println!();
    println!(
        "{} {}",
        Icons::GEAR.style(theme().info.clone()),
        name.style(theme().header.clone())
    );
}

pub fn timing(elapsed: &str) {
    println!("{} {}", Icons::CLOCK.style(theme().dim.clone()), elapsed);
}

pub fn summary_row(label: &str, value: &str) {
    println!("  {} {}", label.style(theme().dim.clone()), value);
}

pub fn banner(title: &str, subtitle: &str) {
    let width = 60;
    let line = "─".repeat(width - 2);
    
    println!();
    println!("  {}", format!("┌{}┐", line).style(theme().dim.clone()));
    
    let title_line = center_text(title, width - 4);
    println!("  {}{}{}", "│".style(theme().dim.clone()), title_line, "│".style(theme().dim.clone()));
    
    let subtitle_line = center_text(subtitle, width - 4);
    println!("  {}{}{}", "│".style(theme().dim.clone()), subtitle_line, "│".style(theme().dim.clone()));
    
    println!("  {}", format!("└{}┘", line).style(theme().dim.clone()));
    println!();
}

fn center_text(text: &str, width: usize) -> String {
    let text_len = console::measure_text_width(text);
    if text_len >= width {
        // Truncate if too long (naive truncation for now)
        return text.chars().take(width).collect();
    }
    let left = (width - text_len) / 2;
    let right = width - text_len - left;
    format!(" {} ", format!("{}{}{}", " ".repeat(left), text, " ".repeat(right)))
}

pub fn human_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.2} {}", size, units[unit_idx])
}
