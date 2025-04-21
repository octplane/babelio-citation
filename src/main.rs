use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    DefaultTerminal,
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};

struct App {
    isbn: String,
    input_mode: InputMode,
    flash: Option<String>,
    error: Option<String>,
    should_exit: bool,

    book_list: BookList,
}

struct BookList {
    items: Vec<Book>,
    state: ListState,
}

#[derive(Debug)]
struct Book {
    title: String,
    author: String,
    url: String,
    thumbnail: String,
}

#[derive(PartialEq)]
enum InputMode {
    Normal,
    Editing,
    Viewing,
}

impl Default for App {
    fn default() -> Self {
        Self {
            isbn: String::new(),
            flash: None,
            input_mode: InputMode::Normal,
            book_list: BookList::default(),
            should_exit: false,
            error: None,
        }
    }
}

impl BookList {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
        }
    }
}

impl FromIterator<(&'static str, &'static str, &'static str, &'static str)> for BookList {
    fn from_iter<
        T: IntoIterator<Item = (&'static str, &'static str, &'static str, &'static str)>,
    >(
        iter: T,
    ) -> Self {
        let items = iter
            .into_iter()
            .map(|(title, author, url, thumbnail)| Book::new(title, author, url, thumbnail))
            .collect();
        let state = ListState::default();
        Self { items, state }
    }
}

impl Book {
    fn new(title: &str, author: &str, url: &str, thumbnail: &str) -> Self {
        Self {
            title: title.to_string(),
            author: author.to_string(),
            url: url.to_string(),
            thumbnail: thumbnail.to_string(),
        }
    }
}

