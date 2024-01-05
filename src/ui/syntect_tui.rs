//! # syntect-ratatui
//!
//! `syntect-ratatui` is a lightweight toolset for converting from text stylised by
//! [syntect](https://docs.rs/syntect/latest/syntect) into stylised text renderable in
//! [ratatui](https://docs.rs/ratatui/latest/ratatui/) applications.
//!
//! Contributions welcome! Feel free to fork and submit a pull request.
use custom_error::custom_error;

custom_error! {
    #[derive(PartialEq)]
    pub SyntectTuiError
    UnknownFontStyle { bits: u8 } = "Unable to convert syntect::FontStyle into ratatui::style::Modifier: unsupported bits ({bits}) value.",
}

/// Converts a line segment highlighed using [syntect::easy::HighlightLines::highlight_line](https://docs.rs/syntect/latest/syntect/easy/struct.HighlightLines.html#method.highlight_line) into a [ratatui::text::Span](https://docs.rs/ratatui/latest/ratatui/text/struct.Span.html).
///
/// Syntect colours are RGBA while Ratatui colours are RGB, so colour conversion is lossy. However, if a Syntect colour's alpha value is `0`, then we preserve this to some degree by returning a value of `None` for that colour (i.e. its colourless).
///
/// Additionally, [syntect::highlighting::Style](https://docs.rs/syntect/latest/syntect/highlighting/struct.Style.html) does not support underlines having a different color than the text it is applied to, unlike [ratatui::style::Style](https://docs.rs/ratatui/latest/ratatui/style/struct.Style.html).
/// Because of this the `underline_color` is set to match the `foreground`.
///
/// # Examples
/// Basic usage:
/// ```
/// let input_text = "hello";
/// let input_style = syntect::highlighting::Style {
///     foreground: syntect::highlighting::Color { r: 255, g: 0, b: 0, a: 255 },
///     background: syntect::highlighting::Color { r: 0, g: 0, b: 0, a: 0 },
///     font_style: syntect::highlighting::FontStyle::BOLD
/// };
/// let expected_style = ratatui::style::Style {
///     fg: Some(ratatui::style::Color::Rgb(255, 0, 0)),
///     bg: None,
///     underline_color: Some(ratatui::style::Color::Rgb(255, 0, 0)),
///     add_modifier: ratatui::style::Modifier::BOLD,
///     sub_modifier: ratatui::style::Modifier::empty()
/// };
/// let expected_span = ratatui::text::Span::styled(input_text, expected_style);
/// let actual_span = syntect_tui::into_span((input_style, input_text)).unwrap();
/// assert_eq!(expected_span, actual_span);
/// ```
///
/// Here's a more complex example that builds upon syntect's own example for `HighlightLines`:
/// ```
/// use syntect::easy::HighlightLines;
/// use syntect::parsing::SyntaxSet;
/// use syntect::highlighting::{ThemeSet, Style};
/// use syntect::util::LinesWithEndings;
/// use syntect_tui::into_span;
///
/// let ps = SyntaxSet::load_defaults_newlines();
/// let ts = ThemeSet::load_defaults();
/// let syntax = ps.find_syntax_by_extension("rs").unwrap();
/// let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
/// let s = "pub struct Wow { hi: u64 }\nfn blah() -> u64 {}";
/// for line in LinesWithEndings::from(s) { // LinesWithEndings enables use of newlines mode
///     let line_spans: Vec<ratatui::text::Span> =
///         h.highlight_line(line, &ps)
///          .unwrap()
///          .into_iter()
///          .filter_map(|segment| into_span(segment).ok())
///          .collect();
///     let spans = ratatui::text::Line::from(line_spans);
///     print!("{:?}", spans);
/// }
///
/// ```
///
/// # Errors
/// Can return `SyntectTuiError::UnknownFontStyle` if the input [FontStyle](https://docs.rs/syntect/latest/syntect/highlighting/struct.FontStyle.html) is not supported.
///
/// All explicit compositions of `BOLD`, `ITALIC` & `UNDERLINE` are supported, however, implicit bitflag coercions are not. For example, even though `FontStyle::from_bits(3)` is coerced to `Some(FontStyle::BOLD | FontStyle::ITALIC)`, we ignore this result as it would be a pain to handle all implicit coercions.
pub fn into_span<'a>(
  (style, content): (syntect::highlighting::Style, &'a str),
) -> Result<ratatui::text::Span<'a>, SyntectTuiError> {
  Ok(ratatui::text::Span::styled(
    String::from(content),
    translate_style(style)?,
  ))
}

