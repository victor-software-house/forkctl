use crate::layout;
use crate::protocol::ColorMode;
use anstyle::{AnsiColor, Style};
use clap::{Command, CommandFactory};
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use std::fmt::Write as _;
use std::io::{self, Write};

const NARROW_HELP_WIDTH: u16 = 64;

const HEADING: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Cyan)));
const OPTION: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Green)));
const VALUE: Style = Style::new()
    .bold()
    .fg_color(Some(anstyle::Color::Ansi(AnsiColor::Yellow)));
const MUTED: Style = Style::new().fg_color(Some(anstyle::Color::Ansi(AnsiColor::BrightBlack)));

pub fn try_emit<C: CommandFactory>() -> io::Result<bool> {
    let args = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let wants_help = args.len() == 1 || args.iter().any(|arg| arg == "-h" || arg == "--help");
    if !wants_help {
        return Ok(false);
    }
    let color = color_mode(&args);
    let mut root = C::command();
    root.build();
    let command = select_command(root, &args[1..]);
    let output = render(command);
    let choice = match color {
        ColorMode::Auto => anstream::ColorChoice::Auto,
        ColorMode::Always => anstream::ColorChoice::Always,
        ColorMode::Never => anstream::ColorChoice::Never,
    };
    let mut stream = anstream::AutoStream::new(io::stdout().lock(), choice);
    stream.write_all(output.as_bytes())?;
    stream.flush()?;
    Ok(true)
}

fn select_command(mut command: Command, args: &[String]) -> Command {
    for value in args {
        if value == "-h" || value == "--help" {
            break;
        }
        if value.starts_with('-') {
            continue;
        }
        let Some(next) = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == value)
            .cloned()
        else {
            continue;
        };
        command = next;
    }
    command
}

fn color_mode(args: &[String]) -> ColorMode {
    for (index, arg) in args.iter().enumerate() {
        let value = arg.strip_prefix("--color=").or_else(|| {
            (arg == "--color" || arg == "-c")
                .then(|| args.get(index + 1))
                .flatten()
                .map(String::as_str)
        });
        match value {
            Some("always") => return ColorMode::Always,
            Some("never") => return ColorMode::Never,
            Some("auto") => return ColorMode::Auto,
            _ => {}
        }
    }
    ColorMode::Auto
}

pub fn render(mut command: Command) -> String {
    command.build();
    if let Some(width) = layout::terminal_width() {
        command = command.term_width(usize::from(width));
    }
    let mut output = String::new();
    let usage = command.render_usage().to_string();
    let usage = format!(
        "{}{}{}",
        HEADING.render(),
        usage.trim(),
        HEADING.render_reset()
    );
    layout::push_line(&mut output, &usage);
    output.push('\n');
    if let Some(about) = command.get_about() {
        layout::push_line(&mut output, &about.to_string());
        output.push('\n');
    }
    let subcommands = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .map(|subcommand| {
            vec![
                styled(HEADING, subcommand.get_name()),
                subcommand
                    .get_about()
                    .map_or_else(String::new, ToString::to_string),
            ]
        })
        .collect::<Vec<_>>();
    if !subcommands.is_empty() {
        section(&mut output, "Commands", subcommands);
    }
    let positionals = command
        .get_positionals()
        .filter(|arg| !arg.is_hide_set())
        .map(|arg| {
            vec![
                String::new(),
                styled(VALUE, &value_label(arg)),
                String::new(),
                description(arg),
            ]
        })
        .collect::<Vec<_>>();
    if !positionals.is_empty() {
        section(&mut output, "Arguments", positionals);
    }
    let mut headings = Vec::<String>::new();
    for arg in command
        .get_arguments()
        .filter(|arg| !arg.is_positional() && !arg.is_hide_set() && arg.get_id().as_str() != "help")
    {
        let heading = arg
            .get_help_heading()
            .map_or_else(|| "Options".to_string(), ToString::to_string);
        if !headings.contains(&heading) {
            headings.push(heading);
        }
    }
    if command
        .get_arguments()
        .any(|arg| arg.get_id().as_str() == "help")
    {
        headings.push("Help".into());
    }
    for heading in headings {
        let rows = command
            .get_arguments()
            .filter(|arg| {
                if heading == "Help" {
                    return arg.get_id().as_str() == "help";
                }
                !arg.is_positional()
                    && !arg.is_hide_set()
                    && arg.get_id().as_str() != "help"
                    && arg.get_help_heading().map_or("Options", |value| value) == heading
            })
            .map(|arg| {
                vec![
                    arg.get_short()
                        .map_or_else(String::new, |value| styled(OPTION, &format!("-{value}"))),
                    arg.get_long()
                        .map_or_else(String::new, |value| styled(OPTION, &format!("--{value}"))),
                    styled(VALUE, &value_label(arg)),
                    description(arg),
                ]
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            section(&mut output, &heading, rows);
        }
    }
    output
}

fn section(output: &mut String, title: &str, rows: Vec<Vec<String>>) {
    let _ = writeln!(
        output,
        "{}{}{}",
        HEADING.render(),
        title,
        HEADING.render_reset()
    );
    if layout::terminal_width().is_some_and(|width| width < NARROW_HELP_WIDTH) {
        narrow_rows(output, rows);
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    layout::constrain(&mut table);
    for row in rows {
        table.add_row(row);
    }
    let _ = writeln!(output, "{table}");
}

fn narrow_rows(output: &mut String, rows: Vec<Vec<String>>) {
    for row in rows {
        let (label, description) = if row.len() == 2 {
            (row[0].clone(), row[1].clone())
        } else {
            (
                row.iter()
                    .take(3)
                    .filter(|value| !value.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
                row.get(3).cloned().unwrap_or_default(),
            )
        };
        layout::push_indented(output, &label, 2);
        if !description.is_empty() {
            layout::push_indented(output, &description, 4);
        }
    }
    output.push('\n');
}

fn value_label(arg: &clap::Arg) -> String {
    if matches!(
        arg.get_action(),
        clap::ArgAction::SetTrue
            | clap::ArgAction::SetFalse
            | clap::ArgAction::Help
            | clap::ArgAction::Version
    ) {
        return String::new();
    }
    let names = arg
        .get_value_names()
        .map(|names| {
            names
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let choices = arg
        .get_possible_values()
        .into_iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_string())
        .collect::<Vec<_>>();
    if choices.is_empty() {
        names
    } else {
        format!("[{}]", choices.join("|"))
    }
}

fn description(arg: &clap::Arg) -> String {
    let mut description = arg.get_help().map_or_else(String::new, ToString::to_string);
    let defaults = arg
        .get_default_values()
        .iter()
        .map(|value| value.to_string_lossy())
        .collect::<Vec<_>>();
    if !defaults.is_empty()
        && !matches!(
            arg.get_action(),
            clap::ArgAction::SetTrue | clap::ArgAction::SetFalse
        )
    {
        if !description.is_empty() {
            description.push(' ');
        }
        description.push_str(&styled(
            MUTED,
            &format!("[default: {}]", defaults.join(", ")),
        ));
    }
    if arg.is_required_set() {
        if !description.is_empty() {
            description.push(' ');
        }
        description.push_str(&styled(MUTED, "[required]"));
    }
    description
}

fn styled(style: Style, value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("{}{value}{}", style.render(), style.render_reset())
    }
}
