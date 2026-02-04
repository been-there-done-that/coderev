use crate::ui::theme;
use owo_colors::OwoColorize;

pub struct TableRow {
    pub metric: String,
    pub value: String,
}

pub struct SimpleTable {
    rows: Vec<TableRow>,
}

impl SimpleTable {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn add_row(&mut self, label: &str, value: &str) {
        self.rows.push(TableRow {
            metric: label.to_string(),
            value: value.to_string(),
        });
    }

    pub fn print(&self) {
        if self.rows.is_empty() {
            return;
        }

        // 1. Calculate column widths
        let mut m_w = console::measure_text_width("Metric");
        let mut v_w = console::measure_text_width("Value");

        for row in &self.rows {
            m_w = m_w.max(console::measure_text_width(&row.metric));
            v_w = v_w.max(console::measure_text_width(&row.value));
        }

        // Add padding (1 space on each side)
        m_w += 2;
        v_w += 2;

        let inner_width = m_w + v_w + 2; // Col1 + Gap (2 spaces) + Col2
        let dim = theme().dim.clone();
        let h_line = "─".repeat(inner_width);

        // 2. Borders
        let top = format!("┌{}┐", h_line);
        let mid = format!("├{}┤", h_line);
        let bot = format!("└{}┘", h_line);

        // 3. Print
        println!("  {}", top.style(dim.clone()));
        
        // Header
        let h_m = pad_str("Metric", m_w);
        let h_v = pad_str("Value", v_w);
        println!(
            "  │{}  {}│", 
            h_m.style(theme().header.clone()), 
            h_v.style(theme().header.clone())
        );

        println!("  {}", mid.style(dim.clone()));

        for row in &self.rows {
            println!(
                "  │{}  {}│",
                pad_str(&row.metric, m_w),
                pad_str(&row.value, v_w)
            );
        }

        println!("  {}", bot.style(dim.clone()));
    }
}

fn pad_str(s: &str, width: usize) -> String {
    let s_width = console::measure_text_width(s);
    if s_width >= width {
        return s.to_string();
    }
    let padding = width - s_width;
    // Indent by 1 space, then pad the right
    format!(" {}{}", s, " ".repeat(padding - 1))
}