/// Converts a
/// [syntect::highlighting::Style](https://docs.rs/syntect/latest/syntect/highlighting/struct.Style.html)
/// into a [ratatui::style::Style](https://docs.rs/ratatui/latest/ratatui/style/struct.Style.html).
///
/// Syntect colours are RGBA while Ratatui colours are RGB, so colour conversion is lossy. However, if a Syntect colour's alpha value is `0`, then we preserve this to some degree by returning a value of `None` for that colour (i.e. its colourless).
///
/// # Examples
/// Basic usage:
/// ```
/// let input = syntect::highlighting::Style {
///     foreground: syntect::highlighting::Color { r: 255, g: 0, b: 0, a: 255 },
///     background: syntect::highlighting::Color { r: 0, g: 0, b: 0, a: 0 },
///     font_style: syntect::highlighting::FontStyle::BOLD
/// };
/// let expected = ratatui::style::Style {
///     fg: Some(ratatui::style::Color::Rgb(255, 0, 0)),
///     bg: None,
///     underline_color: Some(ratatui::style::Color::Rgb(255, 0, 0)),
///     add_modifier: ratatui::style::Modifier::BOLD,
///     sub_modifier: ratatui::style::Modifier::empty()
/// };
/// let actual = syntect_tui::translate_style(input).unwrap();
/// assert_eq!(expected, actual);
/// ```
/// # Errors
/// Can return `SyntectTuiError::UnknownFontStyle` if the input [FontStyle](https://docs.rs/syntect/latest/syntect/highlighting/struct.FontStyle.html) is not supported.
///
/// All explicit compositions of `BOLD`, `ITALIC` & `UNDERLINE` are supported, however, implicit bitflag coercions are not. For example, even though `FontStyle::from_bits(3)` is coerced to `Some(FontStyle::BOLD | FontStyle::ITALIC)`, we ignore this result as it would be a pain to handle all implicit coercions.
pub fn translate_style(
  syntect_style: syntect::highlighting::Style,
) -> Result<ratatui::style::Style, SyntectTuiError> {
  Ok(ratatui::style::Style {
    fg: translate_colour(syntect_style.foreground),
    bg: translate_colour(syntect_style.background),
    underline_color: translate_colour(syntect_style.foreground),
    add_modifier: translate_font_style(syntect_style.font_style)?,
    sub_modifier: ratatui::style::Modifier::empty(),
  })
}

/// Converts a
/// [syntect::highlighting::Color](https://docs.rs/syntect/latest/syntect/highlighting/struct.Color.html)
/// into a [ratatui::style::Color](https://docs.rs/ratatui/latest/ratatui/style/enum.Color.html).
///
///
/// # Examples
/// Basic usage:
/// ```
/// let input = syntect::highlighting::Color { r: 255, g: 0, b: 0, a: 255 };
/// let expected = Some(ratatui::style::Color::Rgb(255, 0, 0));
/// let actual = syntect_tui::translate_colour(input);
/// assert_eq!(expected, actual);
/// ```
/// Syntect colours are RGBA while Ratatui colours are RGB, so colour conversion is lossy. However, if a Syntect colour's alpha value is `0`, then we preserve this to some degree by returning a value of `None` for that colour (i.e. colourless):
/// ```
/// assert_eq!(
///     None,
///     syntect_tui::translate_colour(syntect::highlighting::Color { r: 255, g: 0, b: 0, a: 0 })
/// );
/// ```
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

/// Converts a
/// [syntect::highlighting::FontStyle](https://docs.rs/syntect/latest/syntect/highlighting/struct.FontStyle.html)
/// into a [ratatui::style::Modifier](https://docs.rs/ratatui/latest/ratatui/style/struct.Modifier.html).
///
///
/// # Examples
/// Basic usage:
/// ```
/// let input = syntect::highlighting::FontStyle::BOLD | syntect::highlighting::FontStyle::ITALIC;
/// let expected = ratatui::style::Modifier::BOLD | ratatui::style::Modifier::ITALIC;
/// let actual = syntect_tui::translate_font_style(input).unwrap();
/// assert_eq!(expected, actual);
/// ```
/// # Errors
/// Can return `SyntectTuiError::UnknownFontStyle` if the input [FontStyle](https://docs.rs/syntect/latest/syntect/highlighting/struct.FontStyle.html) is not supported.
///
/// All explicit compositions of `BOLD`, `ITALIC` & `UNDERLINE` are supported, however, implicit bitflag coercions are not. For example, even though `FontStyle::from_bits(3)` is coerced to `Some(FontStyle::BOLD | FontStyle::ITALIC)`, we ignore this result as it would be a pain to handle all implicit coercions.
pub fn translate_font_style(
  syntect_font_style: syntect::highlighting::FontStyle,
) -> Result<ratatui::style::Modifier, SyntectTuiError> {
  use ratatui::style::Modifier;
  use syntect::highlighting::FontStyle;
  match syntect_font_style {
    x if x == FontStyle::empty() => Ok(Modifier::empty()),
    x if x == FontStyle::BOLD => Ok(Modifier::BOLD),
    x if x == FontStyle::ITALIC => Ok(Modifier::ITALIC),
    x if x == FontStyle::UNDERLINE => Ok(Modifier::UNDERLINED),
    x if x == FontStyle::BOLD | FontStyle::ITALIC => Ok(Modifier::BOLD | Modifier::ITALIC),
    x if x == FontStyle::BOLD | FontStyle::UNDERLINE => Ok(Modifier::BOLD | Modifier::UNDERLINED),
    x if x == FontStyle::ITALIC | FontStyle::UNDERLINE => {
      Ok(Modifier::ITALIC | Modifier::UNDERLINED)
    }
    x if x == FontStyle::BOLD | FontStyle::ITALIC | FontStyle::UNDERLINE => {
      Ok(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED)
    }
    unknown => Err(SyntectTuiError::UnknownFontStyle {
      bits: unknown.bits(),
    }),
  }
}

