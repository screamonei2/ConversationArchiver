// src/console.rs

use std::{collections::HashMap, io::{self, Write}, sync::Mutex};
use termion::{clear, cursor, raw::IntoRawMode};

pub struct ConsoleManager {
    service_statuses: Mutex<HashMap<String, String>>,
}

impl ConsoleManager {
    pub fn new() -> Self {
        Self {
            service_statuses: Mutex::new(HashMap::new()),
        }
    }

    pub fn update_status(&self, service: &str, status: &str) {
        let mut statuses = self.service_statuses.lock().unwrap();
        statuses.insert(service.to_string(), status.to_string());
        self.print_status(&statuses);
    }

    fn print_status(&self, statuses: &HashMap<String, String>) {
        let mut stdout = io::stdout().into_raw_mode().unwrap();
        write!(stdout, "{}{}{}", clear::All, cursor::Goto(1, 1), cursor::Hide).unwrap();

        write!(stdout, "---\n").unwrap();
        write!(stdout, "--- Service Status ---\n").unwrap();
        for (service, status) in statuses.iter() {
            write!(stdout, "{}: {}\n", service, status).unwrap();
        }
        write!(stdout, "---\n").unwrap();
        stdout.flush().unwrap();
    }
}