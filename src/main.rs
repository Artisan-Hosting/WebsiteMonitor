use artisan_middleware::config::AppConfig;
use artisan_middleware::log;
use artisan_middleware::logger::{get_log_level, set_log_level, LogLevel};
use artisan_middleware::notifications::{Email, EmailSecure};
use artisan_middleware::state_persistence::AppState;
use artisan_middleware::{state_persistence::StatePersistence, timestamp::current_timestamp};
use config::{get_config, load_settings, Settings};
use dusa_collection_utils::errors::{ErrorArrayItem, Errors};
use dusa_collection_utils::stringy::Stringy;
use dusa_collection_utils::types::PathType;
use reqwest::header::USER_AGENT;
use reqwest::Client;
use std::time::Duration;
use tokio::time::Instant;
mod config;
mod mailing;

#[tokio::main]
async fn main() {
    // Initialization
    let config: AppConfig = get_config();
    let state_path: PathType = StatePersistence::get_state_path(&config);
    let mut state: AppState = load_initial_state(&config, &state_path);

    let settings: Settings = match load_settings() {
        Ok(loaded_data) => {
            log!(LogLevel::Debug, "settings data loaded: {}", loaded_data);
            loaded_data
        }
        Err(e) => {
            log!(
                LogLevel::Error,
                "Error occoured while loading settings: {}",
                e.to_string()
            );
            state
                .error_log
                .push(ErrorArrayItem::new(Errors::InvalidFile, e.to_string()));
            return;
        }
    };

    // Set log level
    configure_logging(&config, &mut state, &state_path);

    // Debugging print out
    if state.config.debug_mode {
        println!("{}", state);
        println!("{}", settings);
    };

    state.is_active = true;
    state.data = String::from("Website Monitor Initialized");
    update_state(&mut state, &state_path);
    simple_pretty::output("GREEN", "Website monitor running!");

    loop {
        // running health check
        let results = run_health_checks(&settings.websites.urls).await;
        let report = generate_report(&results);

        let email_data: Email = Email {
            subject: Stringy::new("Website Monitor Report"),
            body: Stringy::from_string(report),
        };

        let secure_mail: EmailSecure = match EmailSecure::new(email_data) {
            Ok(loaded_data) => {
                log!(LogLevel::Trace, "Encrypted report data");
                loaded_data
            },
            Err(e) => {
                log!(LogLevel::Error, "Error occurred while preparing to send email: {}", e.to_string());
                state.error_log.push(e);
                update_state(&mut state, &state_path);
                return;
            },
        };

        if let Err(err) = secure_mail.send() {
            log!(LogLevel::Error, "Error occurred while preparing to send email: {}", err.to_string());
            state.error_log.push(err);
            update_state(&mut state, &state_path);
        };

        state.event_counter = state.event_counter + 1;
        update_state(&mut state, &state_path);
        tokio::time::sleep(Duration::from_secs(settings.app.interval_seconds)).await;
    }
}

// Load initial state, creating a new state if necessary
fn load_initial_state(config: &AppConfig, state_path: &PathType) -> AppState {
    match StatePersistence::load_state(state_path) {
        Ok(loaded_data) => {
            log!(LogLevel::Info, "Previous state data loaded");
            loaded_data
        }
        Err(_) => {
            log!(
                LogLevel::Warn,
                "No previous state file found, creating a new one"
            );
            let state = get_initial_state(config);
            if let Err(err) = StatePersistence::save_state(&state, state_path) {
                log!(
                    LogLevel::Error,
                    "Error occurred while saving new state: {}",
                    err
                );
            }
            state
        }
    }
}

// Create an initial state
fn get_initial_state(config: &AppConfig) -> AppState {
    AppState {
        data: String::new(),
        last_updated: current_timestamp(),
        event_counter: 0,
        is_active: false,
        error_log: vec![],
        config: config.clone(),
    }
}

// Update state and persist it to disk
fn update_state(state: &mut AppState, path: &PathType) {
    state.last_updated = current_timestamp();
    if let Err(err) = StatePersistence::save_state(state, path) {
        log!(LogLevel::Error, "Failed to save state: {}", err);
        state.is_active = false;
        state.error_log.push(ErrorArrayItem::new(
            Errors::GeneralError,
            format!("{}", err),
        ));
    }
}

