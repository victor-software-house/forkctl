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
    terminal_width().map_or_else(|| value.to_string(), |width| wrap_to(value, width))
}

fn wrap_to(value: &str, width: u16) -> String {
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

pub fn push_indented(output: &mut String, value: &str, indentation: u16) {
    let prefix = " ".repeat(usize::from(indentation));
    let wrapped = terminal_width().map_or_else(
        || value.to_string(),
        |width| wrap_to(value, width.saturating_sub(indentation)),
    );
    for line in wrapped.lines() {
        output.push_str(&prefix);
        output.push_str(line);
        output.push('\n');
    }
}

pub fn push_line(output: &mut String, value: &str) {
    output.push_str(&wrap(value));
    if !output.ends_with('\n') {
        output.push('\n');
    }
}
