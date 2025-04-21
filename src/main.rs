use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Prompt for ISBN
    print!("Enter an ISBN: ");
    io::stdout().flush()?;
    let mut isbn = String::new();
    io::stdin().read_line(&mut isbn)?;
    let isbn = isbn.trim();

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

    // Write response to file
    let mut file = File::create("page.html")?;
    let body = response.text().await?;
    file.write_all(body.as_bytes())?;

    // Convert encoding (in Rust, you might use a crate like encoding_rs)
    let converted_body = encoding_rs::WINDOWS_1252.decode(&body.as_bytes()).0;
    let mut utf8_file = File::create("pageu.html")?;
    utf8_file.write_all(converted_body.as_bytes())?;

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

    // Print results
    println!("Titles: {:?}", titles);
    println!("Authors: {:?}", authors);
    println!("URLs: {:?}", urls);
    println!("Thumbnails: {:?}", thumbnails);

    // Clean up files
    std::fs::remove_file("page.html")?;
    std::fs::remove_file("pageu.html")?;

    Ok(())
}
