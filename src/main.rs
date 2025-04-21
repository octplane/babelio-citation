use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use std::io::{self};

struct App {
    isbn: String,
    input_mode: InputMode,
    error: Option<String>,

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
            input_mode: InputMode::Normal,
            book_list: BookList::default(),
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
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Ok((titles, authors, urls, thumbnails))
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        // Render the UI
        terminal.draw(|f| ui(f, &app))?;

        // Handle input events
        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('e') => {
                        app.input_mode = InputMode::Editing;
                    }
                    KeyCode::Char('v') => {
                        app.input_mode = InputMode::Viewing;
                        app.book_list.state.select(Some(0));
                    }
                    KeyCode::Char('q') => return Ok(()),
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        // Perform search
                        let isbn = app.isbn.clone();
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        match rt.block_on(fetch_book_info(&isbn)) {
                            Ok((titles, authors, urls, thumbnails)) => {
                                app.book_list.items.clear();
                                app.book_list.items.extend(
                                    titles
                                        .iter()
                                        .zip(authors.iter())
                                        .zip(urls.iter())
                                        .zip(thumbnails.iter())
                                        .map(|(((title, author), url), thumbnail)| {
                                            Book::new(title, author, url, thumbnail)
                                        }),
                                );
                                app.error = None;
                                app.input_mode = InputMode::Viewing;
                                app.book_list.state.select(Some(0));
                            }
                            Err(e) => {
                                app.error = Some(e.to_string());
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        app.isbn.push(c);
                    }
                    KeyCode::Backspace => {
                        app.isbn.pop();
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
                InputMode::Viewing => match key.code {
                    KeyCode::Down => {}
                    KeyCode::Up => {}
                    KeyCode::Right => {}
                    KeyCode::Char('q') => {
                        app.input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }
}

fn render_results(book_list: &BookList, area: Rect, f: &mut Frame) {
    let items: Vec<_> = book_list
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

    f.render_stateful_widget(list, area, &mut book_list.state.clone());
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.size());

    // Input area
    let input = Paragraph::new(app.isbn.clone())
        .style(match app.input_mode {
            InputMode::Normal | InputMode::Viewing => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .block(Block::default().borders(Borders::ALL).title("ISBN"));
    f.render_widget(input, chunks[0]);

    render_results(&app.book_list, chunks[1], f);

    // Help/Instructions
    let help_text = match app.input_mode {
        InputMode::Normal => "Press 'e' to edit ISBN, 'v' to navigate results, 'q' to quit",
        InputMode::Editing => "Enter ISBN, press Enter to search, Esc to cancel",
        InputMode::Viewing => "Select a result with Up/Down, press 'q' to go back",
    };
    let help = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(help, chunks[2]);

    // Place cursor
    if app.input_mode == InputMode::Editing {
        f.set_cursor(chunks[0].x + app.isbn.len() as u16 + 1, chunks[0].y + 1)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let app = App::default();
    let result = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;

    Ok(())
}
