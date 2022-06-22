extern crate reqwest; // 0.9.18

use std::sync::mpsc::{self, Receiver, Sender};
use std::{thread, io};
use std::{env, fmt, io::Read};

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
    let args: Vec<String> = env::args().collect();
    let second_arg = args.get(1);
    if second_arg.is_some() {
        let session_id = second_arg.unwrap();
        if session_id.trim().is_empty() {
            return Err(String::from("Session id can't be empty"));
        }
        return Ok(session_id.to_owned());
    }

    println!("Please enter your session id: ");
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .map_err(|err| err.to_string())?;

    let session_id = buffer.trim().to_string();
    if session_id.is_empty() {
        return Err(String::from("Session id can't be empty"));
    }

    Ok(session_id)
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
                if res.is_ok() {
                    thread_send.send(res.unwrap()).unwrap();
                } else {
                    println!("Rental \"{}\" failed", url.clone());
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

    let lines = body.lines();

    // let mut address = String::new();
    let mut queue_length = String::new();
    let mut queue_position = String::new();

    for line in lines {
        if line.contains("sökande just nu") {
            queue_length = line.split_whitespace().next().unwrap().trim().to_string();
        }
        if line.contains("före dig om du anmäler intresse") {
            queue_position = line
                .split_whitespace()
                .next()
                .unwrap()
                .trim()
                .chars()
                .skip(1)
                .map(String::from)
                .collect();
        }
    }

    let rental = Rental {
        link: link.clone(),
        queue_length: queue_length.parse().unwrap(),
        queue_position: queue_position.parse().unwrap(),
    };

    Ok(rental)
}

struct Rental {
    queue_length: u32,
    queue_position: u32,
    link: String,
}

impl fmt::Display for Rental {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {} / {}",
            self.link, self.queue_position, self.queue_length
        )
    }
}