// Configure logging and update the state accordingly
fn configure_logging(config: &AppConfig, state: &mut AppState, state_path: &PathType) {
    if config.debug_mode {
        set_log_level(LogLevel::Debug);
    } else {
        set_log_level(LogLevel::Info);
    }
    log!(LogLevel::Info, "Loglevel: {}", get_log_level());
    state.config.debug_mode = config.debug_mode;
    update_state(state, state_path);
}

use std::collections::HashMap;

async fn run_health_checks(urls: &[String]) -> HashMap<String, HealthCheckResult> {
    let mut results = HashMap::new();

    for url in urls {
        let result = check_website_health(url).await;
        results.insert(url.clone(), result);
        tokio::time::sleep(Duration::from_nanos(500)).await;
    }

    results
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: String,
    pub dns_time_ms: Option<u128>,
    pub response_time_ms: Option<u128>,
    pub body_time_ms: Option<u128>,
    pub error: Option<String>,
}

fn generate_report(results: &HashMap<String, HealthCheckResult>) -> String {
    let mut report = String::from("Website Health Check Report:\n\n");
    let mut total_up = 0;
    let mut total_down = 0;

    for (url, result) in results {
        report.push_str(&format!("URL: {}\n", url));
        report.push_str(&format!("  Status: {}\n", result.status));

        if result.status == "UP" {
            report.push_str(&format!(
                "  DNS & Request Time: {} ms\n",
                result.dns_time_ms.unwrap_or(0)
            ));
            report.push_str(&format!(
                "  Total Response Time: {} ms\n",
                result.response_time_ms.unwrap_or(0)
            ));
            report.push_str(&format!(
                "  Body Read Time: {} ms\n",
                result.body_time_ms.unwrap_or(0)
            ));
            total_up += 1;
        } else {
            report.push_str(&format!(
                "  Error: {}\n",
                result.error.as_deref().unwrap_or("Unknown error")
            ));
            total_down += 1;
        }

        report.push_str("\n");
    }

    report.push_str(&format!(
        "\nSummary:\n  Total Websites Checked: {}\n  Total UP: {}\n  Total DOWN: {}\n\n",
        results.len(),
        total_up,
        total_down
    ));

    report
}

async fn check_website_health(url: &str) -> HealthCheckResult {
    let client = Client::builder().timeout(Duration::from_secs(30)).build();

    match client {
        Ok(client) => {
            let start_time = Instant::now();
            let dns_start = Instant::now();

            match client
                .get(url)
                .header(USER_AGENT, "HealthChecker/1.0")
                .send()
                .await
            {
                Ok(response) => {
                    let dns_duration: u128 = dns_start.elapsed().as_millis();
                    let response_time: u128 = start_time.elapsed().as_millis();
                    let body_start: Instant = Instant::now();

                    match response.text().await {
                        Ok(_) => {
                            let body_duration = body_start.elapsed().as_millis();
                            HealthCheckResult {
                                status: "UP".to_string(),
                                dns_time_ms: Some(dns_duration),
                                response_time_ms: Some(response_time),
                                body_time_ms: Some(body_duration),
                                error: None,
                            }
                        }
                        Err(e) => {
                            log!(
                                LogLevel::Warn,
                                "Error calculating body time: {}",
                                e.to_string()
                            );
                            HealthCheckResult {
                                status: "DOWN".to_string(),
                                dns_time_ms: Some(dns_duration),
                                response_time_ms: Some(response_time),
                                body_time_ms: None,
                                error: Some(e.to_string()),
                            }
                        }
                    }
                }
                Err(e) => HealthCheckResult {
                    status: "DOWN".to_string(),
                    dns_time_ms: None,
                    response_time_ms: None,
                    body_time_ms: None,
                    error: Some(e.to_string()),
                },
            }
        }
        Err(e) => HealthCheckResult {
            status: "DOWN".to_string(),
            dns_time_ms: None,
            response_time_ms: None,
            body_time_ms: None,
            error: Some(e.to_string()),
        },
    }
}
