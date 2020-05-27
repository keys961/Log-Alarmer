use std::error::Error;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use chrono::Local;
use inotify::{EventMask, Inotify, WatchMask};
use lettre::{SmtpClient, Transport};
use lettre::smtp::authentication::Credentials;
use lettre_email::EmailBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    log: LogConfig,
    email: EmailConfig,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct LogConfig {
    id: String,
    path: String,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct EmailConfig {
    username: String,
    password: String,
    stmp: String,
    target: String,
    count_threshold: i32,
    time_threshold: i64,
}

fn main() {
    match read_configuration() {
        Ok(config) => {
            monitor_log(&config);
        },
        Err(e) => {
            eprintln!("{:?}", e);
            exit(1);
        }
    }
}

#[allow(unused_must_use)]
fn monitor_log(config: &Config) {
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");
    inotify.add_watch(config.log.path.clone(), WatchMask::MODIFY
        | WatchMask::ATTRIB | WatchMask::DELETE_SELF).expect("Failed to add inotify watch");
    let mut buffer = [0u8; 40960];
    let mut count = 0;
    let mut last_time = Local::now().timestamp_millis();
    loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");
        for event in events {
            if event.mask == EventMask::MODIFY {
                println!("File modified: {:?}", event.name);
                count += 1;
            } else if event.mask == EventMask::ATTRIB {
                println!("File attribute modified: {:?}", event.name);
                inotify.rm_watch(event.wd);
                inotify.add_watch(config.log.path.clone(), WatchMask::MODIFY
                    | WatchMask::ATTRIB | WatchMask::DELETE_SELF).expect("Failed to add inotify watch");
                count += 1;
            } else {
                println!("File deleted: {:?}", event.name);
                sleep(Duration::from_millis(1000));
                inotify.rm_watch(event.wd);
                inotify.add_watch(config.log.path.clone(), WatchMask::MODIFY
                    | WatchMask::ATTRIB | WatchMask::DELETE_SELF).expect("Failed to add inotify watch");
            }
        }

        if count >= config.email.count_threshold as usize &&
            Local::now().timestamp_millis() - last_time >= config.email.time_threshold {
            send_email(config);
            count = 0;
            last_time = Local::now().timestamp_millis();
        }
    }
}

fn send_email(config: &Config) {
    let email = EmailBuilder::new()
        .to(config.email.target.as_str())
        .from(config.email.username.as_str())
        .subject("Bot: ERROR Occurred!!")
        .text(format!("Multiple error occurred on {} at {}", config.log.id, Local::now().to_string()))
        .build()
        .unwrap();
    let creds = Credentials::new(
        config.email.username.clone(),
        config.email.password.clone(),
    );
    let mut mailer = SmtpClient::new_simple(config.email.stmp.as_str())
        .unwrap()
        .credentials(creds)
        .smtp_utf8(true)
        .transport();

    let result = mailer.send(email.into());
    if result.is_ok() {
        println!("Email sent.");
    } else {
        eprintln!("Email failed to send: {}", result.err().unwrap().to_string());
    }
    mailer.close();
}

fn read_configuration() -> Result<Config, Box<dyn Error>> {
    let f = std::fs::File::open("./application.yml")?;
    let d: Config = serde_yaml::from_reader(f).unwrap();
    Ok(d)
}