#[cfg(test)]
mod tests {
  use ratatui::{style::Modifier, text::Span};
  use rstest::*;
  use syntect::highlighting::{Color as SyntectColour, FontStyle, Style as SyntectStyle};

  use super::*;

  fn fake_syntect_colour(r: u8, g: u8, b: u8, a: u8) -> SyntectColour {
    SyntectColour { r, g, b, a }
  }

  #[test]
  fn can_convert_to_span() {
    let (r, g, b) = (012_u8, 123_u8, 234_u8);
    let style = SyntectStyle {
      font_style: FontStyle::UNDERLINE,
      foreground: fake_syntect_colour(r, g, b, 128),
      background: fake_syntect_colour(g, b, r, 128),
    };
    let content = "syntax";
    let expected = Ok(Span {
      content: std::borrow::Cow::Owned(String::from(content)),
      style: ratatui::style::Style {
        fg: Some(ratatui::style::Color::Rgb(r, g, b)),
        bg: Some(ratatui::style::Color::Rgb(g, b, r)),
        underline_color: Some(ratatui::style::Color::Rgb(r, g, b)),
        add_modifier: Modifier::UNDERLINED,
        sub_modifier: Modifier::empty(),
      },
    });
    let actual = into_span((style, content));
    assert_eq!(expected, actual);
  }

  #[test]
  fn translate_style_ok() {
    let (r, g, b) = (012_u8, 123_u8, 234_u8);
    let input = SyntectStyle {
      font_style: FontStyle::UNDERLINE,
      foreground: fake_syntect_colour(r, g, b, 128),
      background: fake_syntect_colour(g, b, r, 128),
    };
    let expected = Ok(
      ratatui::style::Style::default()
        .fg(ratatui::style::Color::Rgb(r, g, b))
        .bg(ratatui::style::Color::Rgb(g, b, r))
        .underline_color(ratatui::style::Color::Rgb(r, g, b))
        .add_modifier(Modifier::UNDERLINED),
    );
    let actual = translate_style(input);
    assert_eq!(expected, actual);
  }

  #[test]
  fn translate_style_err() {
    let colour = fake_syntect_colour(012, 123, 234, 128);
    let input = SyntectStyle {
      font_style: unsafe { FontStyle::from_bits_unchecked(254) },
      foreground: colour.to_owned(),
      background: colour,
    };
    let expected = Err(SyntectTuiError::UnknownFontStyle { bits: 254 });
    let actual = translate_style(input);
    assert_eq!(expected, actual);
  }

  #[rstest]
  #[case::with_alpha(
    fake_syntect_colour(012, 123, 234, 128),
    Some(ratatui::style::Color::Rgb(012, 123, 234))
  )]
  #[case::without_alpha(fake_syntect_colour(012, 123, 234, 0), None)]
  fn check_translate_colour(
    #[case] input: SyntectColour,
    #[case] expected: Option<ratatui::style::Color>,
  ) {
    assert_eq!(expected, translate_colour(input));
  }

  #[rstest]
  #[case::empty(FontStyle::empty(), Ok(Modifier::empty()))]
  #[case::bold(FontStyle::BOLD, Ok(Modifier::BOLD))]
  #[case::italic(FontStyle::ITALIC, Ok(Modifier::ITALIC))]
  #[case::underline(FontStyle::UNDERLINE, Ok(Modifier::UNDERLINED))]
  #[case::bold_italic(FontStyle::BOLD | FontStyle::ITALIC, Ok(Modifier::BOLD | Modifier::ITALIC))]
  #[case::bold_underline(FontStyle::BOLD | FontStyle::UNDERLINE, Ok(Modifier::BOLD | Modifier::UNDERLINED))]
  #[case::italic_underline(FontStyle::ITALIC | FontStyle::UNDERLINE, Ok(Modifier::ITALIC | Modifier::UNDERLINED))]
  #[case::bold_italic_underline(
        FontStyle::BOLD | FontStyle::ITALIC | FontStyle::UNDERLINE,
        Ok(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED)
    )]
  #[case::err(
        unsafe { FontStyle::from_bits_unchecked(254) } ,
        Err(SyntectTuiError::UnknownFontStyle { bits: 254 })
    )]
  fn check_translate_font_style(
    #[case] input: FontStyle,
    #[case] expected: Result<Modifier, SyntectTuiError>,
  ) {
    let actual = translate_font_style(input);
    assert_eq!(expected, actual);
  }
}
