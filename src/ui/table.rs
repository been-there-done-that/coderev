use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
pub struct TableRow {
    #[tabled(rename = "Metric")]
    pub metric: String,
    #[tabled(rename = "Value")]
    pub value: String,
}

pub struct TableBuilder {
    rows: Vec<TableRow>,
}

impl TableBuilder {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn add_row(&mut self, label: &str, value: &str) {
        self.rows.push(TableRow {
            metric: label.to_string(),
            value: value.to_string(),
        });
    }

    pub fn build(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }

        let table = Table::new(&self.rows).with(Style::rounded()).to_string();

        table
    }
}

pub fn stats_table(stats: &[(&str, &str)]) -> String {
    let mut builder = TableBuilder::new();
    for (label, value) in stats {
        builder.add_row(label, value);
    }
    builder.build()
}
