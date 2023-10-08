extern crate reqwest; // 0.9.18

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::{fmt, io::Read};
use scraper::{Html, Selector};
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session_id = get_session_id()?;
    let mut rentals: Vec<Rental> = fetch_rentals_from_boplats(&session_id)?;

    rentals.sort_by_key(|rental| (rental.queue_position, rental.queue_length));

    for rental in &rentals {
        println!("{}", rental);
    }

    Ok(())
}

fn get_session_id() -> Result<String, String> {
    let cli = Cli::parse();
    let session_id = cli.session_id;

    if session_id.trim().is_empty() {
        return Err(String::from("Session id can't be empty"));
    }
    
    Ok(session_id.to_owned())
}

fn fetch_rentals_from_boplats(session_id: &str) -> Result<Vec<Rental>, Box<dyn std::error::Error>> {
    let body = fetch_list_of_rentals(session_id).unwrap();
    let mut rentals: Vec<Rental> = Vec::new();

    let lines = body.lines();
    let (send, receive): (Sender<Rental>, Receiver<Rental>) = mpsc::channel();

    let owned_session_id = session_id.to_owned();

    let mut children = Vec::new();
    for line in lines {
        if line.contains("search-result-link") {
            let url: String = line.chars().skip(11).take(60).collect();
            let thread_send = send.clone();
            let thread_session_id = owned_session_id.clone();

            children.push(thread::spawn(move || {
                let res = fetch_rental_from_boplats(
                    url.clone(),
                    &thread_session_id
                );
                match res {
                    Ok(rental) => thread_send.send(rental).unwrap(),
                    Err(_) => println!("Fetching rental \"{}\" failed", url.clone()),
                }
            }));
        }
    }

    for child in children {
        child.join().unwrap();
    }

    drop(send);
    for rental in receive {
        rentals.push(rental);
    }

    Ok(rentals)
}

fn fetch_list_of_rentals(session_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let mut res = client
        .post("https://nya.boplats.se/sok")
        .body("itemtype=1hand&city=508A8CB406FE001F00030A60&filterrequirements=on&search=search")
        .header("Cookie", format!("Boplats-session={};", session_id))
        .send()?;

    let mut body = String::new();

    res.read_to_string(&mut body)?;

    Ok(body)
}

fn fetch_rental_from_boplats(
    link: String,
    session_id: &str,
) -> Result<Rental, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let mut res = client
        .get(link.clone())
        .header("Cookie", format!("Boplats-session={};", session_id))
        .send()?;

    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let document = Html::parse_document(&body);

    let queue_position = get_queue_position_from_rental_document(&document);
    let queue_length = get_queue_length_from_rental_document(&document);
    let location = get_location_from_rental_document(&document);

    let rental = Rental {
        queue_length,
        queue_position,
        location,
        link: link.clone(),
    };

    Ok(rental)
}

fn get_location_from_rental_document(document: &Html) -> String {
    let selector = Selector::parse("#maincontent > div > div.pageblock.objectinfo.pure-u-1.pure-u-md-1-2 > div > div.properties > div:nth-child(2) > p").unwrap();
    let location = document.select(&selector).next().unwrap().inner_html();

    location
}

fn get_queue_length_from_rental_document(document: &Html) -> u32 {
    let selector = Selector::parse("#predicted-position").unwrap();
    let container = document.select(&selector).next().unwrap().inner_html();
    let queue_length = container.split_whitespace().next().unwrap().trim().parse::<u32>().unwrap();

    queue_length
}

fn get_queue_position_from_rental_document(document: &Html) -> u32 {
    let selector = Selector::parse("#predicted-position").unwrap();
    let container = document.select(&selector).next().unwrap().inner_html();

    let parentheses_char = "(".chars().next().unwrap();
    let queue_position = container
        .chars()
        .skip_while(|char| char.ne(&parentheses_char))
        .skip(1)
        .take_while(|char| !char.is_whitespace())
        .map(String::from)
        .collect::<String>()
        .parse()
        .unwrap();

    queue_position
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(value_parser)]
    session_id: String,
}

struct Rental {
    queue_length: u32,
    queue_position: u32,
    location: String,
    link: String,
}

impl fmt::Display for Rental {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} / {} {}",
            self.link, self.queue_position, self.queue_length, self.location
        )
    }
}
