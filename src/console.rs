// src/console.rs

use std::{
    collections::HashMap,
    io::{self, Write},
    sync::Mutex,
    time::SystemTime,
};
use termion::{clear, cursor, raw::IntoRawMode, color, style};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub status: String,
    pub last_updated: DateTime<Utc>,
    pub connection_state: ConnectionState,
    pub additional_info: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Connecting,
    Disconnected,
    Error,
    Unknown,
}

pub struct ConsoleManager {
    service_statuses: Mutex<HashMap<String, ServiceStatus>>,
    opportunities: Mutex<Vec<OpportunityDisplay>>,
    start_time: SystemTime,
}

#[derive(Debug, Clone)]
pub struct OpportunityDisplay {
    pub id: String,
    pub dex_pair: String,
    pub token_pair: String,
    pub profit_percent: f64,
    pub profit_usd: f64,
    pub timestamp: DateTime<Utc>,
}

impl ConsoleManager {
    pub fn new() -> Self {
        Self {
            service_statuses: Mutex::new(HashMap::new()),
            opportunities: Mutex::new(Vec::new()),
            start_time: SystemTime::now(),
        }
    }

    pub fn update_status(&self, service: &str, status: &str) {
        let mut statuses = self.service_statuses.lock().unwrap();
        
        let connection_state = self.determine_connection_state(status);
        let service_status = ServiceStatus {
            status: status.to_string(),
            last_updated: Utc::now(),
            connection_state,
            additional_info: None,
        };
        
        statuses.insert(service.to_string(), service_status);
        drop(statuses);
        
        self.refresh_display();
    }

    pub fn update_service_status(&self, service: &str, status: &str, description: &str, additional_info: Option<String>) {
        let mut statuses = self.service_statuses.lock().unwrap();
        
        let connection_state = self.determine_connection_state(status);
        let service_status = ServiceStatus {
            status: description.to_string(),
            last_updated: Utc::now(),
            connection_state,
            additional_info,
        };
        
        statuses.insert(service.to_string(), service_status);
        drop(statuses);
        
        self.refresh_display();
    }

    pub fn update_status_with_info(&self, service: &str, status: &str, additional_info: &str) {
        let mut statuses = self.service_statuses.lock().unwrap();
        
        let connection_state = self.determine_connection_state(status);
        let service_status = ServiceStatus {
            status: status.to_string(),
            last_updated: Utc::now(),
            connection_state,
            additional_info: Some(additional_info.to_string()),
        };
        
        statuses.insert(service.to_string(), service_status);
        drop(statuses);
        
        self.refresh_display();
    }

    pub fn add_opportunity(&self, opportunity: OpportunityDisplay) {
        let mut opportunities = self.opportunities.lock().unwrap();
        opportunities.insert(0, opportunity); // Insert at beginning for newest first
        
        // Keep only last 20 opportunities
        if opportunities.len() > 20 {
            opportunities.truncate(20);
        }
        drop(opportunities);
        
        self.refresh_display();
    }

    pub fn clear_opportunities(&self) {
        let mut opportunities = self.opportunities.lock().unwrap();
        opportunities.clear();
        drop(opportunities);
        
        self.refresh_display();
    }

    fn determine_connection_state(&self, status: &str) -> ConnectionState {
        let status_lower = status.to_lowercase();
        
        if status_lower.contains("connected") || status_lower.contains("fetched") {
            ConnectionState::Connected
        } else if status_lower.contains("connecting") || status_lower.contains("fetching") {
            ConnectionState::Connecting
        } else if status_lower.contains("disconnected") || status_lower.contains("failed") {
            ConnectionState::Disconnected
        } else if status_lower.contains("error") {
            ConnectionState::Error
        } else {
            ConnectionState::Unknown
        }
    }

    fn get_status_indicator(&self, state: &ConnectionState) -> String {
        match state {
            ConnectionState::Connected => format!("{}●{}", color::Fg(color::Green), style::Reset),
            ConnectionState::Connecting => format!("{}●{}", color::Fg(color::Yellow), style::Reset),
            ConnectionState::Disconnected => format!("{}●{}", color::Fg(color::Red), style::Reset),
            ConnectionState::Error => format!("{}●{}", color::Fg(color::Magenta), style::Reset),
            ConnectionState::Unknown => format!("{}●{}", color::Fg(color::White), style::Reset),
        }
    }

