pub fn into_span<'a>(
    (style, content): (syntect::highlighting::Style, &'a str),
) -> anyhow::Result<ratatui::text::Span<'a>> {
    Ok(ratatui::text::Span::styled(
        String::from(content),
        translate_style(style)?,
    ))
}

pub fn translate_style(
    syntect_style: syntect::highlighting::Style,
) -> anyhow::Result<ratatui::style::Style> {
    Ok(ratatui::style::Style {
        fg: translate_colour(syntect_style.foreground),
        bg: translate_colour(syntect_style.background),
        underline_color: translate_colour(syntect_style.foreground),
        add_modifier: translate_font_style(syntect_style.font_style)?,
        sub_modifier: ratatui::style::Modifier::empty(),
    })
}

pub fn translate_colour(
    syntect_color: syntect::highlighting::Color,
) -> Option<ratatui::style::Color> {
    match syntect_color {
        syntect::highlighting::Color { r, g, b, a } if a > 0 => {
            Some(ratatui::style::Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

pub fn translate_font_style(
    syntect_font_style: syntect::highlighting::FontStyle,
) -> anyhow::Result<ratatui::style::Modifier> {
    use ratatui::style::Modifier;
    use syntect::highlighting::FontStyle;
    match syntect_font_style {
        x if x == FontStyle::empty() => Ok(Modifier::empty()),
        x if x == FontStyle::BOLD => Ok(Modifier::BOLD),
        x if x == FontStyle::ITALIC => Ok(Modifier::ITALIC),
        x if x == FontStyle::UNDERLINE => Ok(Modifier::UNDERLINED),
        x if x == FontStyle::BOLD | FontStyle::ITALIC => Ok(Modifier::BOLD | Modifier::ITALIC),
        x if x == FontStyle::BOLD | FontStyle::UNDERLINE => {
            Ok(Modifier::BOLD | Modifier::UNDERLINED)
        }
        x if x == FontStyle::ITALIC | FontStyle::UNDERLINE => {
            Ok(Modifier::ITALIC | Modifier::UNDERLINED)
        }
        x if x == FontStyle::BOLD | FontStyle::ITALIC | FontStyle::UNDERLINE => {
            Ok(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED)
        }
        _ => anyhow::bail!("Unknown font style"),
    }
}
