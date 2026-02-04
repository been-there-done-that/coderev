use tabled::{Table, Tabled, settings::{Style, Modify, object::Rows, Border}};

#[derive(Tabled)]
struct Row { name: String }

fn main() {
    let mut table = Table::new(vec![Row { name: "test".to_string() }]);
    table.with(Style::modern().off_horizontal());
    // Try to find correct Border method
    // table.with(Modify::new(Rows::new(1..2)).with(Border::default().top('─')));
}
