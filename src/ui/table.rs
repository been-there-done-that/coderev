use tabled::{settings::{Style, themes::Theme, style::HorizontalLine}, Table, Tabled};

#[derive(Tabled)]
pub struct TableRow {
    #[tabled(rename = "Metric")]
    pub metric: String,
    #[tabled(rename = "Value")]
    pub value: String,
}

pub struct TableBuilder {
    rows: Vec<TableRow>,
    separators: Vec<usize>,
}

impl TableBuilder {
    pub fn new() -> Self {
        Self { 
            rows: Vec::new(),
            separators: Vec::new(),
        }
    }

    pub fn add_row(&mut self, label: &str, value: &str) {
        self.rows.push(TableRow {
            metric: label.to_string(),
            value: value.to_string(),
        });
    }

    pub fn add_separator(&mut self) {
        if !self.rows.is_empty() {
            self.separators.push(self.rows.len());
        }
    }

    pub fn build(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let mut table = Table::new(&self.rows);
        let mut theme = Theme::from(Style::modern());
        theme.remove_horizontal_lines();
        
        // Add header separator (between header and first data row)
        // Row 0 is header.
        theme.insert_horizontal_line(1, HorizontalLine::inherit(Style::modern()));

        for &idx in &self.separators {
            // idx is number of data rows added.
            // If we have 1 header + N data rows, indices are 0..=N.
            // A separator below data row idx (which is row idx in the table)
            // will be at index idx+1.
            theme.insert_horizontal_line(idx + 1, HorizontalLine::inherit(Style::modern()));
        }

        table.with(theme).to_string()
    }
}

pub fn stats_table(stats: &[(&str, &str)]) -> String {
    let mut builder = TableBuilder::new();
    for (label, value) in stats {
        builder.add_row(label, value);
    }
    builder.build()
}