    fn refresh_display(&self) {
        let statuses = self.service_statuses.lock().unwrap();
        let opportunities = self.opportunities.lock().unwrap();
        
        // Try to use raw mode, but fall back to regular stdout if it fails
        let stdout_result = io::stdout().into_raw_mode();
        let use_raw_mode = stdout_result.is_ok();
        
        if !use_raw_mode {
            // If raw mode fails, just print a simple status update
            println!("\n=== SOLANA ARBITRAGE BOT STATUS ===");
            
            let uptime = self.start_time.elapsed().unwrap_or_default();
            let uptime_str = format!("{}h {}m {}s", 
                uptime.as_secs() / 3600,
                (uptime.as_secs() % 3600) / 60,
                uptime.as_secs() % 60
            );
            println!("Uptime: {} | Time: {}", uptime_str, Utc::now().format("%H:%M:%S UTC"));
            
            println!("\nDEX CONNECTIONS:");
            let mut sorted_services: Vec<_> = statuses.iter().collect();
            sorted_services.sort_by_key(|(name, _)| *name);
            
            for (service, service_status) in &sorted_services {
                if ["orca", "raydium", "phoenix"].contains(&service.as_str()) {
                    let status_char = match service_status.connection_state {
                        ConnectionState::Connected => "✓",
                        ConnectionState::Connecting => "⋯",
                        ConnectionState::Disconnected => "✗",
                        ConnectionState::Error => "!",
                        ConnectionState::Unknown => "?",
                    };
                    
                    print!("{} {}: {}", status_char, service.to_uppercase(), service_status.status);
                    if let Some(ref info) = service_status.additional_info {
                        print!(" ({})", info);
                    }
                    println!();
                }
            }
            
            if opportunities.is_empty() {
                println!("\nNo arbitrage opportunities detected yet...");
            } else {
                println!("\nRecent opportunities: {}", opportunities.len());
            }
            
            return;
        }
        
        let mut stdout = stdout_result.unwrap();
        
        // Clear screen and hide cursor
        write!(stdout, "{}{}{}", clear::All, cursor::Goto(1, 1), cursor::Hide).unwrap();
        
        // Header with title and uptime
        let uptime = self.start_time.elapsed().unwrap_or_default();
        let uptime_str = format!("{}h {}m {}s", 
            uptime.as_secs() / 3600,
            (uptime.as_secs() % 3600) / 60,
            uptime.as_secs() % 60
        );
        
        write!(stdout, "{}{}═══════════════════════════════════════════════════════════════════════════════{}", 
            style::Bold, color::Fg(color::Cyan), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}{}  🚀 SOLANA ARBITRAGE BOT  {}│{}  Uptime: {}  {}│{}  {} {}", 
            style::Bold, color::Fg(color::Cyan),
            style::Reset, color::Fg(color::White),
            uptime_str, style::Reset,
            color::Fg(color::White), 
            Utc::now().format("%H:%M:%S UTC"),
            style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}{}═══════════════════════════════════════════════════════════════════════════════{}", 
            style::Bold, color::Fg(color::Cyan), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        
        // DEX Status Section
        write!(stdout, "{}{}DEX CONNECTIONS{}", style::Bold, color::Fg(color::White), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        
        // Sort services for consistent display
        let mut sorted_services: Vec<_> = statuses.iter().collect();
        sorted_services.sort_by_key(|(name, _)| *name);
        
        for (service, service_status) in &sorted_services {
            if ["orca", "raydium", "phoenix"].contains(&service.as_str()) {
                let indicator = self.get_status_indicator(&service_status.connection_state);
                let time_ago = (Utc::now() - service_status.last_updated).num_seconds();
                
                write!(stdout, "  {} {}{}{}  │  {}  │  {}{}s ago{}", 
                    indicator,
                    style::Bold, service.to_uppercase(), style::Reset,
                    service_status.status,
                    color::Fg(color::LightBlack), time_ago, style::Reset).unwrap();
                
                if let Some(ref info) = service_status.additional_info {
                    write!(stdout, "  │  {}{}{}", color::Fg(color::LightBlue), info, style::Reset).unwrap();
                }
                let _ = write!(stdout, "\r\n");
            }
        }
        
        // System Status Section
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}{}SYSTEM STATUS{}", style::Bold, color::Fg(color::White), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        
        for (service, service_status) in &sorted_services {
            if !["orca", "raydium", "phoenix"].contains(&service.as_str()) {
                let indicator = self.get_status_indicator(&service_status.connection_state);
                let time_ago = (Utc::now() - service_status.last_updated).num_seconds();
                
                write!(stdout, "  {} {}{}{}  │  {}  │  {}{}s ago{}", 
                    indicator,
                    style::Bold, service, style::Reset,
                    service_status.status,
                    color::Fg(color::LightBlack), time_ago, style::Reset).unwrap();
                let _ = write!(stdout, "\r\n");
            }
        }
        
        // Opportunities Section
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}{}ARBITRAGE OPPORTUNITIES{}", style::Bold, color::Fg(color::White), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        
        if opportunities.is_empty() {
            write!(stdout, "  {}No opportunities detected yet...{}", 
                color::Fg(color::LightBlack), style::Reset).unwrap();
            let _ = write!(stdout, "\r\n");
        } else {
            // Table header
            write!(stdout, "  {}{}TIME      │ DEX PAIR        │ TOKEN PAIR           │ PROFIT %  │ PROFIT USD{}", 
                style::Bold, color::Fg(color::White), style::Reset).unwrap();
            let _ = write!(stdout, "\r\n");
            write!(stdout, "  {}─────────────────────────────────────────────────────────────────────────────{}", 
                color::Fg(color::LightBlack), style::Reset).unwrap();
            let _ = write!(stdout, "\r\n");
            
            for opportunity in opportunities.iter().take(15) {
                let profit_color = if opportunity.profit_percent >= 1.0 {
                    "\x1b[32m" // Green
                } else if opportunity.profit_percent >= 0.5 {
                    "\x1b[33m" // Yellow
                } else {
                    "\x1b[37m" // White
                };
                
                write!(stdout, "  {} │ {:15} │ {:20} │ {}{:7.2}%\x1b[0m │ {}{:8.2}\x1b[0m", 
                    opportunity.timestamp.format("%H:%M:%S"),
                    opportunity.dex_pair,
                    opportunity.token_pair,
                    profit_color, opportunity.profit_percent,
                    profit_color, opportunity.profit_usd).unwrap();
                let _ = write!(stdout, "\r\n");
            }
        }
        
        // Footer
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}{}═══════════════════════════════════════════════════════════════════════════════{}", 
            style::Bold, color::Fg(color::Cyan), style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        write!(stdout, "{}Legend: {}●{} Connected  {}●{} Connecting  {}●{} Disconnected  {}●{} Error{}", 
            color::Fg(color::LightBlack),
            color::Fg(color::Green), style::Reset,
            color::Fg(color::Yellow), style::Reset,
            color::Fg(color::Red), style::Reset,
            color::Fg(color::Magenta), style::Reset,
            style::Reset).unwrap();
        let _ = write!(stdout, "\r\n");
        
        stdout.flush().unwrap();
    }
}