async fn fetch_book_info(
    isbn: &str,
) -> Result<(Vec<String>, Vec<String>, Vec<String>, Vec<String>), Box<dyn std::error::Error>> {
    // Create custom headers
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:129.0) Gecko/20100101 Firefox/129.0",
        ),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/png,image/svg+xml,*/*;q=0.8"));
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("en-US,en;q=0.8,fr;q=0.5,fr-FR;q=0.3"),
    );

    // Create client with custom headers
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    // Perform POST request
    let response = client
        .post("https://www.babelio.com/recherche.php")
        .form(&[("Recherche", isbn), ("recherche", "")])
        .send()
        .await?;

    // Get response body
    let body = response.text().await?;

    // Convert encoding
    let converted_body = encoding_rs::WINDOWS_1252.decode(&body.as_bytes()).0;

    // Extract information using regex
    let title_regex = Regex::new(r#"<a href="/livres/[^"]*" class="titre1" >([^<]+)</a>"#)?;
    let author_regex = Regex::new(r#"<a href="/auteur/[^"]*" class="libelle" >([^<]+)</a>"#)?;
    let url_regex = Regex::new(r#"<a href="(/livres/[^"]*)"#)?;
    let thumbnail_regex = Regex::new(r#"<img loading=\"lazy\" src=\"([^"]*)\""#)?;

    // Extract titles
    let titles: Vec<_> = title_regex
        .captures_iter(&converted_body)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    // Extract authors
    let authors: Vec<_> = author_regex
        .captures_iter(&converted_body)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    // Extract URLs
    let urls: Vec<_> = url_regex
        .captures_iter(&converted_body)
        .filter_map(|cap| {
            cap.get(1)
                .map(|m| format!("https://www.babelio.com{}", m.as_str()))
        })
        .collect();

    // Extract thumbnails
    let thumbnails: Vec<_> = thumbnail_regex
        .captures_iter(&converted_body)
        .filter_map(|cap| {
            cap.get(1)
                .map(|m| format!("https://www.babelio.com{}", m.as_str()))
        })
        .collect();

    Ok((titles, authors, urls, thumbnails))
}

impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            }
        }
        Ok(())
    }
    fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('e') => {
                    self.input_mode = InputMode::Editing;
                }
                KeyCode::Char('v') => {
                    self.input_mode = InputMode::Viewing;
                    self.book_list.state.select(Some(0));
                }
                KeyCode::Char('q') => {
                    self.should_exit = true;
                }
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Enter => {
                    // Perform search
                    let isbn = self.isbn.clone();
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    match rt.block_on(fetch_book_info(&isbn)) {
                        Ok((titles, authors, urls, thumbnails)) => {
                            self.book_list.items.clear();
                            self.book_list.items.extend(
                                titles
                                    .iter()
                                    .zip(authors.iter())
                                    .zip(urls.iter())
                                    .zip(thumbnails.iter())
                                    .map(|(((title, author), url), thumbnail)| {
                                        Book::new(title, author, url, thumbnail)
                                    }),
                            );
                            self.error = None;
                            self.input_mode = InputMode::Viewing;
                            self.book_list.state.select(Some(0));
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                        }
                    }
                }
                KeyCode::Char(c) => {
                    self.isbn.push(c);
                }
                KeyCode::Backspace => {
                    self.isbn.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                _ => {}
            },
            InputMode::Viewing => match key.code {
                KeyCode::Down => {
                    self.book_list.state.select_next();
                }
                KeyCode::Up => {
                    self.book_list.state.select_previous();
                }
                KeyCode::Enter => {
                    cli_clipboard::set_contents(self.markdown_text().to_owned()).unwrap();
                    self.flash = Some(String::from("Saved to clipboard!"));
                }

                KeyCode::Char('q') => {
                    self.input_mode = InputMode::Normal;
                }
                _ => {}
            },
        }
    }
    fn render_results(&mut self, area: Rect, buf: &mut Buffer) {
        let items: Vec<_> = self
            .book_list
            .items
            .iter()
            .map(|book| {
                let title = format!("{} by {}", book.title, book.author);
                ListItem::new(title)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Results"))
            .highlight_style(Style::default().bg(Color::Yellow))
            .highlight_symbol(">> ");

        StatefulWidget::render(list, area, buf, &mut self.book_list.state);
    }
    fn markdown_text(&mut self) -> String {
        if let Some(ix) = self.book_list.state.selected() {
            let book: &Book = &self.book_list.items[ix];
            format!(
                "{0} par {1}\n- Sur [Babelio]({2})\nThumb: {3}",
                book.title, book.author, book.url, book.thumbnail
            )
        } else {
            format!("Search something!")
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [search, list, render, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(area);

        // Input area
        let input = Paragraph::new(self.isbn.clone())
            .style(match self.input_mode {
                InputMode::Normal | InputMode::Viewing => Style::default(),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            })
            .block(Block::default().borders(Borders::ALL).title("query"));
        Widget::render(input, search, buf);

        self.render_results(list, buf);

        let results = Paragraph::new(self.markdown_text())
            .style(match self.input_mode {
                InputMode::Normal | InputMode::Editing => Style::default(),
                InputMode::Viewing => Style::default().fg(Color::Yellow),
            })
            .block(Block::default().borders(Borders::ALL).title("Markdown"));
        Widget::render(results, render, buf);

        // Help/Instructions
        let help_text = match self.input_mode {
            InputMode::Normal => "Press 'e' to edit query, 'v' to navigate results, 'q' to quit",
            InputMode::Editing => "Enter query, press Enter to search, Esc to cancel",
            InputMode::Viewing => {
                "Select a result with Up/Down, press Enter to copy to clipboard press 'q' to go back"
            }
        };

        let final_text = match &self.flash {
            Some(f) => format!("{} || {}", f, help_text),
            _ => help_text.into(),
        };

        if self.flash.is_some() {
            self.flash = None;
        }

        let help = Paragraph::new(final_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Cyan));
        Widget::render(help, footer, buf);

        // Place cursor
        //if sefl.input_mode == InputMode::Editing {
        //    SetCursorStyle
        //    set_cursor_position(Position::new(
        //        chunks[0].x + app.isbn.len() as u16 + 1,
        //        chunks[0].y + 1,
        //    ))
        //}
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::default().run(terminal);
    ratatui::restore();
    app_result
}
