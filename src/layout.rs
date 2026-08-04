use comfy_table::{ContentArrangement, Table, presets::NOTHING};

const MIN_WIDTH: u16 = 20;

pub fn terminal_width() -> Option<u16> {
    Table::new().width().or_else(|| {
        std::env::var("COLUMNS")
            .ok()?
            .parse::<u16>()
            .ok()
            .filter(|width| *width >= MIN_WIDTH)
    })
}

pub fn constrain(table: &mut Table) {
    if let Some(width) = terminal_width() {
        table.set_width(width);
    }
}

pub fn wrap(value: &str) -> String {
    let Some(width) = terminal_width() else {
        return value.to_string();
    };
    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(width)
        .add_row([value]);
    table
        .column_mut(0)
        .expect("wrapped text has one column")
        .set_padding((0, 0));
    table
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn push_line(output: &mut String, value: &str) {
    output.push_str(&wrap(value));
    if !output.ends_with('\n') {
        output.push('\n');
    }
}
